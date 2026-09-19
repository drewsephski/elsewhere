//! PR revision slice: resume, feedback, certified update on same branch.

use agent_core::{
    dispatch_github_coding_tool, AgentComputer, AgentGithubCoding, AllowAllApprovalGate,
    ToolRunContext,
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

mod github_coding_shell;

use github_coding_shell::ShellWorkspaceComputer;

fn demo_readme_path(run_id: &str) -> String {
    format!(
        "{}/README.md",
        cloud_host::github_coding::checkout_root("acme", "demo", run_id)
    )
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
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;
    use tar::EntryType;

    let mut tar_buf = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_buf);
        let data = b"Hello";
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o100644);
        header.set_entry_type(EntryType::Regular);
        header.set_path("repo-main/README.md").unwrap();
        header.set_cksum();
        builder.append(&header, &data[..]).unwrap();
        builder.finish().unwrap();
    }
    let mut gz = Vec::new();
    let mut enc = GzEncoder::new(&mut gz, Compression::default());
    enc.write_all(&tar_buf).unwrap();
    enc.finish().unwrap();
    gz
}

fn coding_service(
    pool: PgPool,
    connectors: Arc<PostgresAgentConnectors>,
    github: GitHubClient,
) -> Arc<dyn AgentGithubCoding> {
    PostgresAgentGithubCoding::new(connectors, github, pool)
}

const PR_HEAD_SHA: &str = "pr_head_sha_111";
const PR_HEAD_TREE: &str = "pr_head_tree_222";
const WORKING_BRANCH: &str = "elsewhere/readme-fix-runrev1";

fn test_secret_box() -> ConnectorSecretBox {
    let key = base64::engine::general_purpose::STANDARD.encode([7u8; 32]);
    ConnectorSecretBox::from_base64_key(&key).expect("test key")
}

async fn mock_pr_resume_apis(server: &MockServer, tarball: Vec<u8>) {
    Mock::given(method("GET"))
        .and(path_regex(r"^/user/installations$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
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
            "repositories": [{
                "name": "demo",
                "full_name": "acme/demo",
                "owner": { "login": "acme" }
            }]
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"/repos/acme/demo/pulls/42$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "number": 42,
            "state": "open",
            "title": "Fix README",
            "html_url": "https://github.com/acme/demo/pull/42",
            "head": {
                "ref": WORKING_BRANCH,
                "sha": PR_HEAD_SHA,
                "repo": { "full_name": "acme/demo" }
            },
            "base": { "ref": "main" }
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(&format!(r"/repos/acme/demo/tarball/{}$", PR_HEAD_SHA)))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(tarball))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(&format!(r"/repos/acme/demo/git/commits/{}", PR_HEAD_SHA)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "sha": PR_HEAD_SHA,
            "tree": { "sha": PR_HEAD_TREE }
        })))
        .mount(server)
        .await;
}

async fn mock_pr_feedback_apis(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path_regex(r"/repos/acme/demo/pulls/42/reviews"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "id": 1,
            "user": { "login": "reviewer" },
            "state": "CHANGES_REQUESTED",
            "body": "Please fix tests",
            "html_url": "https://github.com/acme/demo/pull/42#review-1"
        }])))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"/repos/acme/demo/pulls/42/comments"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "id": 10,
            "user": { "login": "reviewer" },
            "path": "README.md",
            "line": 1,
            "body": "nit",
            "html_url": "https://github.com/acme/demo/pull/42#discussion-10"
        }])))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"/repos/acme/demo/issues/42/comments"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(&format!(r"/repos/acme/demo/commits/{}/status", PR_HEAD_SHA)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "state": "failure",
            "statuses": [{
                "context": "ci/test",
                "state": "failure",
                "description": "tests failed",
                "target_url": "https://ci.example/run/1"
            }]
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(&format!(r"/repos/acme/demo/commits/{}/check-runs", PR_HEAD_SHA)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "check_runs": [{
                "id": 99,
                "name": "test",
                "status": "completed",
                "conclusion": "failure",
                "details_url": "https://ci.example/check/99"
            }]
        })))
        .mount(server)
        .await;
}

async fn mock_pr_update_apis(server: &MockServer) {
    let branch_encoded = urlencoding::encode(WORKING_BRANCH);
    Mock::given(method("GET"))
        .and(path_regex(&format!(r"/repos/acme/demo/git/commits/{}", PR_HEAD_SHA)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "sha": PR_HEAD_SHA,
            "tree": { "sha": PR_HEAD_TREE },
            "parents": [{ "sha": "parent_before_pr" }]
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(&format!(
            r"/repos/acme/demo/git/trees/{}",
            PR_HEAD_TREE
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tree": [{
                "path": "README.md",
                "mode": "100644",
                "type": "blob",
                "sha": "old_readme_blob_sha"
            }]
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(&format!(
            r"/repos/acme/demo/git/ref/heads/{}$",
            branch_encoded
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": { "sha": PR_HEAD_SHA }
        })))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path_regex(r"/repos/acme/demo/git/blobs"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "sha": "blob_rev_1" })))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path_regex(r"/repos/acme/demo/git/trees"))
        .and(body_string_contains(PR_HEAD_TREE))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "sha": "tree_rev_1" })))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path_regex(r"/repos/acme/demo/git/commits"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "sha": "commit_rev_1" })))
        .mount(server)
        .await;
    Mock::given(method("PATCH"))
        .and(path_regex(&format!(
            r"/repos/acme/demo/git/refs/heads/{}$",
            branch_encoded
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ref": format!("refs/heads/{}", WORKING_BRANCH),
            "object": { "sha": "commit_rev_1" }
        })))
        .mount(server)
        .await;
}

#[sqlx::test(migrations = "./migrations")]
async fn github_resume_rejects_non_elsewhere_branch(pool: PgPool) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex(r"^/user/installations$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "installations": [{ "id": 1, "account": { "login": "acme", "id": 1, "type": "User" } }]
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"/user/installations/1/repositories.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "repositories": [{ "name": "demo", "full_name": "acme/demo", "owner": { "login": "acme" } }]
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"/repos/acme/demo/pulls/7$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "state": "open",
            "head": { "ref": "feature/manual", "sha": "abc", "repo": { "full_name": "acme/demo" } },
            "base": { "ref": "main" }
        })))
        .mount(server)
        .await;

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
    let coding = coding_service(pool, connectors, github);
    let computer = ShellWorkspaceComputer::new();
    let run = ToolRunContext {
        run_id: "run-rev-1".into(),
        request_id: "req-rev-1".into(),
        owner_id: "alice".into(),
        bot_id: "bot".into(),
        computer_id: "comp".into(),
        tool_invocation_id: None,
    };
    let err = dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_resume_pull_request",
        r#"{"owner":"acme","repo":"demo","pullRequestNumber":7}"#,
        &AtomicBool::new(false),
        &AllowAllApprovalGate,
        &run,
    )
    .await
    .expect_err("branch");
    assert!(err.message().contains("Elsewhere-managed"));
}

#[sqlx::test(migrations = "./migrations")]
async fn github_resume_and_feedback(pool: PgPool) {
    let server = MockServer::start().await;
    let tarball = minimal_tarball_with_readme();
    mock_pr_resume_apis(&server, tarball).await;
    mock_pr_feedback_apis(&server).await;

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
    let coding = coding_service(pool, connectors, github);
    let computer = ShellWorkspaceComputer::new();
    let run = ToolRunContext {
        run_id: "run-rev-2".into(),
        request_id: "req-rev-2".into(),
        owner_id: "alice".into(),
        bot_id: "bot".into(),
        computer_id: "comp".into(),
        tool_invocation_id: None,
    };
    let cancel = AtomicBool::new(false);
    let gate = AllowAllApprovalGate;

    let resumed = dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_resume_pull_request",
        r#"{"owner":"acme","repo":"demo","pullRequestNumber":42}"#,
        &cancel,
        &gate,
        &run,
    )
    .await
    .expect("resume");
    assert_eq!(resumed.get("phase").and_then(|v| v.as_str()), Some("resuming_pull_request"));

    let feedback = dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_get_pull_request_feedback",
        r#"{"owner":"acme","repo":"demo","pullRequestNumber":42}"#,
        &cancel,
        &gate,
        &run,
    )
    .await
    .expect("feedback");
    let reviews = feedback
        .get("reviews")
        .and_then(|v| v.as_array())
        .expect("reviews");
    assert!(!reviews.is_empty());
    let checks = feedback.get("checks").expect("checks");
    assert_eq!(
        checks.get("combinedState").and_then(|v| v.as_str()),
        Some("failure")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn github_update_denied_does_not_patch(pool: PgPool) {
    let server = MockServer::start().await;
    mock_pr_resume_apis(&server, minimal_tarball_with_readme()).await;
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
    let coding = coding_service(pool, connectors, github);
    let computer = ShellWorkspaceComputer::new();
    let run = ToolRunContext {
        run_id: "run-rev-3".into(),
        request_id: "req-rev-3".into(),
        owner_id: "alice".into(),
        bot_id: "bot".into(),
        computer_id: "comp".into(),
        tool_invocation_id: None,
    };
    let cancel = AtomicBool::new(false);
    dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_resume_pull_request",
        r#"{"owner":"acme","repo":"demo","pullRequestNumber":42}"#,
        &cancel,
        &AllowAllApprovalGate,
        &run,
    )
    .await
    .expect("resume");
    computer
        .write_file(&demo_readme_path("run-rev-3"), b"Revised")
        .await
        .unwrap();
    dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_review_publish",
        r#"{"checkCommands":[]}"#,
        &cancel,
        &AllowAllApprovalGate,
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

    let err = dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_update_pull_request",
        r#"{"commitMessage":"Address review"}"#,
        &cancel,
        &DenyGate,
        &run,
    )
    .await
    .expect_err("denied");
    assert!(matches!(err, agent_core::ToolError::Denied(_)));
    let posts = server.received_requests().await.unwrap_or_default();
    assert!(
        !posts.iter().any(|r| r.method.as_str() == "PATCH"),
        "denied update must not PATCH GitHub"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn github_update_pushes_same_pr_branch(pool: PgPool) {
    let server = MockServer::start().await;
    mock_pr_resume_apis(&server, minimal_tarball_with_readme()).await;
    mock_pr_update_apis(&server).await;

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
    let coding = coding_service(pool, connectors, github);
    let computer = ShellWorkspaceComputer::new();
    let run = ToolRunContext {
        run_id: "run-rev-4".into(),
        request_id: "req-rev-4".into(),
        owner_id: "alice".into(),
        bot_id: "bot".into(),
        computer_id: "comp".into(),
        tool_invocation_id: None,
    };
    let cancel = AtomicBool::new(false);
    let gate = AllowAllApprovalGate;

    dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_resume_pull_request",
        r#"{"owner":"acme","repo":"demo","pullRequestNumber":42}"#,
        &cancel,
        &gate,
        &run,
    )
    .await
    .expect("resume");
    computer
        .write_file(&demo_readme_path("run-rev-4"), b"Revised content")
        .await
        .unwrap();
    dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_run_check",
        r#"{"command":"true"}"#,
        &cancel,
        &gate,
        &run,
    )
    .await
    .expect("check");
    dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_review_publish",
        r#"{"checkCommands":["true"]}"#,
        &cancel,
        &gate,
        &run,
    )
    .await
    .expect("review");

    let updated = dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_update_pull_request",
        r#"{"commitMessage":"Address review feedback"}"#,
        &cancel,
        &gate,
        &run,
    )
    .await
    .expect("update");
    assert_eq!(
        updated.get("phase").and_then(|v| v.as_str()),
        Some("pull_request_updated")
    );
    assert_eq!(
        updated
            .get("pullRequest")
            .and_then(|p| p.get("number"))
            .and_then(|n| n.as_i64()),
        Some(42)
    );

    let posts = server.received_requests().await.unwrap_or_default();
    assert!(
        !posts.iter().any(|r| {
            r.url.path().ends_with("/pulls") && r.method.as_str() == "POST"
        }),
        "must not open a new pull request"
    );
    assert!(
        posts.iter().any(|r| r.method.as_str() == "PATCH"),
        "expected branch update PATCH"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn github_update_blocks_when_remote_head_moved(pool: PgPool) {
    let server = MockServer::start().await;
    mock_pr_resume_apis(&server, minimal_tarball_with_readme()).await;
    let branch_encoded = urlencoding::encode(WORKING_BRANCH);
    Mock::given(method("GET"))
        .and(path_regex(&format!(
            r"/repos/acme/demo/git/ref/heads/{}$",
            branch_encoded
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": { "sha": "someone_else_pushed" }
        })))
        .mount(server)
        .await;

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
    let coding = coding_service(pool, connectors, github);
    let computer = ShellWorkspaceComputer::new();
    let run = ToolRunContext {
        run_id: "run-rev-5".into(),
        request_id: "req-rev-5".into(),
        owner_id: "alice".into(),
        bot_id: "bot".into(),
        computer_id: "comp".into(),
        tool_invocation_id: None,
    };
    let cancel = AtomicBool::new(false);
    let gate = AllowAllApprovalGate;

    dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_resume_pull_request",
        r#"{"owner":"acme","repo":"demo","pullRequestNumber":42}"#,
        &cancel,
        &gate,
        &run,
    )
    .await
    .expect("resume");
    computer
        .write_file(&demo_readme_path("run-rev-5"), b"Revised")
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

    let err = dispatch_github_coding_tool(
        Some(&coding),
        &computer,
        "github_update_pull_request",
        r#"{"commitMessage":"Address review"}"#,
        &cancel,
        &gate,
        &run,
    )
    .await
    .expect_err("drift");
    assert!(err.message().contains("changed on GitHub"));
}
