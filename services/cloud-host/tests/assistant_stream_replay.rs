use cloud_host::auth::{JwtVerifier, JwtVerifierConfig};
use cloud_host::config::{AuthMode, Config};
use cloud_host::db::resources::{insert_bot, insert_computer_placeholder};
use cloud_host::{build_router, test_signing, AppState};
use serde_json::json;
use sqlx::PgPool;
use std::time::Duration;
use tower::ServiceExt;
use uuid::Uuid;

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

fn jwt_state(pool: PgPool) -> AppState {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere".into());
    let config = Config {
        database_url,
        openai_api_key: Some("test-key".into()),
        sprite_token: "test-sprite".into(),
        api_token: "test-token".into(),
        auth_mode: AuthMode::Jwt,
        jwt_issuer: Some(TEST_JWT_ISSUER.into()),
        jwt_audience: Some(TEST_JWT_AUDIENCE.into()),
        jwt_jwks_url: Some("http://127.0.0.1:9/jwks".into()),
        cors_web_origin: None,
        allow_codex_login: false,
        sprites_api_base: "http://127.0.0.1:9".into(),
        max_concurrent_runs: 2,
        run_timeout_secs: 120,
        bind_addr: "127.0.0.1:0".into(),
        run_engine: cloud_host::run_engine_select::RunEngineMode::Responses,
        codex_executable: None,
        codex_profiles_dir: None,
        tool_approval_timeout_secs: 300,
        enforce_tool_approvals_internal: false,
        legacy_local_approval_bypass: false,
        browser_enabled: false,
        connector_secret_key: None,
        github_client_id: None,
        github_client_secret: None,
        github_oauth_redirect_uri: None,
    };
    let mut state = AppState::new(pool, config);
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

fn apply_delta(answer: &mut String, commentary: &mut String, payload: &serde_json::Value) {
    let phase = payload.get("phase").and_then(|v| v.as_str()).unwrap_or("answer");
    let delta = payload.get("delta").and_then(|v| v.as_str()).unwrap_or("");
    if delta.is_empty() {
        return;
    }
    if phase == "commentary" {
        commentary.push_str(delta);
    } else {
        answer.push_str(delta);
    }
}

async fn collect_sse_events(
    app: &axum::Router,
    run_id: &str,
    owner_token: &str,
    last_event_id: Option<i64>,
) -> Vec<(Option<String>, String, String)> {
    let mut request = axum::http::Request::builder()
        .uri(format!("/v1/runs/{run_id}/events"))
        .header("Authorization", format!("Bearer {}", owner_token));
    if let Some(id) = last_event_id {
        request = request.header("Last-Event-ID", id.to_string());
    }
    let response = app
        .clone()
        .oneshot(request.body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let bytes = tokio::time::timeout(
        Duration::from_secs(2),
        axum::body::to_bytes(response.into_body(), 2 * 1024 * 1024),
    )
    .await
    .ok()
    .and_then(|result| result.ok())
    .unwrap_or_default();
    let buffer = String::from_utf8_lossy(&bytes);
    let mut events = Vec::new();
    for block in buffer.split("\n\n") {
        if block.trim().is_empty() {
            continue;
        }
        let mut event_id = None;
        let mut event_type = "message".to_string();
        let mut data = String::new();
        for line in block.lines() {
            if let Some(id) = line.strip_prefix("id: ") {
                event_id = Some(id.trim().to_string());
            } else if let Some(name) = line.strip_prefix("event: ") {
                event_type = name.trim().to_string();
            } else if let Some(body) = line.strip_prefix("data: ") {
                data = body.to_string();
            }
        }
        if !data.is_empty() {
            events.push((event_id, event_type, data));
        }
    }
    events
}

#[tokio::test]
async fn assistant_delta_sse_replay_reconstructs_answer_without_gaps() {
    let Some(pool) = try_test_pool().await else {
        eprintln!("skipping assistant_delta_sse_replay_reconstructs_answer_without_gaps");
        return;
    };
    let owner = format!("owner-{}", Uuid::new_v4());
    let computer = insert_computer_placeholder(&pool, &owner, "SSE")
        .await
        .unwrap();
    let bot = insert_bot(
        &pool,
        &owner,
        "Bot",
        "",
        "gpt-5.6-luna",
        Some(&computer.id),
        "auto",
        "sky-wisp",
    )
    .await
    .unwrap();

    let request_id = format!("req-{}", Uuid::new_v4());
    let run_id = format!("run-{}", Uuid::new_v4());
    let conversation_id = format!("conv-{}", Uuid::new_v4());

    sqlx::query(
        r#"
        INSERT INTO conversations (id, bot_id, owner_id, created_at, updated_at)
        VALUES ($1, $2, $3, NOW(), NOW())
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(&conversation_id)
    .bind(&bot.id)
    .bind(&owner)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO agent_runs (
            id, request_id, bot_id, conversation_id, computer_id, model, status, step_count, owner_id, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, 'gpt-5.6-luna', 'succeeded', 0, $6, NOW(), NOW())
        "#,
    )
    .bind(&run_id)
    .bind(&request_id)
    .bind(&bot.id)
    .bind(&conversation_id)
    .bind(&computer.id)
    .bind(&owner)
    .execute(&pool)
    .await
    .unwrap();

    let item_id = "item-answer-1";
    let deltas = ["Hel", "lo ", "wor", "ld"];
    for (index, delta) in deltas.iter().enumerate() {
        let end = (index + 1) * delta.len();
        let payload = json!({
            "itemId": item_id,
            "phase": "answer",
            "delta": delta,
            "startOffset": end - delta.len(),
            "endOffset": end,
        });
        sqlx::query(
            "INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, 'assistant_delta', $2)",
        )
        .bind(&request_id)
        .bind(payload)
        .execute(&pool)
        .await
        .unwrap();
    }

    let commentary_payload = json!({
        "itemId": "item-commentary-1",
        "phase": "commentary",
        "delta": "thinking",
        "startOffset": 0,
        "endOffset": 8,
    });
    sqlx::query(
        "INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, 'assistant_delta', $2)",
    )
    .bind(&request_id)
    .bind(commentary_payload)
    .execute(&pool)
    .await
    .unwrap();

    let full_answer = "Hello world";
    sqlx::query(
        "INSERT INTO run_events (request_id, event_type, payload_json) VALUES ($1, 'terminal', $2)",
    )
    .bind(&request_id)
    .bind(json!({ "status": "succeeded", "fullContent": full_answer }))
    .execute(&pool)
    .await
    .unwrap();

    let app = build_router(jwt_state(pool.clone()));
    let owner_token = token(&owner);

    let first_batch = collect_sse_events(&app, &run_id, &owner_token, None).await;
    let first_delta_ids: Vec<i64> = first_batch
        .iter()
        .filter(|(_, kind, _)| kind == "assistant_delta")
        .filter_map(|(id, _, _)| id.as_ref()?.parse().ok())
        .collect();
    assert!(first_delta_ids.len() >= 4);

    let reconnect_from = first_delta_ids[1];
    let second_batch = collect_sse_events(&app, &run_id, &owner_token, Some(reconnect_from)).await;

    let mut answer = String::new();
    let mut commentary = String::new();
    for (_, _kind, data) in first_batch
        .iter()
        .filter(|(_, kind, _)| kind == "assistant_delta")
    {
        let payload: serde_json::Value = serde_json::from_str(data).unwrap();
        apply_delta(&mut answer, &mut commentary, &payload);
    }

    assert_eq!(commentary, "thinking");
    assert_eq!(answer, full_answer);

    for (event_id, kind, _) in &second_batch {
        if kind != "assistant_delta" {
            continue;
        }
        let id = event_id
            .as_ref()
            .and_then(|value| value.parse::<i64>().ok())
            .expect("assistant_delta missing durable id");
        assert!(id > reconnect_from, "catch-up replayed an old event id");
    }

    let catchup_has_terminal = second_batch
        .iter()
        .any(|(_, kind, _)| kind == "terminal");
    assert!(catchup_has_terminal);

    let terminal_payload: serde_json::Value = second_batch
        .iter()
        .find(|(_, kind, _)| kind == "terminal")
        .map(|(_, _, data)| serde_json::from_str(data).unwrap())
        .expect("terminal event in catch-up");
    assert_eq!(
        terminal_payload.get("fullContent").and_then(|v| v.as_str()),
        Some(full_answer)
    );
}
