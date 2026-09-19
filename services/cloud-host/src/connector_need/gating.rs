use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use agent_core::{
    AgentConnectors, AgentGithubCoding, ConnectorError, ConnectorToolDefinition, GithubCodingError,
    RunStore,
};
use async_trait::async_trait;
use serde_json::Value;

use crate::connector_need::service::ConnectorNeedService;
use crate::connectors::github_access::{
    classify_github_access, connector_error_for_unsatisfied, ConnectorNeedRequest, GithubAccess,
};
use crate::connectors::service::PostgresAgentConnectors;
use crate::events::cloud_event_sink::CloudEventSink;
use crate::github_coding::PostgresAgentGithubCoding;
use crate::redact::redact_secrets;

#[derive(Clone)]
pub struct GithubNeedGate {
    connectors: Arc<PostgresAgentConnectors>,
    needs: ConnectorNeedService,
    events: Arc<CloudEventSink>,
    store: Arc<dyn RunStore>,
    run_id: String,
    bot_id: String,
    owner_id: String,
    cancel: Arc<AtomicBool>,
}

impl GithubNeedGate {
    pub fn new(
        connectors: Arc<PostgresAgentConnectors>,
        needs: ConnectorNeedService,
        events: Arc<CloudEventSink>,
        store: Arc<dyn RunStore>,
        run_id: String,
        bot_id: String,
        owner_id: String,
        cancel: Arc<AtomicBool>,
    ) -> Arc<Self> {
        Arc::new(Self {
            connectors,
            needs,
            events,
            store,
            run_id,
            bot_id,
            owner_id,
            cancel,
        })
    }

    async fn ensure_github_access(
        &self,
        tool_name: &str,
        arguments: &Value,
    ) -> Result<(), ConnectorError> {
        loop {
            match classify_github_access(
                self.connectors.as_ref(),
                &self.owner_id,
                tool_name,
                arguments,
            )
            .await?
            {
                GithubAccess::Ready => return Ok(()),
                GithubAccess::Need(reason) => {
                    let resolution = self
                        .needs
                        .request_and_wait(
                            ConnectorNeedRequest {
                                owner_id: self.owner_id.clone(),
                                run_id: self.run_id.clone(),
                                bot_id: self.bot_id.clone(),
                                tool_name: tool_name.to_string(),
                                reason: reason.clone(),
                                arguments: arguments.clone(),
                            },
                            &self.cancel,
                            &self.events,
                            &self.store,
                            self.connectors.as_ref(),
                        )
                        .await
                        .map_err(|e| ConnectorError::Internal(e.to_string()))?;
                    if resolution.satisfied() {
                        self.connectors.invalidate_github_catalog(&self.owner_id);
                        continue;
                    }
                    return Err(connector_error_for_unsatisfied(&reason));
                }
            }
        }
    }
}

pub struct GatedAgentConnectors {
    inner: Arc<PostgresAgentConnectors>,
    gate: Arc<GithubNeedGate>,
}

impl GatedAgentConnectors {
    pub fn new(inner: Arc<PostgresAgentConnectors>, gate: Arc<GithubNeedGate>) -> Arc<Self> {
        Arc::new(Self { inner, gate })
    }
}

#[async_trait]
impl AgentConnectors for GatedAgentConnectors {
    async fn dispatch_connector_tool(
        &self,
        owner_id: &str,
        tool_name: &str,
        arguments: &Value,
    ) -> Result<Value, ConnectorError> {
        if tool_name.starts_with("github_") {
            self.gate.ensure_github_access(tool_name, arguments).await?;
        }
        self.inner
            .dispatch_connector_tool(owner_id, tool_name, arguments)
            .await
    }

    async fn search_connected_app_tools(
        &self,
        owner_id: &str,
        query: Option<&str>,
        source: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Value, ConnectorError> {
        self.inner
            .search_connected_app_tools(owner_id, query, source, limit)
            .await
    }

    async fn load_connected_app_tool(
        &self,
        owner_id: &str,
        tool_id: &str,
    ) -> Result<ConnectorToolDefinition, ConnectorError> {
        self.inner.load_connected_app_tool(owner_id, tool_id).await
    }

    async fn execute_connected_app_tool(
        &self,
        owner_id: &str,
        tool_id: &str,
        arguments: &Value,
    ) -> Result<Value, ConnectorError> {
        self.inner
            .execute_connected_app_tool(owner_id, tool_id, arguments)
            .await
    }
}

pub struct GatedAgentGithubCoding {
    inner: Arc<PostgresAgentGithubCoding>,
    gate: Arc<GithubNeedGate>,
}

impl GatedAgentGithubCoding {
    pub fn new(inner: Arc<PostgresAgentGithubCoding>, gate: Arc<GithubNeedGate>) -> Arc<Self> {
        Arc::new(Self { inner, gate })
    }
}

#[async_trait]
impl AgentGithubCoding for GatedAgentGithubCoding {
    async fn dispatch_tool(
        &self,
        owner_id: &str,
        run_id: &str,
        request_id: &str,
        computer_id: &str,
        computer: &dyn agent_core::AgentComputer,
        tool_name: &str,
        arguments: &Value,
    ) -> Result<Value, GithubCodingError> {
        self.gate
            .ensure_github_access(tool_name, arguments)
            .await
            .map_err(map_connector_error)?;
        self.inner
            .dispatch_tool(
                owner_id,
                run_id,
                request_id,
                computer_id,
                computer,
                tool_name,
                arguments,
            )
            .await
    }

    async fn confirm_publish_approval(
        &self,
        owner_id: &str,
        run_id: &str,
        computer: &dyn agent_core::AgentComputer,
    ) -> Result<(), GithubCodingError> {
        self.inner
            .confirm_publish_approval(owner_id, run_id, computer)
            .await
    }

    async fn approval_arguments(
        &self,
        owner_id: &str,
        run_id: &str,
        computer: &dyn agent_core::AgentComputer,
        tool_name: &str,
        arguments: &Value,
    ) -> Result<Value, GithubCodingError> {
        self.inner
            .approval_arguments(owner_id, run_id, computer, tool_name, arguments)
            .await
    }
}

fn map_connector_error(err: ConnectorError) -> GithubCodingError {
    match err {
        ConnectorError::NotConnected => GithubCodingError::NotConnected,
        ConnectorError::ReconnectRequired => GithubCodingError::ReconnectRequired,
        ConnectorError::NotFound => GithubCodingError::NotFound,
        ConnectorError::Validation(m) => GithubCodingError::Validation(m),
        ConnectorError::Provider(m) => GithubCodingError::Provider(redact_secrets(&m)),
        ConnectorError::Internal(m) => GithubCodingError::Internal(m),
    }
}
