//! PR revision slice: resume, feedback, certified update on same branch.

use agent_core::{
    dispatch_github_coding_tool, AgentGithubCoding, AllowAllApprovalGate, ToolRunContext,
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

use super::github_coding_shell::ShellWorkspaceComputer;
use super::{app_credential, coding_service, demo_readme_path, minimal_tarball_with_readme};

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
        .and(path_regex(format!(r"/repos/acme/demo/tarball/{}$", PR_HEAD_SHA)))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(tarball))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(format!(r"/repos/acme/demo/git/commits/{}", PR_HEAD_SHA)))
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
        .and(path_regex(format!(r"/repos/acme/demo/commits/{}/status", PR_HEAD_SHA)))
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
        .and(path_regex(format!(r"/repos/acme/demo/commits/{}/check-runs", PR_HEAD_SHA)))
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
        .and(path_regex(format!(r"/repos/acme/demo/git/commits/{}", PR_HEAD_SHA)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "sha": PR_HEAD_SHA,
            "tree": { "sha": PR_HEAD_TREE },
            "parents": [{ "sha": "parent_before_pr" }]
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(format!(
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
        .and(path_regex(format!(
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
        .and(path_regex(format!(
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
        .and(path_regex(format!(
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
