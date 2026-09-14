use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::{header, Request, Response, StatusCode},
    middleware::{self, Next},
    Router,
};
use rand::RngCore;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use agent_core::{AgentComputer, ToolApprovalGate, ToolRunContext};

use crate::error::ComputerMcpError;
use crate::tools::ComputerHandler;

pub const MCP_BEARER_ENV_VAR: &str = "ELSEWHERE_MCP_TOKEN";

/// Loopback Streamable HTTP MCP server backed by a single `AgentComputer`.
pub struct ComputerMcpServer {
    url: String,
    bearer_token: String,
    cancel: CancellationToken,
    join: JoinHandle<()>,
}

impl ComputerMcpServer {
    pub async fn start(
        computer: Arc<dyn AgentComputer>,
        gate: Arc<dyn ToolApprovalGate>,
        run: ToolRunContext,
        run_cancel: Arc<AtomicBool>,
    ) -> Result<Self, ComputerMcpError> {
        let bearer_token = generate_bearer_token();
        let cancel = CancellationToken::new();

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| ComputerMcpError::Internal(format!("bind loopback MCP: {e}")))?;
        let addr = listener
            .local_addr()
            .map_err(|e| ComputerMcpError::Internal(e.to_string()))?;
        ensure_loopback(addr)?;

        let mut config = StreamableHttpServerConfig::default();
        config.cancellation_token = cancel.clone();
        config.json_response = true;
        config.legacy_session_mode = false;

        let handler = ComputerHandler::new(computer, gate, run, run_cancel);
        let service: StreamableHttpService<ComputerHandler, LocalSessionManager> =
            StreamableHttpService::new(move || Ok(handler.clone()), Default::default(), config);

        let expected = bearer_token.clone();
        let router = Router::new()
            .nest_service("/mcp", service)
            .layer(middleware::from_fn_with_state(
                expected,
                bearer_auth_middleware,
            ));

        let ct = cancel.clone();
        let join = tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(async move { ct.cancelled().await })
                .await;
        });

        let url = format!("http://{addr}/mcp");
        Ok(Self {
            url,
            bearer_token,
            cancel,
            join,
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn bearer_token(&self) -> &str {
        &self.bearer_token
    }

    pub fn bearer_env_var(&self) -> &'static str {
        MCP_BEARER_ENV_VAR
    }

    pub async fn shutdown(self) {
        self.cancel.cancel();
        let _ = self.join.await;
    }
}

fn ensure_loopback(addr: SocketAddr) -> Result<(), ComputerMcpError> {
    if !addr.ip().is_loopback() {
        return Err(ComputerMcpError::Internal(
            "MCP server must bind to loopback".into(),
        ));
    }
    Ok(())
}

fn generate_bearer_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

async fn bearer_auth_middleware(
    State(expected): State<String>,
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    if request.uri().path() == "/mcp" || request.uri().path().starts_with("/mcp/") {
        let authorized = request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|value| value == format!("Bearer {expected}"));
        if !authorized {
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::from("unauthorized"))
                .unwrap_or_else(|_| Response::new(Body::empty()));
        }
    }
    next.run(request).await
}
