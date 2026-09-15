use cloud_host::{
    db::resources,
    group_router::{self, GroupRoutingMode, ValidatedRouteDecision},
    set_test_group_route_decider,
    drain_one_pending_route,
    groups::{self, SendGroupMessageRequest},
    run_engine_select::RunEngineMode,
    AppState, Config,
};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

async fn bot(pool: &PgPool, owner: &str, name: &str) -> resources::BotRow {
    let computer_id = resources::insert_computer_placeholder(pool, owner, "Computer")
        .await
        .unwrap()
        .id;
    resources::insert_bot(
        pool,
        owner,
        name,
        &format!("Role for {name}"),
        "gpt-5.6-luna",
        Some(&computer_id),
        "responses",
        "sky-wisp",
    )
    .await
    .unwrap()
}

fn test_config() -> Config {
    Config {
        database_url: std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere".into()),
        openai_api_key: Some("test-key".into()),
        sprite_token: "test-sprite".into(),
        api_token: "test-token".into(),
        auth_mode: cloud_host::config::AuthMode::InternalToken,
        jwt_issuer: None,
        jwt_audience: None,
        jwt_jwks_url: None,
        cors_web_origin: None,
        allow_codex_login: false,
        sprites_api_base: "http://127.0.0.1:9".into(),
        max_concurrent_runs: 2,
        run_timeout_secs: 120,
        bind_addr: "127.0.0.1:0".into(),
        run_engine: RunEngineMode::Responses,
        codex_executable: None,
        codex_profiles_dir: None,
        tool_approval_timeout_secs: 300,
        enforce_tool_approvals_internal: false,
        browser_enabled: false,
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn auto_send_admits_pending_without_runs(pool: PgPool) {
    let a = bot(&pool, "alice", "Researcher").await;
    let b = bot(&pool, "alice", "Designer").await;
    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Team".into(),
            bot_ids: vec![a.id.clone(), b.id.clone()],
        },
    )
    .await
    .unwrap();

    let send = groups::send_group_message(
        &pool,
        "alice",
        &group.id,
        &Uuid::new_v4().to_string(),
        SendGroupMessageRequest {
            body: "Can someone research this?".into(),
            recipient_bot_ids: None,
            mention_mode: None,
            routing_mode: Some("auto".into()),
        },
    )
    .await
    .unwrap();

    assert_eq!(
        send.message.routing.as_ref().map(|r| r.status.as_str()),
        Some("pending")
    );
    assert!(send.recipients.is_empty());
    let runs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs WHERE conversation_id = $1")
        .bind(&group.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(runs, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn router_selects_researcher(pool: PgPool) {
    let researcher = bot(&pool, "alice", "Researcher").await;
    let designer = bot(&pool, "alice", "Designer").await;
    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Team".into(),
            bot_ids: vec![researcher.id.clone(), designer.id.clone()],
        },
    )
    .await
    .unwrap();

    set_test_group_route_decider(Some(Arc::new(|input| {
        let picked = input
            .candidates
            .iter()
            .find(|c| c.name == "Researcher")
            .map(|c| c.bot_id.clone())
            .unwrap();
        Ok(ValidatedRouteDecision {
            mode: GroupRoutingMode::Specific,
            bot_ids: vec![picked],
            decision_code: Some("single_owner".into()),
        })
    })));

    let send = groups::send_group_message(
        &pool,
        "alice",
        &group.id,
        &Uuid::new_v4().to_string(),
        SendGroupMessageRequest {
            body: "Research competitors".into(),
            recipient_bot_ids: None,
            mention_mode: None,
            routing_mode: Some("auto".into()),
        },
    )
    .await
    .unwrap();

    let state = AppState::new(pool.clone(), test_config());
    assert!(drain_one_pending_route(&state).await.unwrap());

    set_test_group_route_decider(None);

    let recipients: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM group_message_recipients WHERE message_id = $1",
    )
    .bind(&send.message.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(recipients, 1);
    let kind: String = sqlx::query_scalar(
        "SELECT routing_kind FROM group_message_recipients WHERE message_id = $1",
    )
    .bind(&send.message.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(kind, "auto");
    let status: String = sqlx::query_scalar(
        "SELECT routing_status FROM group_message_sends WHERE message_id = $1",
    )
    .bind(&send.message.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "resolved");
}

#[sqlx::test(migrations = "./migrations")]
async fn explicit_mention_skips_router_and_resolves_immediately(pool: PgPool) {
    let researcher = bot(&pool, "alice", "Researcher").await;
    let designer = bot(&pool, "alice", "Designer").await;
    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Team".into(),
            bot_ids: vec![researcher.id.clone(), designer.id.clone()],
        },
    )
    .await
    .unwrap();

    let send = groups::send_group_message(
        &pool,
        "alice",
        &group.id,
        &Uuid::new_v4().to_string(),
        SendGroupMessageRequest {
            body: "@Researcher investigate".into(),
            recipient_bot_ids: Some(vec![researcher.id.clone()]),
            mention_mode: None,
            routing_mode: Some("specific".into()),
        },
    )
    .await
    .unwrap();

    assert_eq!(send.recipients.len(), 1);
    assert_eq!(
        send.message.routing.as_ref().map(|r| r.status.as_str()),
        Some("resolved")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn delete_pending_auto_message_cancels_route(pool: PgPool) {
    let a = bot(&pool, "alice", "A").await;
    let b = bot(&pool, "alice", "B").await;
    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Team".into(),
            bot_ids: vec![a.id.clone(), b.id.clone()],
        },
    )
    .await
    .unwrap();
    let send = groups::send_group_message(
        &pool,
        "alice",
        &group.id,
        &Uuid::new_v4().to_string(),
        SendGroupMessageRequest {
            body: "hello".into(),
            recipient_bot_ids: None,
            mention_mode: None,
            routing_mode: Some("auto".into()),
        },
    )
    .await
    .unwrap();

    let state = AppState::new(pool.clone(), test_config());
    groups::delete_transcript_message(&state, "alice", &group.id, &send.message.id)
        .await
        .unwrap();

    let status: Option<String> = sqlx::query_scalar(
        "SELECT routing_status FROM group_message_sends WHERE message_id = $1",
    )
    .bind(&send.message.id)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert_eq!(status.as_deref(), Some("cancelled"));
}
