//! Bot-facing routine tools backed by the existing routines service.

use agent_core::{
    AgentRoutines, BotRoutineSchedule, RoutineContext, RoutineCreateDraft, RoutineError,
    RoutineMutationResult, RoutineSummary,
};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::PgPool;

use crate::error::ApiError;
use crate::routines::{self, RoutineInput};
use crate::schedule::{human_schedule_label, initial_next_run, parse_schedule};

pub struct PostgresAgentRoutines {
    pool: PgPool,
}

impl PostgresAgentRoutines {
    pub fn new(pool: PgPool) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self { pool })
    }
}

fn map_api(err: ApiError) -> RoutineError {
    match err {
        ApiError::NotFound => RoutineError::NotFound,
        ApiError::Validation(m) => RoutineError::Validation(m),
        other => RoutineError::Internal(other.to_string()),
    }
}

pub fn schedule_to_routine_fields(
    schedule: &BotRoutineSchedule,
    timezone: &str,
) -> Result<(String, String, i32), RoutineError> {
    let tz = timezone.trim();
    if tz.is_empty() {
        return Err(RoutineError::Validation("Choose a valid timezone".into()));
    }
    match schedule.repeat.as_str() {
        "every_minutes" => {
            let minutes = schedule.every_minutes.ok_or_else(|| {
                RoutineError::Validation("every_minutes schedules need everyMinutes".into())
            })?;
            if !(15..=43_200).contains(&minutes) {
                return Err(RoutineError::Validation(
                    "Repeat intervals must be between 15 minutes and 30 days".into(),
                ));
            }
            Ok(("interval".into(), minutes.to_string(), minutes))
        }
        "daily" => {
            let at = schedule.at.as_deref().ok_or_else(|| {
                RoutineError::Validation("Daily schedules need at (HH:MM)".into())
            })?;
            Ok(("daily".into(), at.to_string(), 60))
        }
        "weekdays" => {
            let at = schedule.at.as_deref().ok_or_else(|| {
                RoutineError::Validation("Weekday schedules need at (HH:MM)".into())
            })?;
            Ok(("weekly".into(), format!("weekdays|{at}"), 60))
        }
        "weekly" => {
            let at = schedule.at.as_deref().ok_or_else(|| {
                RoutineError::Validation("Weekly schedules need at (HH:MM)".into())
            })?;
            let days = schedule
                .days
                .as_deref()
                .filter(|d| !d.is_empty())
                .unwrap_or("weekdays");
            Ok(("weekly".into(), format!("{days}|{at}"), 60))
        }
        other => Err(RoutineError::Validation(format!(
            "Unsupported schedule repeat: {other}"
        ))),
    }
}

fn to_summary(view: &routines::RoutineView) -> RoutineSummary {
    let preview = if view.instructions.len() > 160 {
        Some(format!("{}…", view.instructions.chars().take(160).collect::<String>()))
    } else {
        Some(view.instructions.clone())
    };
    RoutineSummary {
        id: view.id.clone(),
        name: view.name.clone(),
        enabled: view.enabled,
        schedule_label: view.schedule_label.clone(),
        timezone: view.timezone.clone(),
        next_run_at: view.next_run_at.to_rfc3339(),
        instructions_preview: preview,
    }
}

fn human_message(view: &routines::RoutineView, verb: &str) -> String {
    format!(
        "{verb} \"{}\" — {} ({})",
        view.name,
        view.schedule_label,
        view.timezone
    )
}

#[async_trait]
impl AgentRoutines for PostgresAgentRoutines {
    async fn list(&self, ctx: &RoutineContext) -> Result<Vec<RoutineSummary>, RoutineError> {
        let rows = routines::list(&self.pool, &ctx.owner_id)
            .await
            .map_err(map_api)?;
        Ok(rows
            .into_iter()
            .filter(|row| row.bot_id == ctx.bot_id)
            .map(|row| to_summary(&row))
            .collect())
    }

    async fn validate_create(
        &self,
        ctx: &RoutineContext,
        name: &str,
        instructions: &str,
        schedule: &BotRoutineSchedule,
        timezone: &str,
        destination_conversation_id: Option<&str>,
    ) -> Result<RoutineCreateDraft, RoutineError> {
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() || trimmed_name.chars().count() > 100 {
            return Err(RoutineError::Validation(
                "Give your routine a name of up to 100 characters".into(),
            ));
        }
        let trimmed_instructions = instructions.trim();
        if trimmed_instructions.is_empty() || trimmed_instructions.len() > 100_000 {
            return Err(RoutineError::Validation(
                "Add an assignment of up to 100,000 bytes".into(),
            ));
        }
        let (schedule_kind, schedule_expression, interval_minutes) =
            schedule_to_routine_fields(schedule, timezone)?;
        let schedule_def = parse_schedule(
            &schedule_kind,
            &schedule_expression,
            timezone,
            Some(interval_minutes),
        )
        .map_err(map_api)?;
        let schedule_label = format!("Scheduled · {}", human_schedule_label(&schedule_def));

        let destination = if let Some(id) = destination_conversation_id.filter(|s| !s.is_empty()) {
            let mut tx = self
                .pool
                .begin()
                .await
                .map_err(|e| RoutineError::Internal(e.to_string()))?;
            crate::groups::assert_bot_may_use_conversation(
                &mut tx,
                &ctx.owner_id,
                &ctx.bot_id,
                id,
            )
            .await
            .map_err(map_api)?;
            tx.commit()
                .await
                .map_err(|e| RoutineError::Internal(e.to_string()))?;
            Some(id.to_string())
        } else {
            Some(ctx.source_conversation_id.clone())
        };

        Ok(RoutineCreateDraft {
            name: trimmed_name.to_string(),
            instructions: trimmed_instructions.to_string(),
            timezone: timezone.trim().to_string(),
            schedule_label,
            schedule: schedule.clone(),
            schedule_kind,
            schedule_expression,
            interval_minutes,
            destination_conversation_id: destination,
        })
    }

    async fn create_validated(
        &self,
        ctx: &RoutineContext,
        draft: &RoutineCreateDraft,
    ) -> Result<RoutineMutationResult, RoutineError> {
        let now = Utc::now();
        let schedule_def = parse_schedule(
            &draft.schedule_kind,
            &draft.schedule_expression,
            &draft.timezone,
            Some(draft.interval_minutes),
        )
        .map_err(map_api)?;
        let next_run_at = initial_next_run(&schedule_def, now, now).map_err(map_api)?;
        let input = RoutineInput {
            bot_id: ctx.bot_id.clone(),
            name: draft.name.clone(),
            instructions: draft.instructions.clone(),
            interval_minutes: Some(draft.interval_minutes),
            next_run_at,
            enabled: true,
            schedule_kind: Some(draft.schedule_kind.clone()),
            schedule_expression: Some(draft.schedule_expression.clone()),
            timezone: Some(draft.timezone.clone()),
            destination_conversation_id: draft.destination_conversation_id.clone(),
            failure_policy: None,
            skill_id: None,
            pinned_skill_version: None,
            trigger_mode: Some("schedule".into()),
        };
        input.validate().map_err(map_api)?;
        let view = routines::save(&self.pool, &ctx.owner_id, None, &input)
            .await
            .map_err(map_api)?;
        let routine = to_summary(&view);
        Ok(RoutineMutationResult {
            routine,
            message: format!(
                "Routine created: \"{}\" — {}. Next run {}.",
                view.name,
                view.schedule_label,
                view.next_run_at.to_rfc3339()
            ),
        })
    }

    async fn set_enabled(
        &self,
        ctx: &RoutineContext,
        routine_id: &str,
        enabled: bool,
    ) -> Result<RoutineMutationResult, RoutineError> {
        let existing = routines::get(&self.pool, &ctx.owner_id, routine_id)
            .await
            .map_err(map_api)?;
        if existing.bot_id != ctx.bot_id {
            return Err(RoutineError::Forbidden);
        }
        let view = routines::set_enabled(&self.pool, &ctx.owner_id, routine_id, enabled)
            .await
            .map_err(map_api)?;
        let routine = to_summary(&view);
        let verb = if enabled { "Resumed" } else { "Paused" };
        Ok(RoutineMutationResult {
            routine,
            message: human_message(&view, verb),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_weekday_schedule() {
        let spec = BotRoutineSchedule {
            repeat: "weekdays".into(),
            every_minutes: None,
            at: Some("08:00".into()),
            days: None,
        };
        let (kind, expr, _) = schedule_to_routine_fields(&spec, "America/Chicago").unwrap();
        assert_eq!(kind, "weekly");
        assert_eq!(expr, "weekdays|08:00");
    }

    #[test]
    fn rejects_bad_repeat() {
        let spec = BotRoutineSchedule {
            repeat: "cron".into(),
            every_minutes: None,
            at: None,
            days: None,
        };
        assert!(schedule_to_routine_fields(&spec, "UTC").is_err());
    }
}
