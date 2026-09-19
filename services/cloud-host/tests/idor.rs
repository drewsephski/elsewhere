use cloud_host::auth::{JwtVerifier, JwtVerifierConfig};
use cloud_host::config::{AuthMode, Config};
use cloud_host::db::resources::insert_computer_placeholder;
use cloud_host::{build_router, test_signing, AppState};
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
        browser_profiles_dir: None,
        tool_approval_timeout_secs: 300,
        enforce_tool_approvals_internal: false,
        legacy_local_approval_bypass: false,
        browser_enabled: false,
        connector_secret_key: None,
        github_client_id: None,
        github_client_secret: None,
        github_oauth_redirect_uri: None,
        github_app_slug: None,
        slack_client_id: None,
        slack_client_secret: None,
        slack_signing_secret: None,
        slack_oauth_redirect_uri: None,
        slack_api_base: "https://slack.com/api".into(),
        local_mac_credential_key: None,
        allow_skill_draft_heuristic: false,
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

#[tokio::test]
async fn user_cannot_read_other_users_bot() {
    let Some(pool) = try_test_pool().await else {
        return;
    };
    let computer = insert_computer_placeholder(&pool, "user-a", "A computer")
        .await
        .unwrap();
    let bot = cloud_host::db::resources::insert_bot(
        &pool,
        "user-a",
        "Bot A",
        "instructions",
        "gpt-5.6-luna",
        Some(computer.id.as_str()),
        "auto",
        "sky-wisp",
    )
    .await
    .unwrap();

    let app = build_router(jwt_state(pool));
    let resp = app
        .oneshot(
            http::Request::builder()
                .uri(format!("/v1/bots/{}", bot.id))
                .header("authorization", format!("Bearer {}", token("user-b")))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn user_cannot_delete_other_users_bot() {
    let Some(pool) = try_test_pool().await else {
        return;
    };
    let bot = cloud_host::db::resources::insert_bot(
        &pool,
        "user-a",
        "Bot A",
        "i",
        "gpt-5.6-luna",
        None,
        "auto",
        "sky-wisp",
    )
    .await
    .unwrap();
    let app = build_router(jwt_state(pool));
    let resp = app
        .oneshot(
            http::Request::builder()
                .method("DELETE")
                .uri(format!("/v1/bots/{}", bot.id))
                .header("authorization", format!("Bearer {}", token("user-b")))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn user_can_delete_own_bot_with_conversations() {
    let Some(pool) = try_test_pool().await else {
        return;
    };
    let bot = cloud_host::db::resources::insert_bot(
        &pool,
        "user-a",
        "Bot A",
        "i",
        "gpt-5.6-luna",
        None,
        "auto",
        "sky-wisp",
    )
    .await
    .unwrap();
    let conv_id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO conversations (id, owner_id, bot_id) VALUES ($1, 'user-a', $2)")
        .bind(&conv_id)
        .bind(&bot.id)
        .execute(&pool)
        .await
        .unwrap();

    let app = build_router(jwt_state(pool.clone()));
    let resp = app
        .oneshot(
            http::Request::builder()
                .method("DELETE")
                .uri(format!("/v1/bots/{}", bot.id))
                .header("authorization", format!("Bearer {}", token("user-a")))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::NO_CONTENT);

    let still_there: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM bots WHERE id = $1)")
        .bind(&bot.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!still_there);
    let conv_left: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM conversations WHERE id = $1)")
            .bind(&conv_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!conv_left);
}

#[tokio::test]
async fn user_can_delete_own_bot_that_is_in_a_group() {
    let Some(pool) = try_test_pool().await else {
        return;
    };
    let computer = insert_computer_placeholder(&pool, "user-a", "Computer A")
        .await
        .unwrap();
    let designer = cloud_host::db::resources::insert_bot(
        &pool,
        "user-a",
        "Designer",
        "i",
        "gpt-5.6-luna",
        Some(&computer.id),
        "auto",
        "sky-wisp",
    )
    .await
    .unwrap();
    let researcher = cloud_host::db::resources::insert_bot(
        &pool,
        "user-a",
        "Researcher",
        "i",
        "gpt-5.6-luna",
        Some(&computer.id),
        "auto",
        "sky-wisp",
    )
    .await
    .unwrap();
    let group = cloud_host::groups::create_group(
        &pool,
        "user-a",
        cloud_host::groups::CreateGroupRequest {
            name: "Team 1".into(),
            bot_ids: vec![designer.id.clone(), researcher.id.clone()],
        },
    )
    .await
    .unwrap();

    let app = build_router(jwt_state(pool.clone()));
    let resp = app
        .oneshot(
            http::Request::builder()
                .method("DELETE")
                .uri(format!("/v1/bots/{}", designer.id))
                .header("authorization", format!("Bearer {}", token("user-a")))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::NO_CONTENT);

    let still_there: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM bots WHERE id = $1)")
        .bind(&designer.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!still_there);
    let group_left: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM conversations WHERE id = $1)")
            .bind(&group.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(group_left);
}

#[tokio::test]
async fn user_cannot_rename_or_delete_other_users_group() {
    let Some(pool) = try_test_pool().await else {
        return;
    };
    let computer = insert_computer_placeholder(&pool, "user-a", "Computer A")
        .await
        .unwrap();
    let a = cloud_host::db::resources::insert_bot(
        &pool,
        "user-a",
        "A",
        "i",
        "gpt-5.6-luna",
        Some(&computer.id),
        "auto",
        "sky-wisp",
    )
    .await
    .unwrap();
    let b = cloud_host::db::resources::insert_bot(
        &pool,
        "user-a",
        "B",
        "i",
        "gpt-5.6-luna",
        Some(&computer.id),
        "auto",
        "sky-wisp",
    )
    .await
    .unwrap();
    let group = cloud_host::groups::create_group(
        &pool,
        "user-a",
        cloud_host::groups::CreateGroupRequest {
            name: "Launch".into(),
            bot_ids: vec![a.id.clone(), b.id.clone()],
        },
    )
    .await
    .unwrap();

    let app = build_router(jwt_state(pool.clone()));
    let rename = app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("PATCH")
                .uri(format!("/v1/conversations/{}", group.id))
                .header("authorization", format!("Bearer {}", token("user-b")))
                .header("content-type", "application/json")
                .body(axum::body::Body::from(r#"{"name":"Stolen"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rename.status(), http::StatusCode::NOT_FOUND);

    let delete = app
        .oneshot(
            http::Request::builder()
                .method("DELETE")
                .uri(format!("/v1/conversations/{}", group.id))
                .header("authorization", format!("Bearer {}", token("user-b")))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete.status(), http::StatusCode::NOT_FOUND);

    let still_there: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM conversations WHERE id = $1)")
            .bind(&group.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(still_there);
}

#[tokio::test]
async fn user_cannot_stream_other_users_run_events() {
    let Some(pool) = try_test_pool().await else {
        return;
    };
    let run_id = Uuid::new_v4().to_string();
    let request_id = Uuid::new_v4().to_string();
    let bot_id = Uuid::new_v4().to_string();
    let conv_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO bots (id, owner_id, name, system_prompt, model, engine_preference) VALUES ($1, 'user-a', 'a', '', 'gpt-5.6-luna', 'auto')",
    )
    .bind(&bot_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO conversations (id, owner_id, bot_id) VALUES ($1, 'user-a', $2)")
        .bind(&conv_id)
        .bind(&bot_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        r#"
        INSERT INTO agent_runs (
            id, owner_id, request_id, bot_id, conversation_id, model, status, step_count, created_at, updated_at
        ) VALUES ($1, 'user-a', $2, $3, $4, 'gpt-5.6-luna', 'complete', 0, NOW(), NOW())
        "#,
    )
    .bind(&run_id)
    .bind(&request_id)
    .bind(&bot_id)
    .bind(&conv_id)
    .execute(&pool)
    .await
    .unwrap();

    let app = build_router(jwt_state(pool));
    let resp = app
        .oneshot(
            http::Request::builder()
                .uri(format!("/v1/runs/{run_id}/events"))
                .header("authorization", format!("Bearer {}", token("user-b")))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn provider_status_requires_auth_and_serializes() {
    let Some(pool) = try_test_pool().await else {
        return;
    };
    let app = build_router(jwt_state(pool.clone()));
    let anon = app
        .clone()
        .oneshot(
            http::Request::builder()
                .uri("/v1/providers/status")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(anon.status(), http::StatusCode::UNAUTHORIZED);

    let ok = app
        .oneshot(
            http::Request::builder()
                .uri("/v1/providers/status")
                .header("authorization", format!("Bearer {}", token("user-a")))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ok.status(), http::StatusCode::OK);
    let body = axum::body::to_bytes(ok.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.get("codexInstalled").is_some());
    assert!(json.get("defaultModel").is_some());
    assert!(json.get("chatgptConnected").is_some());
}

#[tokio::test]
async fn provider_profiles_are_persistent_private_and_owner_scoped() {
    let pool = try_test_pool()
        .await
        .expect("test Postgres must be running");
    let mut config = (*jwt_state(pool.clone()).config).clone();
    let root = std::env::temp_dir().join(format!("elsewhere-profiles-test-{}", Uuid::new_v4()));
    config.codex_profiles_dir = Some(root.clone());
    let owner_a = format!("../user-a-{}", Uuid::new_v4());
    let owner_b = format!("user-b-{}", Uuid::new_v4());
    let a = cloud_host::provider_profile::profile_for_owner(&pool, &config, &owner_a)
        .await
        .unwrap()
        .unwrap();
    let again = cloud_host::provider_profile::profile_for_owner(&pool, &config, &owner_a)
        .await
        .unwrap()
        .unwrap();
    let b = cloud_host::provider_profile::profile_for_owner(&pool, &config, &owner_b)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(a, again);
    assert_ne!(a, b);
    assert_eq!(a.parent(), Some(root.as_path()));
    assert!(Uuid::parse_str(a.file_name().unwrap().to_str().unwrap()).is_ok());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&a).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    config.codex_profiles_dir = None;
    assert!(
        cloud_host::provider_profile::profile_for_owner(&pool, &config, &owner_a)
            .await
            .is_err()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn user_cannot_cancel_another_users_chatgpt_login() {
    use cloud_host::app_state::PendingCodexLogin;
    use cloud_host::codex_ops::CodexOperationKind;
    let pool = try_test_pool()
        .await
        .expect("test Postgres must be running");
    let mut state = jwt_state(pool);
    let mut config = (*state.config).clone();
    config.allow_codex_login = true;
    state.config = std::sync::Arc::new(config);
    let client = codex_provider::CodexAppServerClient::from_process(
        codex_provider::spawn_fake_app_server().await.unwrap(),
    )
    .await
    .unwrap();
    let codex_permit = state
        .codex_ops
        .try_acquire(CodexOperationKind::Login)
        .expect("test login permit");
    *state.codex_login_client.lock().await = Some(PendingCodexLogin {
        owner_id: "user-a".into(),
        login_id: "private-login".into(),
        auth_url: "https://auth.openai.com/codex/device".into(),
        user_code: "secret-code".into(),
        expires_at: std::time::Instant::now() + std::time::Duration::from_secs(600),
        client: std::sync::Arc::new(client),
        codex_permit,
    });
    let response = build_router(state.clone())
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri("/v1/providers/codex/login/cancel")
                .header("authorization", format!("Bearer {}", token("user-b")))
                .header("content-type", "application/json")
                .body(axum::body::Body::from(r#"{"loginId":"private-login"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
    assert!(state.codex_login_client.lock().await.is_some());
}

#[tokio::test]
async fn jwt_cannot_impersonate_trusted_local_runner() {
    let pool = try_test_pool()
        .await
        .expect("test Postgres must be running");
    let response = build_router(jwt_state(pool))
        .oneshot(
            http::Request::builder()
                .uri("/v1/providers/status")
                .header("authorization", format!("Bearer {}", token("legacy-local")))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn progress_replay_drains_every_page_and_respects_cursor() {
    let pool = try_test_pool()
        .await
        .expect("test Postgres must be running");
    let owner = format!("replay-{}", Uuid::new_v4());
    let computer = insert_computer_placeholder(&pool, &owner, "Computer")
        .await
        .unwrap();
    let bot = cloud_host::db::resources::insert_bot(
        &pool,
        &owner,
        "Scout",
        "",
        "gpt-5.6-luna",
        Some(&computer.id),
        "codex",
        "sky-wisp",
    )
    .await
    .unwrap();
    let run = cloud_host::work::enqueue(
        &pool,
        &owner,
        &Uuid::new_v4().to_string(),
        &bot.id,
        None,
        "Summarize",
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO run_events (request_id,event_type,payload_json) SELECT $1, 'progress', jsonb_build_object('step', n) FROM generate_series(1,1001) n")
        .bind(&run.request_id).execute(&pool).await.unwrap();
    cloud_host::work::request_cancel(&pool, &owner, &run.run_id)
        .await
        .unwrap();
    let cursor: i64 = sqlx::query_scalar("SELECT MIN(id) FROM run_events WHERE request_id=$1")
        .bind(&run.request_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let response = build_router(jwt_state(pool))
        .oneshot(
            http::Request::builder()
                .uri(format!("/v1/runs/{}/events", run.run_id))
                .header("authorization", format!("Bearer {}", token(&owner)))
                .header("last-event-id", cursor.to_string())
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert_eq!(body.matches("event: progress").count(), 1001);
    assert!(!body.contains("event: queued"));
    assert!(body.contains("event: terminal"));
}

#[sqlx::test(migrations = "./migrations")]
async fn result_downloads_and_context_are_owner_scoped(pool: PgPool) {
    let computer = insert_computer_placeholder(&pool, "alice", "Computer")
        .await
        .unwrap();
    let bot = cloud_host::db::resources::insert_bot(
        &pool,
        "alice",
        "Scout",
        "Instructions",
        "gpt-5.6-luna",
        Some(&computer.id),
        "codex",
        "sky-wisp",
    )
    .await
    .unwrap();
    let run = cloud_host::work::enqueue(&pool, "alice", "private-result", &bot.id, None, "Work")
        .await
        .unwrap();
    cloud_host::results::save(
        &pool,
        &run.run_id,
        "report.html",
        "file",
        b"<script>secret</script>",
    )
    .await
    .unwrap();
    let id: Uuid = sqlx::query_scalar("SELECT id FROM work_results WHERE run_id = $1")
        .bind(&run.run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let app = build_router(jwt_state(pool));
    for path in [
        format!("/v1/results/{id}/download"),
        format!("/v1/runs/{}/results", run.run_id),
        format!("/v1/bots/{}/context", bot.id),
    ] {
        let response = app
            .clone()
            .oneshot(
                http::Request::builder()
                    .uri(path)
                    .header("authorization", format!("Bearer {}", token("bob")))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
    }
    let response = app
        .oneshot(
            http::Request::builder()
                .uri(format!("/v1/results/{id}/download"))
                .header("authorization", format!("Bearer {}", token("alice")))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(
        response.headers()["content-type"],
        "application/octet-stream"
    );
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert!(response.headers()["content-disposition"]
        .to_str()
        .unwrap()
        .starts_with("attachment;"));
    let body = axum::body::to_bytes(response.into_body(), 100)
        .await
        .unwrap();
    assert_eq!(&body[..], b"<script>secret</script>");
}

#[sqlx::test(migrations = "./migrations")]
async fn workspace_presence_tracks_real_work_and_is_private(pool: PgPool) {
    let computer = insert_computer_placeholder(&pool, "alice", "Research computer")
        .await
        .unwrap();
    let bot = cloud_host::db::resources::insert_bot(
        &pool,
        "alice",
        "Scout",
        "Instructions",
        "gpt-5.6-luna",
        Some(&computer.id),
        "codex",
        "sky-wisp",
    )
    .await
    .unwrap();
    let app = build_router(jwt_state(pool.clone()));
    async fn overview(app: axum::Router, owner: &str) -> serde_json::Value {
        let response = app
            .oneshot(
                http::Request::builder()
                    .uri("/v1/workspace")
                    .header("authorization", format!("Bearer {}", token(owner)))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), http::StatusCode::OK);
        serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 100000)
                .await
                .unwrap(),
        )
        .unwrap()
    }
    assert_eq!(
        overview(app.clone(), "alice").await["bots"][0]["presence"],
        "ready"
    );
    let run =
        cloud_host::work::enqueue(&pool, "alice", "presence", &bot.id, None, "Research brief")
            .await
            .unwrap();
    assert_eq!(
        overview(app.clone(), "alice").await["bots"][0]["presence"],
        "queued"
    );
    cloud_host::work::claim_next(&pool).await.unwrap();
    assert_eq!(
        overview(app.clone(), "alice").await["bots"][0]["presence"],
        "working"
    );
    sqlx::query(
        r#"
        INSERT INTO run_user_questions (
            id, owner_id, run_id, request_id, tool_invocation_id, question, options, status, requested_at, updated_at
        ) VALUES ('presence-question','alice',$1,'presence','inv-q','Pick one?','["A","B"]'::jsonb,'pending',NOW(),NOW())
        "#,
    )
    .bind(&run.run_id)
    .execute(&pool)
    .await
    .unwrap();
    let question_waiting = overview(app.clone(), "alice").await;
    assert_eq!(question_waiting["bots"][0]["presence"], "waiting_approval");
    assert_eq!(question_waiting["counts"]["approvals"], 1);
    sqlx::query("DELETE FROM run_user_questions WHERE id = 'presence-question'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        r#"
        INSERT INTO human_intervention_requests (
            id, run_id, owner_id, computer_id, reason, message, status, requested_at, created_at, updated_at
        ) VALUES ('presence-intervention',$1,'alice',$2,'login','Sign in to continue','pending',NOW(),NOW(),NOW())
        "#,
    )
    .bind(&run.run_id)
    .bind(&computer.id)
    .execute(&pool)
    .await
    .unwrap();
    let intervention_waiting = overview(app.clone(), "alice").await;
    assert_eq!(
        intervention_waiting["bots"][0]["presence"],
        "waiting_approval"
    );
    assert_eq!(intervention_waiting["counts"]["approvals"], 1);
    sqlx::query("DELETE FROM human_intervention_requests WHERE id = 'presence-intervention'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tool_approval_requests (id, run_id, owner_id, tool_name, tool_kind, expires_at) VALUES ('presence-approval',$1,'alice','workspace_write','write',NOW()+INTERVAL '5 minutes')")
        .bind(&run.run_id).execute(&pool).await.unwrap();
    let waiting = overview(app.clone(), "alice").await;
    assert_eq!(waiting["bots"][0]["presence"], "waiting_approval");
    assert_eq!(waiting["counts"]["approvals"], 1);
    let foreign = overview(app.clone(), "bob").await;
    assert_eq!(foreign["bots"].as_array().unwrap().len(), 0);
    assert_eq!(foreign["counts"]["working"], 0);
    assert_eq!(foreign["counts"]["approvals"], 0);
    sqlx::query("UPDATE tool_approval_requests SET status='approved' WHERE id='presence-approval'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE agent_runs SET status='completed' WHERE id=$1")
        .bind(&run.run_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        overview(app.clone(), "alice").await["bots"][0]["presence"],
        "saving_results"
    );
    sqlx::query("UPDATE agent_runs SET execution_released_at=NOW() WHERE id=$1")
        .bind(&run.run_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        overview(app.clone(), "alice").await["bots"][0]["presence"],
        "ready"
    );
    sqlx::query("UPDATE sandboxes SET state='archived' WHERE id=$1")
        .bind(&computer.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        overview(app, "alice").await["bots"][0]["presence"],
        "needs_computer"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_bot_setting_edits_preserve_unrelated_fields(pool: PgPool) {
    use cloud_host::db::resources::{insert_bot, patch_bot};
    let computer = insert_computer_placeholder(&pool, "alice", "Computer")
        .await
        .unwrap();
    let bot = insert_bot(
        &pool,
        "alice",
        "Original",
        "Original",
        "gpt-5.6-luna",
        Some(&computer.id),
        "codex",
        "sky-wisp",
    )
    .await
    .unwrap();
    let (name, instructions) = tokio::join!(
        patch_bot(
            &pool,
            "alice",
            &bot.id,
            Some("Renamed"),
            None,
            None,
            None,
            None,
            None,
            None
        ),
        patch_bot(
            &pool,
            "alice",
            &bot.id,
            None,
            Some("Changed role"),
            None,
            None,
            None,
            None,
            None
        )
    );
    name.unwrap();
    instructions.unwrap();
    let updated = cloud_host::db::resources::get_bot_for_owner(&pool, "alice", &bot.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.name, "Renamed");
    assert_eq!(updated.system_prompt, "Changed role");
    assert_eq!(updated.computer_id.as_deref(), Some(computer.id.as_str()));
}

#[sqlx::test(migrations = "./migrations")]
async fn readiness_requires_a_recent_dispatcher_heartbeat(pool: PgPool) {
    let state = jwt_state(pool);
    let app = build_router(state.clone());
    async fn ready(app: axum::Router) -> http::StatusCode {
        app.oneshot(
            http::Request::builder()
                .uri("/ready")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
    }
    assert_eq!(
        ready(app.clone()).await,
        http::StatusCode::SERVICE_UNAVAILABLE
    );
    state
        .dispatcher_alive
        .store(true, std::sync::atomic::Ordering::SeqCst);
    *state.runner_heartbeat.lock().unwrap() = Some(std::time::Instant::now());
    assert_eq!(ready(app.clone()).await, http::StatusCode::OK);
    *state.runner_heartbeat.lock().unwrap() =
        Some(std::time::Instant::now() - std::time::Duration::from_secs(11));
    assert_eq!(ready(app).await, http::StatusCode::SERVICE_UNAVAILABLE);
}
