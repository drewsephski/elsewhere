//! Durable bot vs human browser control leases per computer.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use uuid::Uuid;

use agent_core::ApprovalError;

pub const HUMAN_LEASE_TTL_SECS: i64 = 120;
const WAIT_POLL: Duration = Duration::from_millis(400);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlHolder {
    Bot,
    Human,
}

impl ControlHolder {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "bot" => Some(ControlHolder::Bot),
            "human" => Some(ControlHolder::Human),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ComputerControlState {
    pub holder: ControlHolder,
    pub lease_id: Option<String>,
    pub acquired_at: Option<DateTime<Utc>>,
    pub heartbeat_at: Option<DateTime<Utc>>,
}

pub async fn reconcile_stale_human_leases(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE computer_control_leases
         SET holder = 'bot',
             updated_at = NOW()
         WHERE holder = 'human'
           AND heartbeat_at < NOW() - ($1::bigint * INTERVAL '1 second')",
    )
    .bind(HUMAN_LEASE_TTL_SECS)
    .execute(pool)
    .await?;
    Ok(())
}

async fn load_state(
    pool: &PgPool,
    owner_id: &str,
    computer_id: &str,
) -> Result<ComputerControlState, sqlx::Error> {
    reconcile_stale_human_leases(pool).await?;
    let row: Option<(String, String, DateTime<Utc>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT holder, lease_id, acquired_at, heartbeat_at
         FROM computer_control_leases
         WHERE computer_id = $1 AND owner_id = $2",
    )
    .bind(computer_id)
    .bind(owner_id)
    .fetch_optional(pool)
    .await?;

    Ok(match row {
        Some((holder, lease_id, acquired_at, heartbeat_at)) => ComputerControlState {
            holder: ControlHolder::parse(&holder).unwrap_or(ControlHolder::Bot),
            lease_id: Some(lease_id),
            acquired_at: Some(acquired_at),
            heartbeat_at: Some(heartbeat_at),
        },
        None => ComputerControlState {
            holder: ControlHolder::Bot,
            lease_id: None,
            acquired_at: None,
            heartbeat_at: None,
        },
    })
}

pub async fn get_control_state(
    pool: &PgPool,
    owner_id: &str,
    computer_id: &str,
) -> Result<ComputerControlState, sqlx::Error> {
    load_state(pool, owner_id, computer_id).await
}

pub async fn take_human_control(
    pool: &PgPool,
    owner_id: &str,
    computer_id: &str,
) -> Result<ComputerControlState, TakeControlError> {
    reconcile_stale_human_leases(pool)
        .await
        .map_err(TakeControlError::Db)?;

    let lease_id = Uuid::new_v4().to_string();
    let updated: Option<(String, DateTime<Utc>, DateTime<Utc>)> = sqlx::query_as(
        "INSERT INTO computer_control_leases (computer_id, owner_id, holder, lease_id)
         VALUES ($1, $2, 'human', $3)
         ON CONFLICT (computer_id) DO UPDATE
         SET holder = 'human',
             lease_id = EXCLUDED.lease_id,
             acquired_at = NOW(),
             heartbeat_at = NOW(),
             updated_at = NOW(),
             owner_id = EXCLUDED.owner_id
         WHERE computer_control_leases.owner_id = EXCLUDED.owner_id
           AND (
             computer_control_leases.holder = 'bot'
             OR computer_control_leases.heartbeat_at < NOW() - ($4::bigint * INTERVAL '1 second')
           )
         RETURNING lease_id, acquired_at, heartbeat_at",
    )
    .bind(computer_id)
    .bind(owner_id)
    .bind(&lease_id)
    .bind(HUMAN_LEASE_TTL_SECS)
    .fetch_optional(pool)
    .await
    .map_err(TakeControlError::Db)?;

    if let Some((lease_id, acquired_at, heartbeat_at)) = updated {
        return Ok(ComputerControlState {
            holder: ControlHolder::Human,
            lease_id: Some(lease_id),
            acquired_at: Some(acquired_at),
            heartbeat_at: Some(heartbeat_at),
        });
    }

    let current = load_state(pool, owner_id, computer_id)
        .await
        .map_err(TakeControlError::Db)?;
    if current.holder == ControlHolder::Human {
        touch_human_heartbeat(pool, owner_id, computer_id)
            .await
            .map_err(TakeControlError::Db)?;
        return load_state(pool, owner_id, computer_id)
            .await
            .map_err(TakeControlError::Db);
    }
    Err(TakeControlError::Conflict(
        "Browser control is held by another session".into(),
    ))
}

#[derive(Debug)]
pub enum TakeControlError {
    Db(sqlx::Error),
    Conflict(String),
}

pub async fn return_control_to_bot(
    pool: &PgPool,
    owner_id: &str,
    computer_id: &str,
) -> Result<ComputerControlState, sqlx::Error> {
    sqlx::query(
        "UPDATE computer_control_leases
         SET holder = 'bot', updated_at = NOW()
         WHERE computer_id = $1 AND owner_id = $2",
    )
    .bind(computer_id)
    .bind(owner_id)
    .execute(pool)
    .await?;
    sqlx::query(
        "DELETE FROM computer_control_leases
         WHERE computer_id = $1 AND owner_id = $2 AND holder = 'bot'",
    )
    .bind(computer_id)
    .bind(owner_id)
    .execute(pool)
    .await?;
    load_state(pool, owner_id, computer_id).await
}

pub async fn touch_human_heartbeat(
    pool: &PgPool,
    owner_id: &str,
    computer_id: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE computer_control_leases
         SET heartbeat_at = NOW(), updated_at = NOW()
         WHERE computer_id = $1 AND owner_id = $2 AND holder = 'human'",
    )
    .bind(computer_id)
    .bind(owner_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn require_active_human_control(
    pool: &PgPool,
    owner_id: &str,
    computer_id: &str,
) -> Result<(), HumanControlRequired> {
    reconcile_stale_human_leases(pool)
        .await
        .map_err(HumanControlRequired::Db)?;
    let holder: Option<String> = sqlx::query_scalar(
        "SELECT holder FROM computer_control_leases WHERE computer_id = $1 AND owner_id = $2",
    )
    .bind(computer_id)
    .bind(owner_id)
    .fetch_optional(pool)
    .await
    .map_err(HumanControlRequired::Db)?;

    match holder.as_deref() {
        Some("human") => {
            let touched = touch_human_heartbeat(pool, owner_id, computer_id)
                .await
                .map_err(HumanControlRequired::Db)?;
            if touched {
                Ok(())
            } else {
                Err(HumanControlRequired::NotHuman)
            }
        }
        _ => Err(HumanControlRequired::NotHuman),
    }
}

#[derive(Debug)]
pub enum HumanControlRequired {
    Db(sqlx::Error),
    NotHuman,
}

impl HumanControlRequired {
    pub fn message(&self) -> &'static str {
        match self {
            HumanControlRequired::NotHuman => {
                "Take control of the browser before sending manual input"
            }
            HumanControlRequired::Db(_) => "Could not verify browser control state",
        }
    }
}

pub async fn wait_for_bot_browser_control(
    pool: &PgPool,
    owner_id: &str,
    computer_id: &str,
    cancel: &AtomicBool,
) -> Result<(), ApprovalError> {
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(ApprovalError::Cancelled);
        }
        reconcile_stale_human_leases(pool)
            .await
            .map_err(|e| ApprovalError::Internal(e.to_string()))?;
        let holder: Option<String> = sqlx::query_scalar(
            "SELECT holder FROM computer_control_leases WHERE computer_id = $1 AND owner_id = $2",
        )
        .bind(computer_id)
        .bind(owner_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApprovalError::Internal(e.to_string()))?;

        match holder.as_deref() {
            None | Some("bot") => return Ok(()),
            Some("human") => {
                tokio::time::sleep(WAIT_POLL).await;
            }
            Some(other) => {
                return Err(ApprovalError::Internal(format!(
                    "invalid control holder: {other}"
                )));
            }
        }
    }
}
