use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use agent_core::{
    ApprovalDecision, ApprovalError, ToolApprovalContext, ToolApprovalGate, ToolOperationKind,
};
use async_trait::async_trait;

use crate::events::cloud_event_sink::CloudEventSink;
use crate::approval::service::ApprovalService;

pub struct RunScopedApprovalGate {
    service: ApprovalService,
    events: Arc<CloudEventSink>,
    store: Arc<dyn agent_core::RunStore>,
    cancel: Arc<AtomicBool>,
}

impl RunScopedApprovalGate {
    pub fn new(
        service: ApprovalService,
        events: Arc<CloudEventSink>,
        store: Arc<dyn agent_core::RunStore>,
        cancel: Arc<AtomicBool>,
    ) -> Self {
        Self {
            service,
            events,
            store,
            cancel,
        }
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
        self.service
            .wait_for_mutation_approval(context, &self.events, &self.store, &self.cancel)
            .await
    }
}
