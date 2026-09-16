use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use agent_core::{
    ApprovalDecision, ApprovalError, EventSink, ToolApprovalContext, ToolApprovalGate,
    ToolOperationKind,
};
use async_trait::async_trait;
use serde_json::json;

use crate::approval::service::ApprovalService;
use crate::events::cloud_event_sink::CloudEventSink;
use crate::permission_policies::{PermissionPolicyService, PolicyDecision};

pub struct RunScopedApprovalGate {
    service: ApprovalService,
    policies: PermissionPolicyService,
    events: Arc<CloudEventSink>,
    store: Arc<dyn agent_core::RunStore>,
    pub(crate) cancel: Arc<AtomicBool>,
}

impl RunScopedApprovalGate {
    pub fn new(
        service: ApprovalService,
        policies: PermissionPolicyService,
        events: Arc<CloudEventSink>,
        store: Arc<dyn agent_core::RunStore>,
        cancel: Arc<AtomicBool>,
    ) -> Self {
        Self {
            service,
            policies,
            events,
            store,
            cancel,
        }
    }

    async fn record_policy_decision(
        &self,
        context: &ToolApprovalContext,
        decision: PolicyDecision,
        source: &str,
    ) -> Result<(), ApprovalError> {
        let payload = json!({
            "tool": context.tool_name,
            "decision": decision.as_str(),
            "source": source,
            "waitingForApproval": false,
        });
        let receipt = self
            .store
            .append_run_event(&context.request_id, "permission_policy_resolved", &payload)
            .await
            .map_err(|e| ApprovalError::Internal(e.to_string()))?;
        let _ = self
            .events
            .emit_durable(receipt.id, "permission_policy_resolved", &payload);
        Ok(())
    }
}

#[async_trait]
impl ToolApprovalGate for RunScopedApprovalGate {
    async fn authorize(
        &self,
        context: &ToolApprovalContext,
    ) -> Result<ApprovalDecision, ApprovalError> {
        if context.operation_kind == ToolOperationKind::Read {
            return Ok(ApprovalDecision::Allow);
        }
        if self.cancel.load(Ordering::Relaxed) {
            return Err(ApprovalError::Cancelled);
        }

        let resolved = self
            .policies
            .resolve(&context.owner_id, &context.bot_id, &context.tool_name)
            .await
            .map_err(|e| ApprovalError::Internal(e.to_string()))?;

        if resolved.overridable {
            match resolved.decision {
                PolicyDecision::Allow => {
                    self.record_policy_decision(
                        context,
                        PolicyDecision::Allow,
                        resolved.source.as_str(),
                    )
                    .await?;
                    return Ok(ApprovalDecision::Allow);
                }
                PolicyDecision::Deny => {
                    self.record_policy_decision(
                        context,
                        PolicyDecision::Deny,
                        resolved.source.as_str(),
                    )
                    .await?;
                    return Err(ApprovalError::Denied {
                        reason: resolved.denied_message(),
                    });
                }
                PolicyDecision::Ask => {}
            }
        }

        self.service
            .wait_for_mutation_approval(context, &self.events, &self.store, &self.cancel)
            .await
    }
}
