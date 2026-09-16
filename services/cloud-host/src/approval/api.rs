use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::app_state::AppState;
use crate::approval::service::{PersistPolicyResolution, ToolApprovalRow};
use crate::auth::Principal;
use crate::error::ApiError;
use crate::permission_policies::PolicyDecision;
use agent_core::{
    approval_action_summary, is_policy_overridable_tool, policy_action_label,
    sanitize_tool_arguments,
};

#[derive(Debug, Deserialize)]
pub struct ListApprovalsQuery {
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalSummaryResponse {
    pub approval_id: String,
    pub run_id: String,
    pub tool_name: String,
    pub tool_kind: String,
    pub status: String,
    pub summary: String,
    pub arguments: serde_json::Value,
    pub requested_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub bot_id: Option<String>,
    pub bot_name: Option<String>,
    pub policy_overridable: bool,
    pub policy_action_label: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalListResponse {
    pub approvals: Vec<ApprovalSummaryResponse>,
}

fn row_to_response(row: ToolApprovalRow) -> ApprovalSummaryResponse {
    let tool_name = row.tool_name.clone();
    let sanitized = sanitize_tool_arguments(&tool_name, &row.arguments_json);
    let summary = approval_action_summary(&tool_name, &sanitized);
    let policy_overridable = is_policy_overridable_tool(&tool_name);
    ApprovalSummaryResponse {
        approval_id: row.id,
        run_id: row.run_id,
        tool_name: tool_name.clone(),
        tool_kind: row.tool_kind,
        status: row.status,
        summary,
        arguments: sanitized,
        requested_at: row.requested_at,
        expires_at: row.expires_at,
        bot_id: row.bot_id,
        bot_name: row.bot_name,
        policy_overridable,
        policy_action_label: policy_action_label(&tool_name).to_string(),
    }
}

pub async fn list_approvals(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Query(query): Query<ListApprovalsQuery>,
) -> Result<Json<ApprovalListResponse>, ApiError> {
    let rows = state
        .approvals
        .list_for_owner(principal.owner_id(), query.status.as_deref())
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(ApprovalListResponse {
        approvals: rows.into_iter().map(row_to_response).collect(),
    }))
}

pub async fn get_approval(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(approval_id): Path<String>,
) -> Result<Json<ApprovalSummaryResponse>, ApiError> {
    let row = state
        .approvals
        .get_for_owner(principal.owner_id(), &approval_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(row_to_response(row)))
}

pub async fn approve_approval(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(approval_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let ok = state
        .approvals
        .approve(principal.owner_id(), &approval_id, principal.owner_id())
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    if !ok {
        return Err(ApiError::NotFound);
    }
    Ok(Json(
        json!({ "approvalId": approval_id, "decision": "approved" }),
    ))
}

pub async fn deny_approval(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(approval_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let ok = state
        .approvals
        .deny(
            principal.owner_id(),
            &approval_id,
            principal.owner_id(),
            None,
        )
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    if !ok {
        return Err(ApiError::NotFound);
    }
    Ok(Json(
        json!({ "approvalId": approval_id, "decision": "denied" }),
    ))
}

pub async fn always_allow_approval(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(approval_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    persist_and_respond(
        &state,
        principal.owner_id(),
        &approval_id,
        PolicyDecision::Allow,
    )
    .await
}

pub async fn always_deny_approval(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(approval_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    persist_and_respond(
        &state,
        principal.owner_id(),
        &approval_id,
        PolicyDecision::Deny,
    )
    .await
}

async fn persist_and_respond(
    state: &AppState,
    owner_id: &str,
    approval_id: &str,
    decision: PolicyDecision,
) -> Result<Json<serde_json::Value>, ApiError> {
    match state
        .approvals
        .persist_bot_policy_and_resolve(owner_id, approval_id, owner_id, decision)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
    {
        PersistPolicyResolution::Applied {
            bot_id,
            action,
            decision,
            approval_status,
        } => Ok(Json(json!({
            "approvalId": approval_id,
            "decision": approval_status,
            "policy": {
                "botId": bot_id,
                "action": action,
                "decision": decision,
            }
        }))),
        PersistPolicyResolution::NotOverridable => Err(ApiError::Validation(
            "This action cannot be remembered as a Bot permission".into(),
        )),
        PersistPolicyResolution::NotFound => Err(ApiError::NotFound),
    }
}
