//! GitHub coding workflow: authorized checkout, review, publish approval, PR idempotency.

mod github_coding_shell;

use agent_core::{
    dispatch_github_coding_tool, AgentComputer, AgentGithubCoding, AllowAllApprovalGate,
    FakeAgentComputer, ToolRunContext,
};
use async_trait::async_trait;
use base64::Engine;
use chrono::{Duration, Utc};
use cloud_host::connectors::{
    db::upsert_github_app_credential, secret::ConnectorSecretBox, service::PostgresAgentConnectors,
    GitHubClient,
};
use cloud_host::github_coding::PostgresAgentGithubCoding;
use serde_json::json;
use sqlx::PgPool;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use wiremock::matchers::{body_string_contains, method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

use github_coding_shell::ShellWorkspaceComputer;

fn test_secret_box() -> ConnectorSecretBox {
    let key = base64::engine::general_purpose::STANDARD.encode([9u8; 32]);
    ConnectorSecretBox::from_base64_key(&key).expect("test key")
}

fn app_credential(access: &str) -> cloud_host::connectors::github_client::GitHubCredential {
    cloud_host::connectors::github_client::GitHubCredential {
        kind: "github_app_user".into(),
        access_token: access.into(),
        refresh_token: "refresh".into(),
        access_expires_at: Utc::now() + Duration::hours(1),
        refresh_expires_at: Utc::now() + Duration::days(30),
        token_type: "bearer".into(),
    }
}

fn minimal_tarball_with_readme() -> Vec<u8> {
    let mut body = Vec::new();
    let name = "repo-main/README.md";
    let mut header = [0u8; 512];
    header[..name.len()].copy_from_slice(name.as_bytes());
    let size_octal = format!("{:012o}", 5);
    header[124..136].copy_from_slice(size_octal.as_bytes());
    header[156] = b'0';
    body.extend_from_slice(&header);
    body.extend_from_slice(b"Hello");
    body.extend(vec![0u8; 512 - 5]);
    body.extend([0u8; 512]);
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::Write::write_all(&mut encoder, &body).unwrap();
    encoder.finish().unwrap()
}

async fn mock_github_api(server: &MockServer, tarball_bytes: Vec<u8>) {
    Mock::given(method("GET"))
        .and(path_regex(r"^/user/installations$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total_count": 1,
            "installations": [{
                "id": 1,
                "account": { "login": "acme", "id": 100, "type": "Organization" }
            }]
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"/user/installations/1/repositories.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total_count": 1,
            "repositories": [{
                "id": 9,
                "name": "demo",
                "full_name": "acme/demo",
                "private": false,
                "owner": { "login": "acme" }
            }]
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"/repos/acme/demo$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "default_branch": "main",
            "full_name": "acme/demo"
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"/repos/acme/demo/tarball/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(tarball_bytes))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"/repos/acme/demo/git/ref/heads/main"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": { "sha": "base_sha_abc123" }
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"/repos/acme/demo/git/commits/base_sha_abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "sha": "base_sha_abc123",
            "tree": { "sha": "base_tree_sha_xyz" }
        })))
        .mount(server)
        .await;
}

async fn insert_workspace_exec_events(
    pool: &PgPool,
    request_id: &str,
    command: &str,
    exit_code: i32,
    ok: bool,
) {
    let call_id = format!("call-{}", command.replace(' ', "-"));
    sqlx::query(
        "INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, 'tool_call', $2)",
    )
    .bind(request_id)
    .bind(json!({
        "tool": "workspace_exec",
        "callId": call_id,
        "arguments": { "command": command }
    }))
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, 'tool_result', $2)",
    )
    .bind(request_id)
    .bind(json!({
        "tool": "workspace_exec",
        "callId": call_id,
        "ok": ok,
        "output": json!({
            "ok": ok,
            "exitCode": exit_code,
            "stdout": "",
            "stderr": ""
        }).to_string()
    }))
    .execute(pool)
    .await
    .unwrap();
}

fn coding_service(
    pool: PgPool,
    connectors: Arc<PostgresAgentConnectors>,
    github: GitHubClient,
) -> Arc<dyn AgentGithubCoding> {
    PostgresAgentGithubCoding::new(connectors, github, pool)
}

#[sqlx::test(migrations = "./migrations")]
async fn github_coding_open_rejects_unauthorized_repo(pool: PgPool) {
    let server = MockServer::start().await;
    mock_github_api(&server, minimal_tarball_with_readme()).await;
    let github = GitHubClient::with_api_base(server.uri(), server.uri());
    let secret = test_secret_box();
    upsert_github_app_credential(
        &pool,
        "alice",
        &json!({ "login": "alice" }),
        &app_credential("gho_test"),
        &secret,
    )
    .await
    .unwrap();
    let connectors = PostgresAgentConnectors::new(pool.clone(), secret.into(), github);
    let coding = coding_service(
        pool.clone(),
        connectors,
        GitHubClient::with_api_base(server.uri(), server.uri()),
    );
    let computer = FakeAgentComputer::new().allow_any_path();
    let run = ToolRunContext {
        run_id: "run-1".into(),
        request_id: "req-1".into(),
        owner_id: "alice".into(),
        bot_id: "bot-1".into(),
        computer_id: "comp-1".into(),
        tool_invocation_id: None,
    };
    let cancel = AtomicBool::new(false);
    let gate = AllowAllApprovalGate;
    let err = dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_open_repository",
        r#"{"owner":"evil","repo":"secret"}"#,
        &cancel,
        &gate,
        &run,
    )
    .await
    .expect_err("unauthorized");
    assert!(
        matches!(err, agent_core::ToolError::Denied(_)),
        "expected denied for repo outside installation catalog, got {err:?}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn github_coding_publish_denied_does_not_hit_github(pool: PgPool) {
    let server = MockServer::start().await;
    let tarball = minimal_tarball_with_readme();
    mock_github_api(&server, tarball).await;
    let github = GitHubClient::with_api_base(server.uri(), server.uri());
    let secret = test_secret_box();
    upsert_github_app_credential(
        &pool,
        "alice",
        &json!({ "login": "alice" }),
        &app_credential("gho_test"),
        &secret,
    )
    .await
    .unwrap();
    let connectors = PostgresAgentConnectors::new(pool.clone(), secret.into(), github.clone());
    let coding = coding_service(pool.clone(), connectors, github);
    let computer = ShellWorkspaceComputer::new();
    let run = ToolRunContext {
        run_id: "run-2".into(),
        request_id: "req-2".into(),
        owner_id: "alice".into(),
        bot_id: "bot-1".into(),
        computer_id: "comp-1".into(),
        tool_invocation_id: None,
    };
    let cancel = AtomicBool::new(false);
    let gate = AllowAllApprovalGate;
    dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_open_repository",
        r#"{"owner":"acme","repo":"demo","taskSlug":"readme-fix"}"#,
        &cancel,
        &gate,
        &run,
    )
    .await
    .expect("open");

    computer
        .write_file(
            "/workspace/repos/acme/demo/README.md",
            b"Hello from Elsewhere",
        )
        .await
        .unwrap();

    dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_review_publish",
        r#"{"checkCommands":[]}"#,
        &cancel,
        &gate,
        &run,
    )
    .await
    .expect("review");

    struct DenyGate;
    #[async_trait]
    impl agent_core::ToolApprovalGate for DenyGate {
        async fn authorize(
            &self,
            _context: &agent_core::ToolApprovalContext,
        ) -> Result<agent_core::ApprovalDecision, agent_core::ApprovalError> {
            Ok(agent_core::ApprovalDecision::Deny {
                reason: "no".into(),
            })
        }
    }

    let denied = dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_publish_pull_request",
        r#"{"title":"Fix README","body":"Hello from Elsewhere"}"#,
        &cancel,
        &DenyGate,
        &run,
    )
    .await
    .expect_err("denied");
    assert!(matches!(denied, agent_core::ToolError::Denied(_)));

    let posts = server.received_requests().await.unwrap_or_default();
    assert!(
        !posts.iter().any(|r| r.method.as_str() == "POST"),
        "denied publish must not POST to GitHub"
    );
}

async fn mock_publish_apis(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path_regex(r"/repos/acme/demo/pulls(\?.*)?$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path_regex(r"/repos/acme/demo/git/blobs"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "sha": "blob_sha_1" })))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path_regex(r"/repos/acme/demo/git/trees"))
        .and(body_string_contains("base_tree_sha_xyz"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "sha": "tree_sha_1" })))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path_regex(r"/repos/acme/demo/git/commits"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "sha": "commit_sha_1" })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"/repos/acme/demo/git/refs/heads/elsewhere%2Freadme-fix-run3$"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({ "message": "Not Found" })))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path_regex(r"/repos/acme/demo/git/refs"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "ref": "refs/heads/elsewhere/readme-fix-run3" })))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path_regex(r"/repos/acme/demo/pulls$"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "number": 42,
            "html_url": "https://github.com/acme/demo/pull/42"
        })))
        .mount(server)
        .await;
}

#[sqlx::test(migrations = "./migrations")]
async fn github_coding_publish_allowed_opens_pull_request(pool: PgPool) {
    let server = MockServer::start().await;
    let tarball = minimal_tarball_with_readme();
    mock_github_api(&server, tarball).await;
    mock_publish_apis(&server).await;
    let github = GitHubClient::with_api_base(server.uri(), server.uri());
    let secret = test_secret_box();
    upsert_github_app_credential(
        &pool,
        "alice",
        &json!({ "login": "alice" }),
        &app_credential("gho_test"),
        &secret,
    )
    .await
    .unwrap();
    let connectors = PostgresAgentConnectors::new(pool.clone(), secret.into(), github.clone());
    let coding = coding_service(pool.clone(), connectors, github);
    let computer = ShellWorkspaceComputer::new();
    let run = ToolRunContext {
        run_id: "run-3".into(),
        request_id: "req-3".into(),
        owner_id: "alice".into(),
        bot_id: "bot-1".into(),
        computer_id: "comp-1".into(),
        tool_invocation_id: None,
    };
    let cancel = AtomicBool::new(false);
    let gate = AllowAllApprovalGate;

    dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_open_repository",
        r#"{"owner":"acme","repo":"demo","taskSlug":"readme-fix"}"#,
        &cancel,
        &gate,
        &run,
    )
    .await
    .expect("open");

    computer
        .write_file(
            "/workspace/repos/acme/demo/README.md",
            b"Hello from Elsewhere",
        )
        .await
        .unwrap();

    insert_workspace_exec_events(&pool, "req-3", "pnpm test", 0, true).await;
    dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_review_publish",
        r#"{"checkCommands":["pnpm test"]}"#,
        &cancel,
        &gate,
        &run,
    )
    .await
    .expect("review");

    let published = dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_publish_pull_request",
        r#"{"title":"Fix README","body":"Hello from Elsewhere"}"#,
        &cancel,
        &gate,
        &run,
    )
    .await
    .expect("publish");

    assert_eq!(
        published.get("pullRequest")
            .and_then(|v| v.get("number"))
            .and_then(|v| v.as_u64()),
        Some(42)
    );
    assert!(
        published
            .get("pullRequest")
            .and_then(|v| v.get("url"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("/pull/42")
    );

    let body = published.to_string();
    assert!(
        !body.contains("gho_test"),
        "access token must not appear in tool result"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn github_coding_rejects_forged_check_payload(pool: PgPool) {
    let server = MockServer::start().await;
    mock_github_api(&server, minimal_tarball_with_readme()).await;
    let github = GitHubClient::with_api_base(server.uri(), server.uri());
    let secret = test_secret_box();
    upsert_github_app_credential(
        &pool,
        "alice",
        &json!({ "login": "alice" }),
        &app_credential("gho_test"),
        &secret,
    )
    .await
    .unwrap();
    let connectors = PostgresAgentConnectors::new(pool.clone(), secret.into(), github.clone());
    let coding = coding_service(pool.clone(), connectors, github);
    let computer = ShellWorkspaceComputer::new();
    let run = ToolRunContext {
        run_id: "run-forge".into(),
        request_id: "req-forge".into(),
        owner_id: "alice".into(),
        bot_id: "bot-1".into(),
        computer_id: "comp-1".into(),
        tool_invocation_id: None,
    };
    let cancel = AtomicBool::new(false);
    let gate = AllowAllApprovalGate;
    dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_open_repository",
        r#"{"owner":"acme","repo":"demo","taskSlug":"readme-fix"}"#,
        &cancel,
        &gate,
        &run,
    )
    .await
    .expect("open");
    let err = dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_review_publish",
        r#"{"checks":[{"command":"pnpm test","exitCode":0,"ok":true}]}"#,
        &cancel,
        &gate,
        &run,
    )
    .await
    .expect_err("forged");
    assert!(matches!(err, agent_core::ToolError::MalformedArguments(_)));
}
