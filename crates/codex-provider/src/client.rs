use std::time::Duration;

use serde_json::{json, Value};

use crate::error::CodexProviderError;
use crate::process::{CodexProcessLaunch, ManagedCodexProcess, DEFAULT_REQUEST_TIMEOUT};
use crate::protocol::{
    build_elsewhere_thread_resume_params, build_elsewhere_thread_start_params,
    build_turn_interrupt_params, build_turn_start_params, parse_account_response,
    parse_list_mcp_status, parse_rate_limits_response, parse_thread_resume_response,
    parse_thread_start_response, parse_turn_start_response, CodexAccountState,
    CodexRateLimitsSnapshot, ElsewhereThreadConfig,
};

const CLIENT_NAME: &str = "elsewhere";
const CLIENT_TITLE: &str = "Elsewhere";

pub struct CodexAppServerClient {
    process: ManagedCodexProcess,
    initialized: bool,
}

impl CodexAppServerClient {
    pub async fn launch(launch: CodexProcessLaunch) -> Result<Self, CodexProviderError> {
        let process = ManagedCodexProcess::spawn(launch).await?;
        let mut client = Self {
            process,
            initialized: false,
        };
        client.initialize().await?;
        Ok(client)
    }

    pub async fn initialize(&mut self) -> Result<(), CodexProviderError> {
        if self.initialized {
            return Ok(());
        }
        let params = json!({
            "clientInfo": {
                "name": CLIENT_NAME,
                "title": CLIENT_TITLE,
                "version": env!("CARGO_PKG_VERSION")
            },
            "capabilities": {
                "experimentalApi": true
            }
        });
        self.process
            .request("initialize", params, DEFAULT_REQUEST_TIMEOUT)
            .await?;
        self.process.notify("initialized", None).await?;
        self.initialized = true;
        Ok(())
    }

    pub async fn account(&self) -> Result<CodexAccountState, CodexProviderError> {
        let result = self
            .process
            .request("account/read", json!({}), DEFAULT_REQUEST_TIMEOUT)
            .await?;
        parse_account_response(result)
    }

    pub async fn rate_limits(&self) -> Result<CodexRateLimitsSnapshot, CodexProviderError> {
        let result = self
            .process
            .request("account/rateLimits/read", json!({}), DEFAULT_REQUEST_TIMEOUT)
            .await?;
        parse_rate_limits_response(result)
    }

    pub async fn thread_start_elsewhere(
        &self,
        config: &ElsewhereThreadConfig,
    ) -> Result<String, CodexProviderError> {
        let params = build_elsewhere_thread_start_params(config)?;
        let result = self
            .process
            .request("thread/start", params, Duration::from_secs(120))
            .await?;
        parse_thread_start_response(result)
    }

    pub async fn thread_resume_elsewhere(
        &self,
        thread_id: &str,
        config: &ElsewhereThreadConfig,
    ) -> Result<String, CodexProviderError> {
        let params = build_elsewhere_thread_resume_params(thread_id, config)?;
        let result = self
            .process
            .request("thread/resume", params, Duration::from_secs(120))
            .await?;
        parse_thread_resume_response(result)
    }

    pub async fn thread_compact_start(&self, thread_id: &str) -> Result<(), CodexProviderError> {
        self.process
            .request(
                "thread/compact/start",
                json!({ "threadId": thread_id }),
                Duration::from_secs(120),
            )
            .await?;
        Ok(())
    }

    pub async fn turn_start(
        &self,
        thread_id: &str,
        user_text: &str,
        timeout: Duration,
    ) -> Result<String, CodexProviderError> {
        let params = build_turn_start_params(thread_id, user_text);
        let result = self
            .process
            .request("turn/start", params, timeout)
            .await?;
        parse_turn_start_response(result)
    }

    pub async fn turn_interrupt(
        &self,
        thread_id: &str,
        turn_id: &str,
        timeout: Duration,
    ) -> Result<(), CodexProviderError> {
        let params = build_turn_interrupt_params(thread_id, turn_id);
        self.process
            .request("turn/interrupt", params, timeout)
            .await?;
        Ok(())
    }

    pub async fn thread_archive(&self, thread_id: &str) -> Result<(), CodexProviderError> {
        self.process
            .request(
                "thread/archive",
                json!({ "threadId": thread_id }),
                DEFAULT_REQUEST_TIMEOUT,
            )
            .await?;
        Ok(())
    }

    pub async fn list_mcp_server_tools(
        &self,
        thread_id: &str,
    ) -> Result<Vec<String>, CodexProviderError> {
        self.list_mcp_server_tools_named(thread_id, None).await
    }

    pub async fn list_mcp_server_tools_named(
        &self,
        thread_id: &str,
        server_name: Option<&str>,
    ) -> Result<Vec<String>, CodexProviderError> {
        let result = self
            .process()
            .request(
                "mcpServerStatus/list",
                json!({ "threadId": thread_id, "detail": "full" }),
                DEFAULT_REQUEST_TIMEOUT,
            )
            .await?;
        parse_list_mcp_status(result, server_name)
    }

    pub async fn shutdown(self) -> Result<(), CodexProviderError> {
        self.process.shutdown().await
    }

    pub fn notifications(
        &self,
    ) -> tokio::sync::broadcast::Receiver<crate::protocol::rpc::IncomingMessage> {
        self.process.subscribe_notifications()
    }

    pub fn process(&self) -> &ManagedCodexProcess {
        &self.process
    }

    pub async fn from_process(process: ManagedCodexProcess) -> Result<Self, CodexProviderError> {
        let mut client = Self {
            process,
            initialized: false,
        };
        client.initialize().await?;
        Ok(client)
    }
}
