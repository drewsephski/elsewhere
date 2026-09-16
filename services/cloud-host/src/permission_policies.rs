//! Owner and per-Bot permission policies for approval-gated agent tools.

use agent_core::{
    is_policy_overridable_tool, policy_action_group, policy_action_label, policy_denied_message,
    PolicyActionGroup, POLICY_OVERRIDABLE_TOOL_NAMES,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::resources::get_bot_for_owner;
use crate::error::ApiError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow,
    Ask,
    Deny,
}

impl PolicyDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Ask => "ask",
            Self::Deny => "deny",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, ApiError> {
        match raw {
            "allow" => Ok(Self::Allow),
            "ask" => Ok(Self::Ask),
            "deny" => Ok(Self::Deny),
            _ => Err(ApiError::Validation(
                "decision must be allow, ask, or deny".into(),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicySource {
    Bot,
    Owner,
    Default,
}

impl PolicySource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bot => "bot",
            Self::Owner => "owner",
            Self::Default => "default",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPolicy {
    pub action: String,
    pub decision: PolicyDecision,
    pub source: PolicySource,
    pub overridable: bool,
}

impl ResolvedPolicy {
    pub fn denied_message(&self) -> String {
        policy_denied_message(&self.action)
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct PolicyRow {
    action_key: String,
    decision: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyActionView {
    pub action: String,
    pub label: String,
    pub group: String,
    pub group_label: String,
    pub decision: String,
    pub source: String,
    pub inherited: bool,
    pub inherited_decision: String,
    pub overridable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyCatalogResponse {
    pub actions: Vec<PolicyActionView>,
}

#[derive(Clone)]
pub struct PermissionPolicyService {
    pub pool: PgPool,
}

impl PermissionPolicyService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn validate_overridable_action(action: &str) -> Result<(), ApiError> {
        if is_policy_overridable_tool(action) {
            return Ok(());
        }
        if action == "browser_request_human" {
            return Err(ApiError::Validation(
                "Human browser help cannot be changed with a permission policy".into(),
            ));
        }
        Err(ApiError::Validation(
            "Unknown or non-overridable action".into(),
        ))
    }

    pub async fn resolve(
        &self,
        owner_id: &str,
        bot_id: &str,
        action: &str,
    ) -> Result<ResolvedPolicy, sqlx::Error> {
        if !is_policy_overridable_tool(action) {
            return Ok(ResolvedPolicy {
                action: action.to_string(),
                decision: PolicyDecision::Ask,
                source: PolicySource::Default,
                overridable: false,
            });
        }

        if let Some(row) = self.load_bot_policy(owner_id, bot_id, action, "").await? {
            return Ok(ResolvedPolicy {
                action: action.to_string(),
                decision: parse_stored_decision(&row.decision),
                source: PolicySource::Bot,
                overridable: true,
            });
        }

        if let Some(row) = self.load_owner_policy(owner_id, action, "").await? {
            return Ok(ResolvedPolicy {
                action: action.to_string(),
                decision: parse_stored_decision(&row.decision),
                source: PolicySource::Owner,
                overridable: true,
            });
        }

        Ok(ResolvedPolicy {
            action: action.to_string(),
            decision: PolicyDecision::Ask,
            source: PolicySource::Default,
            overridable: true,
        })
    }

    pub async fn resolve_scoped(
        &self,
        owner_id: &str,
        bot_id: &str,
        action: &str,
        scope_key: &str,
    ) -> Result<ResolvedPolicy, sqlx::Error> {
        if scope_key.is_empty() {
            return self.resolve(owner_id, bot_id, action).await;
        }
        // Never honor an unscoped Allow for connected-app execute.
        if let Some(row) = self
            .load_bot_policy(owner_id, bot_id, action, scope_key)
            .await?
        {
            return Ok(ResolvedPolicy {
                action: action.to_string(),
                decision: parse_stored_decision(&row.decision),
                source: PolicySource::Bot,
                overridable: true,
            });
        }
        if let Some(row) = self.load_owner_policy(owner_id, action, scope_key).await? {
            return Ok(ResolvedPolicy {
                action: action.to_string(),
                decision: parse_stored_decision(&row.decision),
                source: PolicySource::Owner,
                overridable: true,
            });
        }
        Ok(ResolvedPolicy {
            action: action.to_string(),
            decision: PolicyDecision::Ask,
            source: PolicySource::Default,
            overridable: true,
        })
    }

    async fn load_bot_policy(
        &self,
        owner_id: &str,
        bot_id: &str,
        action: &str,
        scope_key: &str,
    ) -> Result<Option<PolicyRow>, sqlx::Error> {
        sqlx::query_as(
            r#"
            SELECT action_key, decision
            FROM bot_permission_policies
            WHERE owner_id = $1 AND bot_id = $2 AND action_key = $3 AND scope_key = $4
            "#,
        )
        .bind(owner_id)
        .bind(bot_id)
        .bind(action)
        .bind(scope_key)
        .fetch_optional(&self.pool)
        .await
    }

    async fn load_owner_policy(
        &self,
        owner_id: &str,
        action: &str,
        scope_key: &str,
    ) -> Result<Option<PolicyRow>, sqlx::Error> {
        sqlx::query_as(
            r#"
            SELECT action_key, decision
            FROM owner_permission_policies
            WHERE owner_id = $1 AND action_key = $2 AND scope_key = $3
            "#,
        )
        .bind(owner_id)
        .bind(action)
        .bind(scope_key)
        .fetch_optional(&self.pool)
        .await
    }

    async fn load_owner_policies(&self, owner_id: &str) -> Result<Vec<PolicyRow>, sqlx::Error> {
        sqlx::query_as(
            r#"
            SELECT action_key, decision
            FROM owner_permission_policies
            WHERE owner_id = $1
            "#,
        )
        .bind(owner_id)
        .fetch_all(&self.pool)
        .await
    }

    async fn load_bot_policies(
        &self,
        owner_id: &str,
        bot_id: &str,
    ) -> Result<Vec<PolicyRow>, sqlx::Error> {
        sqlx::query_as(
            r#"
            SELECT action_key, decision
            FROM bot_permission_policies
            WHERE owner_id = $1 AND bot_id = $2
            "#,
        )
        .bind(owner_id)
        .bind(bot_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn owner_catalog(
        &self,
        owner_id: &str,
    ) -> Result<PolicyCatalogResponse, sqlx::Error> {
        let rows = self.load_owner_policies(owner_id).await?;
        Ok(PolicyCatalogResponse {
            actions: catalog_views(&rows, None),
        })
    }

    pub async fn bot_catalog(
        &self,
        owner_id: &str,
        bot_id: &str,
    ) -> Result<Option<PolicyCatalogResponse>, ApiError> {
        let bot = get_bot_for_owner(&self.pool, owner_id, bot_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        if bot.is_none() {
            return Ok(None);
        }
        let owner_rows = self
            .load_owner_policies(owner_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        let bot_rows = self
            .load_bot_policies(owner_id, bot_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        Ok(Some(PolicyCatalogResponse {
            actions: catalog_views(&owner_rows, Some(&bot_rows)),
        }))
    }

    pub async fn upsert_owner_decision(
        &self,
        owner_id: &str,
        action: &str,
        decision: Option<PolicyDecision>,
    ) -> Result<(), ApiError> {
        Self::validate_overridable_action(action)?;
        match decision {
            None | Some(PolicyDecision::Ask) => {
                sqlx::query(
                    "DELETE FROM owner_permission_policies WHERE owner_id = $1 AND action_key = $2",
                )
                .bind(owner_id)
                .bind(action)
                .execute(&self.pool)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            }
            Some(decision) => {
                let id = Uuid::new_v4().to_string();
                sqlx::query(
                    r#"
                    INSERT INTO owner_permission_policies (
                        id, owner_id, action_key, decision, resource_scope, scope_key, created_at, updated_at
                    )
                    VALUES ($1, $2, $3, $4, NULL, '', NOW(), NOW())
                    ON CONFLICT (owner_id, action_key, scope_key)
                    DO UPDATE SET decision = EXCLUDED.decision, updated_at = NOW()
                    "#,
                )
                .bind(&id)
                .bind(owner_id)
                .bind(action)
                .bind(decision.as_str())
                .execute(&self.pool)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            }
        }
        Ok(())
    }

    pub async fn upsert_bot_decision(
        &self,
        owner_id: &str,
        bot_id: &str,
        action: &str,
        decision: Option<PolicyDecision>,
    ) -> Result<bool, ApiError> {
        Self::validate_overridable_action(action)?;
        let bot = get_bot_for_owner(&self.pool, owner_id, bot_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        if bot.is_none() {
            return Ok(false);
        }
        match decision {
            None => {
                sqlx::query(
                    r#"
                    DELETE FROM bot_permission_policies
                    WHERE owner_id = $1 AND bot_id = $2 AND action_key = $3
                    "#,
                )
                .bind(owner_id)
                .bind(bot_id)
                .bind(action)
                .execute(&self.pool)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            }
            Some(decision) => {
                let id = Uuid::new_v4().to_string();
                sqlx::query(
                    r#"
                    INSERT INTO bot_permission_policies (
                        id, owner_id, bot_id, action_key, decision, resource_scope, scope_key, created_at, updated_at
                    )
                    VALUES ($1, $2, $3, $4, $5, NULL, '', NOW(), NOW())
                    ON CONFLICT (bot_id, action_key, scope_key)
                    DO UPDATE SET decision = EXCLUDED.decision, updated_at = NOW()
                    WHERE bot_permission_policies.owner_id = EXCLUDED.owner_id
                    "#,
                )
                .bind(&id)
                .bind(owner_id)
                .bind(bot_id)
                .bind(action)
                .bind(decision.as_str())
                .execute(&self.pool)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            }
        }
        Ok(true)
    }

    pub async fn upsert_bot_decision_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner_id: &str,
        bot_id: &str,
        action: &str,
        decision: PolicyDecision,
        now: DateTime<Utc>,
        scope_key: &str,
        resource_scope: Option<serde_json::Value>,
    ) -> Result<(), sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO bot_permission_policies (
                id, owner_id, bot_id, action_key, decision, resource_scope, scope_key, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)
            ON CONFLICT (bot_id, action_key, scope_key)
            DO UPDATE SET decision = EXCLUDED.decision, updated_at = EXCLUDED.updated_at,
                resource_scope = EXCLUDED.resource_scope
            WHERE bot_permission_policies.owner_id = EXCLUDED.owner_id
            "#,
        )
        .bind(&id)
        .bind(owner_id)
        .bind(bot_id)
        .bind(action)
        .bind(decision.as_str())
        .bind(resource_scope)
        .bind(scope_key)
        .bind(now)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }
}

fn parse_stored_decision(raw: &str) -> PolicyDecision {
    match raw {
        "allow" => PolicyDecision::Allow,
        "deny" => PolicyDecision::Deny,
        _ => PolicyDecision::Ask,
    }
}

fn catalog_views(
    owner_rows: &[PolicyRow],
    bot_rows: Option<&[PolicyRow]>,
) -> Vec<PolicyActionView> {
    POLICY_OVERRIDABLE_TOOL_NAMES
        .iter()
        .map(|action| {
            let group = policy_action_group(action).unwrap_or(PolicyActionGroup::Files);
            let owner_decision = owner_rows
                .iter()
                .find(|row| row.action_key == *action)
                .map(|row| parse_stored_decision(&row.decision))
                .unwrap_or(PolicyDecision::Ask);
            let bot_decision = bot_rows.and_then(|rows| {
                rows.iter()
                    .find(|row| row.action_key == *action)
                    .map(|row| parse_stored_decision(&row.decision))
            });
            let (decision, source, inherited) = match bot_decision {
                Some(decision) => (decision, PolicySource::Bot, false),
                None if owner_rows.iter().any(|row| row.action_key == *action) => {
                    (owner_decision, PolicySource::Owner, true)
                }
                None => (PolicyDecision::Ask, PolicySource::Default, true),
            };
            PolicyActionView {
                action: (*action).to_string(),
                label: policy_action_label(action).to_string(),
                group: group.as_str().to_string(),
                group_label: group.label().to_string(),
                decision: decision.as_str().to_string(),
                source: source.as_str().to_string(),
                inherited,
                inherited_decision: owner_decision.as_str().to_string(),
                overridable: true,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_catalog_is_ask_without_stored_rows() {
        let views = catalog_views(&[], None);
        assert!(views.iter().all(|view| view.decision == "ask"));
        assert!(views.iter().all(|view| view.inherited));
        assert!(views.iter().all(|view| view.source == "default"));
        assert!(views.iter().any(|view| view.action == "workspace_write"));
        assert!(views.iter().any(|view| view.action == "bot_delegate"));
        assert!(views.iter().any(|view| view.action == "run_subagent"));
        assert!(!views
            .iter()
            .any(|view| view.action == "browser_request_human"));
    }

    #[test]
    fn bot_override_wins_over_owner_default() {
        let owner = vec![PolicyRow {
            action_key: "workspace_write".into(),
            decision: "deny".into(),
        }];
        let bot = vec![PolicyRow {
            action_key: "workspace_write".into(),
            decision: "allow".into(),
        }];
        let views = catalog_views(&owner, Some(&bot));
        let write = views
            .iter()
            .find(|view| view.action == "workspace_write")
            .expect("write");
        assert_eq!(write.decision, "allow");
        assert_eq!(write.source, "bot");
        assert!(!write.inherited);
        assert_eq!(write.inherited_decision, "deny");
        let exec = views
            .iter()
            .find(|view| view.action == "workspace_exec")
            .expect("exec");
        assert_eq!(exec.decision, "ask");
        assert!(exec.inherited);
    }
}
