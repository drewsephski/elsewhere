//! Local Mac pairing protocol, credential hashing, and owner isolation.

mod support;

use cloud_host::auth::{JwtVerifier, JwtVerifierConfig};
use cloud_host::local_mac::db;
use cloud_host::{build_router, test_signing, AppState};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::time::Duration;
use tower::ServiceExt;
use uuid::Uuid;

use support::test_config;

const PAIRING_TEST_KEY: [u8; 32] = [7u8; 32];

fn pairing_test_config() -> cloud_host::Config {
    let mut config = test_config();
    config.auth_mode = cloud_host::config::AuthMode::Hybrid;
    config.jwt_issuer = Some("http://localhost:3000".into());
    config.jwt_audience = Some("elsewhere-cloud-host".into());
    config.jwt_jwks_url = Some("http://127.0.0.1:9/jwks".into());
    config.cors_web_origin = Some("http://localhost:3000".into());
    config.local_mac_credential_key = Some(PAIRING_TEST_KEY);
    config
}

const TEST_JWT_ISSUER: &str = "http://localhost:3000";
const TEST_JWT_AUDIENCE: &str = "elsewhere-cloud-host";

async fn try_test_pool() -> Option<PgPool> {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere".into());
    let pool = tokio::time::timeout(Duration::from_secs(2), PgPool::connect(&url))
        .await
        .ok()?
        .ok()?;
    sqlx::migrate!("./migrations").run(&pool).await.ok()?;
    Some(pool)
}

fn pairing_state(pool: PgPool) -> AppState {
    let mut state = AppState::new(pool, pairing_test_config());
    state.jwt_verifier = Some(JwtVerifier::from_test_decoding_key(
        test_signing::TEST_KID,
        test_signing::verifier(),
        JwtVerifierConfig {
            jwks_url: "http://127.0.0.1:9/jwks".into(),
            issuer: TEST_JWT_ISSUER.into(),
            audience: TEST_JWT_AUDIENCE.into(),
        },
    ));
    state
}

fn token(sub: &str) -> String {
    test_signing::user_token(sub, TEST_JWT_ISSUER, TEST_JWT_AUDIENCE, 300)
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| json!({ "raw": String::from_utf8_lossy(&bytes) }))
}

async fn post_json(
    app: &axum::Router,
    uri: &str,
    auth: Option<&str>,
    body: Value,
) -> axum::response::Response {
    let mut builder = http::Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = auth {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    app.clone()
        .oneshot(
            builder
                .body(axum::body::Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn get_auth(app: &axum::Router, uri: &str, token: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            http::Request::builder()
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn start_pairing(app: &axum::Router, device_name: &str) -> (String, Value) {
    let installation_id = Uuid::new_v4().to_string();
    let response = post_json(
        app,
        "/v1/local-mac/pairings",
        None,
        json!({
            "deviceName": device_name,
            "installationId": installation_id
        }),
    )
    .await;
    assert_eq!(response.status(), http::StatusCode::OK, "start pairing");
    (installation_id, json_body(response).await)
}

#[tokio::test]
async fn mac_can_initiate_pairing_without_jwt_and_does_not_create_sandbox() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping mac_can_initiate_pairing_without_jwt_and_does_not_create_sandbox");
        return;
    };
    let app = build_router(pairing_state(pool.clone()));
    let (installation_id, body) = start_pairing(&app, "Drew's MacBook").await;
    assert!(body["pairingId"].as_str().unwrap().len() > 8);
    assert!(body["pairingSecret"].as_str().unwrap().len() >= 32);
    assert!(body["userCode"].as_str().unwrap().contains('-'));
    assert!(body["verificationUrl"]
        .as_str()
        .unwrap()
        .contains("/pair/mac?pairingId="));
    let nodes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM local_mac_nodes WHERE installation_id = $1",
    )
    .bind(&installation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(nodes, 0);
    let sandboxes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM sandboxes WHERE provider = 'local_mac' AND provider_resource_id = $1",
    )
    .bind(body["pairingId"].as_str().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(sandboxes, 0);
}

#[tokio::test]
async fn pairing_expires_and_wrong_secret_cannot_exchange() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping pairing_expires_and_wrong_secret_cannot_exchange");
        return;
    };
    let app = build_router(pairing_state(pool.clone()));
    let (_, started) = start_pairing(&app, "Expired Mac").await;
    let pairing_id = started["pairingId"].as_str().unwrap().to_string();
    let secret = started["pairingSecret"].as_str().unwrap().to_string();
    sqlx::query("UPDATE local_mac_pairing_sessions SET expires_at = NOW() - interval '1 second' WHERE id = $1")
        .bind(&pairing_id)
        .execute(&pool)
        .await
        .unwrap();

    let expired = post_json(
        &app,
        &format!("/v1/local-mac/pairings/{pairing_id}/exchange"),
        None,
        json!({ "pairingSecret": secret }),
    )
    .await;
    assert_eq!(expired.status(), http::StatusCode::CONFLICT);

    let (_, other) = start_pairing(&app, "Secret Mac").await;
    let bad = post_json(
        &app,
        &format!(
            "/v1/local-mac/pairings/{}/exchange",
            other["pairingId"].as_str().unwrap()
        ),
        None,
        json!({ "pairingSecret": "not-the-secret" }),
    )
    .await;
    assert_eq!(bad.status(), http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn wrong_user_code_and_internal_token_cannot_approve() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping wrong_user_code_and_internal_token_cannot_approve");
        return;
    };
    let app = build_router(pairing_state(pool));
    let (_, started) = start_pairing(&app, "Code Mac").await;
    let pairing_id = started["pairingId"].as_str().unwrap();
    let owner = token("owner-a");

    let wrong = post_json(
        &app,
        &format!("/v1/local-mac/pairings/{pairing_id}/approve"),
        Some(&owner),
        json!({ "userCode": "AAAA-AAAA" }),
    )
    .await;
    assert_eq!(wrong.status(), http::StatusCode::BAD_REQUEST);

    let internal = post_json(
        &app,
        &format!("/v1/local-mac/pairings/{pairing_id}/approve"),
        Some("test-token"),
        json!({ "userCode": started["userCode"] }),
    )
    .await;
    assert_eq!(internal.status(), http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn browser_approval_binds_owner_and_blocks_other_owners() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping browser_approval_binds_owner_and_blocks_other_owners");
        return;
    };
    let app = build_router(pairing_state(pool.clone()));
    let (_, started) = start_pairing(&app, "Bind Mac").await;
    let pairing_id = started["pairingId"].as_str().unwrap();
    let owner_a = format!("owner-a-{}", Uuid::new_v4());
    let owner_b = format!("owner-b-{}", Uuid::new_v4());

    let pending_exchange = post_json(
        &app,
        &format!("/v1/local-mac/pairings/{pairing_id}/exchange"),
        None,
        json!({ "pairingSecret": started["pairingSecret"] }),
    )
    .await;
    assert_eq!(pending_exchange.status(), http::StatusCode::ACCEPTED);
    assert_eq!(json_body(pending_exchange).await["status"], "pending");

    let approved = post_json(
        &app,
        &format!("/v1/local-mac/pairings/{pairing_id}/approve"),
        Some(&token(&owner_a)),
        json!({ "userCode": started["userCode"] }),
    )
    .await;
    assert_eq!(approved.status(), http::StatusCode::OK);
    let approved_body = json_body(approved).await;
    assert_eq!(approved_body["status"], "approved");
    assert!(approved_body.get("credential").is_none());
    assert!(approved_body.get("pairingSecret").is_none());

    let bound: Option<String> =
        sqlx::query_scalar("SELECT owner_id FROM local_mac_pairing_sessions WHERE id = $1")
            .bind(pairing_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(bound.as_deref(), Some(owner_a.as_str()));

    let other = post_json(
        &app,
        &format!("/v1/local-mac/pairings/{pairing_id}/approve"),
        Some(&token(&owner_b)),
        json!({ "userCode": started["userCode"] }),
    )
    .await;
    assert_eq!(other.status(), http::StatusCode::CONFLICT);
}

#[tokio::test]
async fn successful_exchange_issues_credential_once_and_creates_this_mac() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping successful_exchange_issues_credential_once_and_creates_this_mac");
        return;
    };
    let app = build_router(pairing_state(pool.clone()));
    let (installation_id, started) = start_pairing(&app, "Exchange Mac").await;
    let pairing_id = started["pairingId"].as_str().unwrap().to_string();
    let secret = started["pairingSecret"].as_str().unwrap().to_string();
    let owner = format!("owner-ex-{}", Uuid::new_v4());
    let approve = post_json(
        &app,
        &format!("/v1/local-mac/pairings/{pairing_id}/approve"),
        Some(&token(&owner)),
        json!({ "userCode": started["userCode"] }),
    )
    .await;
    assert_eq!(approve.status(), http::StatusCode::OK);

    let first = post_json(
        &app,
        &format!("/v1/local-mac/pairings/{pairing_id}/exchange"),
        None,
        json!({ "pairingSecret": secret }),
    )
    .await;
    assert_eq!(first.status(), http::StatusCode::OK);
    let issued = json_body(first).await;
    let credential = issued["credential"].as_str().unwrap().to_string();
    let computer_id = issued["computerId"].as_str().unwrap().to_string();
    let node_id = issued["nodeId"].as_str().unwrap().to_string();
    assert_eq!(issued["displayName"], "This Mac");
    assert!(credential.starts_with("emac_"));

    let stored_secret: Vec<u8> = sqlx::query_scalar(
        "SELECT pairing_secret_hash FROM local_mac_pairing_sessions WHERE id = $1",
    )
    .bind(&pairing_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_ne!(stored_secret, secret.as_bytes());
    assert!(!String::from_utf8_lossy(&stored_secret).contains(&secret));

    let stored_credential: Vec<u8> =
        sqlx::query_scalar("SELECT credential_hash FROM local_mac_nodes WHERE id = $1")
            .bind(&node_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_ne!(stored_credential, credential.as_bytes());
    assert!(!String::from_utf8_lossy(&stored_credential).contains(&credential));

    let sandbox: (String, String, String, String) = sqlx::query_as(
        "SELECT owner_id, provider, display_name, state FROM sandboxes WHERE id = $1",
    )
    .bind(&computer_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(sandbox.0, owner);
    assert_eq!(sandbox.1, "local_mac");
    assert_eq!(sandbox.2, "This Mac");
    assert_eq!(sandbox.3, "active");

    let listed = get_auth(&app, "/v1/computers", &token(&owner)).await;
    assert_eq!(listed.status(), http::StatusCode::OK);
    let computers = json_body(listed).await;
    let found = computers
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == computer_id)
        .expect("This Mac listed");
    assert_eq!(found["displayName"], "This Mac");
    assert_eq!(found["provider"], "local_mac");
    assert_eq!(found["state"], "active");
    assert_eq!(found["providerMetadata"]["provisioned"], true);

    let other_owner = format!("owner-other-{}", Uuid::new_v4());
    let hidden = get_auth(&app, "/v1/computers", &token(&other_owner)).await;
    let hidden_body = json_body(hidden).await;
    assert!(hidden_body
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["id"] != computer_id));

    let second = post_json(
        &app,
        &format!("/v1/local-mac/pairings/{pairing_id}/exchange"),
        None,
        json!({ "pairingSecret": secret }),
    )
    .await;
    assert_eq!(second.status(), http::StatusCode::CONFLICT);
    let second_body = json_body(second).await;
    assert!(second_body.get("credential").is_none());

    db::validate_node_credential(&pool, &PAIRING_TEST_KEY, &node_id, &credential)
        .await
        .expect("live credential");

    let revoke = post_json(
        &app,
        &format!("/v1/local-mac/nodes/{node_id}/revoke"),
        Some(&token(&other_owner)),
        json!({}),
    )
    .await;
    assert_eq!(revoke.status(), http::StatusCode::NOT_FOUND);

    let revoke_ok = post_json(
        &app,
        &format!("/v1/local-mac/nodes/{node_id}/revoke"),
        Some(&token(&owner)),
        json!({}),
    )
    .await;
    assert_eq!(revoke_ok.status(), http::StatusCode::NO_CONTENT);
    assert!(
        db::validate_node_credential(&pool, &PAIRING_TEST_KEY, &node_id, &credential)
            .await
            .is_err()
    );

    let after_revoke = get_auth(&app, "/v1/computers", &token(&owner)).await;
    let after_body = json_body(after_revoke).await;
    assert!(after_body
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["id"] != computer_id));

    let _ = installation_id;
}

#[tokio::test]
async fn repairing_same_installation_reuses_node_and_rotates_credential() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping repairing_same_installation_reuses_node_and_rotates_credential");
        return;
    };
    let app = build_router(pairing_state(pool.clone()));
    let owner = format!("owner-repair-{}", Uuid::new_v4());
    let installation_id = Uuid::new_v4().to_string();

    async fn pair_once(
        app: &axum::Router,
        owner: &str,
        installation_id: &str,
        device_name: &str,
    ) -> Value {
        let started = post_json(
            app,
            "/v1/local-mac/pairings",
            None,
            json!({
                "deviceName": device_name,
                "installationId": installation_id
            }),
        )
        .await;
        assert_eq!(started.status(), http::StatusCode::OK);
        let body = json_body(started).await;
        let pairing_id = body["pairingId"].as_str().unwrap();
        let approve = post_json(
            app,
            &format!("/v1/local-mac/pairings/{pairing_id}/approve"),
            Some(&token(owner)),
            json!({ "userCode": body["userCode"] }),
        )
        .await;
        assert_eq!(approve.status(), http::StatusCode::OK);
        let exchanged = post_json(
            app,
            &format!("/v1/local-mac/pairings/{pairing_id}/exchange"),
            None,
            json!({ "pairingSecret": body["pairingSecret"] }),
        )
        .await;
        assert_eq!(exchanged.status(), http::StatusCode::OK);
        json_body(exchanged).await
    }

    let first = pair_once(&app, &owner, &installation_id, "First name").await;
    let second = pair_once(&app, &owner, &installation_id, "Second name").await;
    assert_eq!(first["nodeId"], second["nodeId"]);
    assert_eq!(first["computerId"], second["computerId"]);
    assert_ne!(first["credential"], second["credential"]);
    assert!(db::validate_node_credential(
        &pool,
        &PAIRING_TEST_KEY,
        first["nodeId"].as_str().unwrap(),
        first["credential"].as_str().unwrap(),
    )
    .await
    .is_err());
    db::validate_node_credential(
        &pool,
        &PAIRING_TEST_KEY,
        second["nodeId"].as_str().unwrap(),
        second["credential"].as_str().unwrap(),
    )
    .await
    .expect("rotated credential");
}

#[tokio::test]
async fn sprite_computer_placeholder_still_uses_fly_sprite() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping sprite_computer_placeholder_still_uses_fly_sprite");
        return;
    };
    let owner = format!("sprite-{}", Uuid::new_v4());
    let row = cloud_host::db::resources::insert_computer_placeholder(&pool, &owner, "Cloud box")
        .await
        .unwrap();
    assert_eq!(row.provider, "fly_sprite");
    assert_eq!(row.state, "pending");
    assert_ne!(row.display_name, "This Mac");
}
