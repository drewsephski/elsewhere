use agent_core::AgentComputer;
use sprite_computer::{
    default_deny_network_policy, SpriteClient, SpriteClientConfig, SpriteComputer,
    SpriteComputerConfig,
};
use std::time::Duration;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_config(base: &str, name: &str) -> SpriteClientConfig {
    SpriteClientConfig {
        base_url: base.to_string(),
        token: "test-token-secret".into(),
        sprite_name: name.into(),
        workspace_root: "/workspace".into(),
        request_timeout: Duration::from_secs(5),
        auto_create: true,
        network_policy: default_deny_network_policy(),
    }
}

#[tokio::test]
async fn lifecycle_existing_sprite() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sprites/elsewhere-test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "id-1",
            "name": "elsewhere-test",
            "organization": "org",
            "status": "cold"
        })))
        .mount(&server)
        .await;

    let client = SpriteClient::new(test_config(&server.uri(), "elsewhere-test")).unwrap();
    assert!(client.sprite_exists().await.unwrap());
}

#[tokio::test]
async fn lifecycle_existing_sprite_reconciles_network_policy() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sprites/existing-policy"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "id-existing",
            "name": "existing-policy",
            "organization": "org",
            "status": "ready"
        })))
        .expect(2)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/sprites/existing-policy/policy/network"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "rules": [{"action": "deny", "domain": "*"}]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = SpriteClient::new(test_config(&server.uri(), "existing-policy")).unwrap();
    let info = client.ensure_sprite().await.unwrap();
    assert_eq!(info.name, "existing-policy");
    assert_eq!(info.status, "ready");
}

#[tokio::test]
async fn lifecycle_missing_auto_create() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sprites/elsewhere-new"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/sprites"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "id-2",
            "name": "elsewhere-new",
            "organization": "org",
            "status": "cold"
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/sprites/elsewhere-new/policy/network"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "rules": [{"action": "deny", "domain": "*"}]
        })))
        .mount(&server)
        .await;

    let client = SpriteClient::new(test_config(&server.uri(), "elsewhere-new")).unwrap();
    let info = client.ensure_sprite().await.unwrap();
    assert_eq!(info.name, "elsewhere-new");
}

#[tokio::test]
async fn lifecycle_missing_without_auto_create() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sprites/missing"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let mut config = test_config(&server.uri(), "missing");
    config.auto_create = false;
    let client = SpriteClient::new(config).unwrap();
    assert!(matches!(
        client.ensure_sprite().await,
        Err(sprite_computer::SpriteError::NotFound)
    ));
}

#[tokio::test]
async fn filesystem_roundtrip_and_errors() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sprites/fs-test/fs/list"))
        .and(query_param("path", "/workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "path": "/workspace",
            "entries": [{"name":"a.bin","path":"/workspace/a.bin","isDir":false}]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/sprites/fs-test/fs/read"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"\x00\x01\x02"))
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/sprites/fs-test/fs/write"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "path": "/workspace/x",
            "size": 3,
            "mode": "0644"
        })))
        .mount(&server)
        .await;

    let client = SpriteClient::new(test_config(&server.uri(), "fs-test")).unwrap();
    assert_eq!(client.fs_read("/workspace/a.bin").await.unwrap(), vec![0, 1, 2]);
    client
        .fs_write("/workspace/x", b"abc", true)
        .await
        .unwrap();
}

#[tokio::test]
async fn filesystem_read_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sprites/fs-test/fs/read"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let client = SpriteClient::new(test_config(&server.uri(), "fs-test")).unwrap();
    assert!(matches!(
        client.fs_read("/workspace/missing").await,
        Err(sprite_computer::SpriteError::NotFound)
    ));
}

#[tokio::test]
async fn exec_success_and_nonzero() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/sprites/exec-test/exec"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "stdout": "/workspace\n",
            "stderr": "",
            "exitCode": 0
        })))
        .mount(&server)
        .await;

    let client = SpriteClient::new(test_config(&server.uri(), "exec-test")).unwrap();
    let (stdout, _, code) = client
        .exec_http("pwd", "/workspace", Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(code, 0);
    assert!(stdout.contains("/workspace"));
}

#[tokio::test]
async fn exec_nonzero_exit() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/sprites/exec-fail/exec"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "stdout": "",
            "stderr": "fail",
            "exitCode": 2
        })))
        .mount(&server)
        .await;

    let client = SpriteClient::new(test_config(&server.uri(), "exec-fail")).unwrap();
    let (_, _, code) = client
        .exec_http("false", "/workspace", Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(code, 2);
}

#[tokio::test]
async fn policy_applied_on_create() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sprites/policy-test"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/sprites"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "id-3",
            "name": "policy-test",
            "organization": "org",
            "status": "cold"
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/sprites/policy-test/policy/network"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "rules": [{"action": "deny", "domain": "*"}]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = SpriteClient::new(test_config(&server.uri(), "policy-test")).unwrap();
    client.create_sprite().await.unwrap();
}

#[tokio::test]
async fn errors_never_include_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sprites/err-test"))
        .respond_with(
            ResponseTemplate::new(401).set_body_string("Authorization Bearer test-token-secret"),
        )
        .mount(&server)
        .await;

    let client = SpriteClient::new(test_config(&server.uri(), "err-test")).unwrap();
    let err = client.get_sprite().await.unwrap_err();
    let msg = err.to_string();
    assert!(!msg.contains("test-token-secret"));
}

#[tokio::test]
async fn sprite_computer_enforces_workspace_boundary() {
    let server = MockServer::start().await;
    let config = SpriteComputerConfig {
        base_url: server.uri(),
        token: "token".into(),
        sprite_name: "boundary".into(),
        workspace_root: "/workspace".into(),
        request_timeout: Duration::from_secs(5),
        auto_create: false,
        network_policy: default_deny_network_policy(),
        exec_timeout: Duration::from_secs(5),
        browser_enabled: false,
        browser_exec_timeout: Duration::from_secs(120),
    };
    let computer = SpriteComputer::new(config).unwrap();
    let err = computer.read_file("/etc/passwd").await.unwrap_err();
    assert!(matches!(
        err,
        agent_core::ComputerError::SandboxRejected(_)
    ));
}

#[tokio::test]
async fn oversized_file_and_error_bodies_are_rejected() {
    for status in [200, 400] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/sprites/large/fs/read"))
            .respond_with(ResponseTemplate::new(status).set_body_bytes(vec![b'x'; 16 * 1024 * 1024 + 1]))
            .mount(&server).await;
        let client = SpriteClient::new(test_config(&server.uri(), "large")).unwrap();
        let error = client.fs_read("/workspace/large.bin").await.unwrap_err();
        assert!(error.to_string().contains("response body too large"));
    }
}
