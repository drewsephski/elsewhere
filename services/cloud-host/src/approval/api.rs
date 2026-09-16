use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::app_state::AppState;
use crate::approval::service::ToolApprovalRow;
use crate::auth::Principal;
use crate::error::ApiError;
use agent_core::{approval_action_summary, sanitize_tool_arguments};

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
    ApprovalSummaryResponse {
        approval_id: row.id,
        run_id: row.run_id,
        tool_name,
        tool_kind: row.tool_kind,
        status: row.status,
        summary,
        arguments: sanitized,
        requested_at: row.requested_at,
        expires_at: row.expires_at,
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
