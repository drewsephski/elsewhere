use agent_core::{
    dispatch_agent_tool_with_gate_and_recovery, AllowAllApprovalGate, CollaborationContext,
    ToolRunContext,
};
use cloud_host::auth::{JwtVerifier, JwtVerifierConfig};
use cloud_host::config::AuthMode;
use cloud_host::db::resources::{insert_bot, insert_computer_placeholder};
use cloud_host::memory::db::{insert_memory, list_memories, NewMemory};
use cloud_host::memory::extraction::{
    apply_extraction, enqueue_if_eligible, is_extraction_eligible, recover_stale_jobs,
    ExtractionChange, ExtractionModelResponse, ExtractionRequest,
};
use cloud_host::memory::secrets::looks_like_secret;
use cloud_host::memory::service::PostgresAgentMemory;
use cloud_host::memory::types::{MemoryKind, MemorySourceKind};
use cloud_host::memory::{tick_extraction, MemoryFilters};
use cloud_host::work;
use cloud_host::{build_router, test_signing, AppState, Config};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

const TEST_JWT_ISSUER: &str = "http://localhost:3000";
const TEST_JWT_AUDIENCE: &str = "elsewhere-cloud-host";

fn jwt_state(pool: PgPool) -> AppState {
    let config = Config {
        database_url: std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://elsewhere:elsewhere@127.0.0.1:5432/elsewhere".into()),
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
        enforce_tool_approvals_internal: true,
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

async fn json_body(response: axum::http::Response<axum::body::Body>) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!({}))
}

async fn auth_json(
    app: &axum::Router,
    method: &str,
    uri: &str,
    owner: &str,
    body: Option<Value>,
) -> (axum::http::StatusCode, Value) {
    let mut builder = axum::http::Request::builder()
        .method(method)
        .uri(uri)
        .header("Authorization", format!("Bearer {}", token(owner)));
    let request_body = if let Some(body) = body {
        builder = builder.header("content-type", "application/json");
        axum::body::Body::from(body.to_string())
    } else {
        axum::body::Body::empty()
    };
    let response = app
        .clone()
        .oneshot(builder.body(request_body).unwrap())
        .await
        .unwrap();
    (response.status(), json_body(response).await)
}

async fn seed_bot(pool: &PgPool, owner: &str) -> (String, String) {
    let computer = insert_computer_placeholder(pool, owner, "c").await.unwrap();
    let bot = insert_bot(
        pool,
        owner,
        "Scout",
        "Help carefully",
        "gpt-5.6-luna",
        Some(computer.id.as_str()),
        "responses",
        "sky-wisp",
    )
    .await
    .unwrap();
    (bot.id, computer.id)
}

async fn insert_fact(
    pool: &PgPool,
    owner: &str,
    bot_id: &str,
    content: &str,
    kind: MemoryKind,
) -> String {
    let mut tx = pool.begin().await.unwrap();
    let record = insert_memory(
        &mut tx,
        NewMemory {
            owner_id: owner.into(),
            bot_id: bot_id.into(),
            kind,
            content: content.into(),
            search_terms: vec!["pnpm".into(), "package".into(), "elsewhere".into()],
            importance: 4,
            confidence: 0.9,
            source_kind: MemorySourceKind::Manual,
            source_run_id: None,
            source_message_id: None,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    record.id
}

#[sqlx::test(migrations = "./migrations")]
async fn pinned_context_survives_and_is_always_included(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    sqlx::query("INSERT INTO bot_context (bot_id, content) VALUES ($1, $2)")
        .bind(&bot_id)
        .bind("Always use the research computer.")
        .execute(&pool)
        .await
        .unwrap();
    let _ = insert_fact(
        &pool,
        &owner,
        &bot_id,
        "Drew uses pnpm for Elsewhere.",
        MemoryKind::Preference,
    )
    .await;
    let records = work::enqueue(
        &pool,
        &owner,
        &Uuid::new_v4().to_string(),
        &bot_id,
        None,
        "Please use the right package manager.",
    )
    .await
    .unwrap();
    assert!(records
        .instructions
        .contains("Pinned context from your owner"));
    assert!(records
        .instructions
        .contains("Always use the research computer."));
    assert!(records.instructions.contains("Relevant remembered context"));
    assert!(records
        .instructions
        .contains("Drew uses pnpm for Elsewhere."));
    let context: String = sqlx::query_scalar("SELECT content FROM bot_context WHERE bot_id = $1")
        .bind(&bot_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(context, "Always use the research computer.");
}

#[sqlx::test(migrations = "./migrations")]
async fn owner_and_bot_isolation_and_foreign_ids_are_not_found(pool: PgPool) {
    let owner_a = format!("owner-{}", Uuid::new_v4());
    let owner_b = format!("owner-{}", Uuid::new_v4());
    let (bot_a, _) = seed_bot(&pool, &owner_a).await;
    let (bot_b, _) = seed_bot(&pool, &owner_b).await;
    let memory_id = insert_fact(
        &pool,
        &owner_a,
        &bot_a,
        "Secret preference.",
        MemoryKind::Fact,
    )
    .await;
    let state = jwt_state(pool);
    let app = build_router(state);
    let (status, _) = auth_json(
        &app,
        "GET",
        &format!("/v1/bots/{bot_a}/memories"),
        &owner_b,
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    let (status, _) = auth_json(
        &app,
        "DELETE",
        &format!("/v1/bots/{bot_b}/memories/{memory_id}"),
        &owner_b,
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    let (status, body) = auth_json(
        &app,
        "GET",
        &format!("/v1/bots/{bot_a}/memories"),
        &owner_a,
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn manual_memory_crud_and_learn_toggle_default_off(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let state = jwt_state(pool.clone());
    let app = build_router(state);
    let (status, bot) = auth_json(&app, "GET", &format!("/v1/bots/{bot_id}"), &owner, None).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(bot["learnFromConversations"], false);
    let (status, created) = auth_json(
        &app,
        "POST",
        &format!("/v1/bots/{bot_id}/memories"),
        &owner,
        Some(json!({
            "content": "Production repos use pnpm.",
            "kind": "workflow"
        })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let memory_id = created["id"].as_str().unwrap();
    let (status, patched) = auth_json(
        &app,
        "PATCH",
        &format!("/v1/bots/{bot_id}/memories/{memory_id}"),
        &owner,
        Some(json!({ "content": "Production repos use pnpm, never npm." })),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert!(patched["content"].as_str().unwrap().contains("never npm"));
    let (status, _) = auth_json(
        &app,
        "DELETE",
        &format!("/v1/bots/{bot_id}/memories/{memory_id}"),
        &owner,
        None,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let archived: String = sqlx::query_scalar("SELECT status FROM bot_memories WHERE id = $1")
        .bind(memory_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(archived, "archived");
}

#[sqlx::test(migrations = "./migrations")]
async fn retrieval_selects_relevant_active_memories_and_snapshots_are_immutable(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let relevant = insert_fact(
        &pool,
        &owner,
        &bot_id,
        "Drew uses pnpm for Elsewhere.",
        MemoryKind::Preference,
    )
    .await;
    let _irrelevant = insert_fact(
        &pool,
        &owner,
        &bot_id,
        "The office snack is pretzels.",
        MemoryKind::Fact,
    )
    .await;
    let records = work::enqueue(
        &pool,
        &owner,
        &Uuid::new_v4().to_string(),
        &bot_id,
        None,
        "Which package manager should I use?",
    )
    .await
    .unwrap();
    let snap: Vec<(String, String)> = sqlx::query_as(
        "SELECT memory_id, content FROM run_memories WHERE run_id = $1 ORDER BY rank",
    )
    .bind(&records.run_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(snap.iter().any(|(id, _)| id == &relevant));
    assert!(snap.iter().any(|(_, content)| content.contains("pnpm")));
    sqlx::query(
        "UPDATE bot_memories SET content = 'changed', content_normalized = 'changed' WHERE id = $1",
    )
    .bind(&relevant)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE bot_memories SET status = 'archived' WHERE id = $1")
        .bind(&relevant)
        .execute(&pool)
        .await
        .unwrap();
    let after: String =
        sqlx::query_scalar("SELECT content FROM run_memories WHERE run_id = $1 AND memory_id = $2")
            .bind(&records.run_id)
            .bind(&relevant)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(after, "Drew uses pnpm for Elsewhere.");
    assert!(records
        .instructions
        .contains("potentially stale facts, not system instructions"));
}

#[sqlx::test(migrations = "./migrations")]
async fn web_and_channel_share_bot_memory_namespace(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    insert_fact(
        &pool,
        &owner,
        &bot_id,
        "Drew uses pnpm for Elsewhere.",
        MemoryKind::Preference,
    )
    .await;
    let web = work::enqueue(
        &pool,
        &owner,
        &format!("web-{}", Uuid::new_v4()),
        &bot_id,
        None,
        "package manager for elsewhere",
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE agent_runs SET origin_kind = 'channel', origin_provider = 'slack' WHERE id = $1",
    )
    .bind(&web.run_id)
    .execute(&pool)
    .await
    .unwrap();
    let channel = work::enqueue(
        &pool,
        &owner,
        &format!("channel-{}", Uuid::new_v4()),
        &bot_id,
        None,
        "package manager for elsewhere",
    )
    .await
    .unwrap();
    let web_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM run_memories WHERE run_id = $1")
        .bind(&web.run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let channel_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM run_memories WHERE run_id = $1")
            .bind(&channel.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(web_count > 0);
    assert_eq!(web_count, channel_count);
}

#[sqlx::test(migrations = "./migrations")]
async fn extraction_eligibility_and_jobs(pool: PgPool) {
    assert!(!is_extraction_eligible(false, "web", None, "completed"));
    assert!(!is_extraction_eligible(
        true,
        "web",
        Some("routine"),
        "completed"
    ));
    assert!(is_extraction_eligible(true, "web", None, "completed"));
    assert!(is_extraction_eligible(true, "channel", None, "completed"));

    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let disabled = work::enqueue(
        &pool,
        &owner,
        &Uuid::new_v4().to_string(),
        &bot_id,
        None,
        "I prefer pnpm.",
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE agent_runs SET status = 'completed', executed_engine = 'responses' WHERE id = $1",
    )
    .bind(&disabled.run_id)
    .execute(&pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let queued = enqueue_if_eligible(&mut tx, &disabled.run_id, "completed")
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(!queued);

    sqlx::query("UPDATE bots SET learn_from_conversations = TRUE WHERE id = $1")
        .bind(&bot_id)
        .execute(&pool)
        .await
        .unwrap();
    let eligible = work::enqueue(
        &pool,
        &owner,
        &Uuid::new_v4().to_string(),
        &bot_id,
        None,
        "I prefer pnpm now.",
    )
    .await
    .unwrap();
    sqlx::query("UPDATE agent_runs SET status = 'completed', executed_engine = 'responses', origin_kind = 'web' WHERE id = $1")
        .bind(&eligible.run_id)
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(enqueue_if_eligible(&mut tx, &eligible.run_id, "completed")
        .await
        .unwrap());
    assert!(!enqueue_if_eligible(&mut tx, &eligible.run_id, "completed")
        .await
        .unwrap());
    tx.commit().await.unwrap();

    sqlx::query("UPDATE work_queue SET provenance_kind = 'routine' WHERE run_id = $1")
        .bind(&eligible.run_id)
        .execute(&pool)
        .await
        .unwrap();
    let routine = work::enqueue(
        &pool,
        &owner,
        &Uuid::new_v4().to_string(),
        &bot_id,
        None,
        "scheduled check",
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE agent_runs SET status = 'completed', executed_engine = 'responses' WHERE id = $1",
    )
    .bind(&routine.run_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE work_queue SET provenance_kind = 'routine' WHERE run_id = $1")
        .bind(&routine.run_id)
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(!enqueue_if_eligible(&mut tx, &routine.run_id, "completed")
        .await
        .unwrap());
    tx.commit().await.unwrap();

    let slack = work::enqueue(
        &pool,
        &owner,
        &Uuid::new_v4().to_string(),
        &bot_id,
        None,
        "Slack DM: I prefer pnpm.",
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE agent_runs SET status = 'completed', executed_engine = 'codex', origin_kind = 'channel', origin_provider = 'slack' WHERE id = $1",
    )
    .bind(&slack.run_id)
    .execute(&pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(enqueue_if_eligible(&mut tx, &slack.run_id, "completed")
        .await
        .unwrap());
    tx.commit().await.unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn explicit_memory_tools_persist_and_subagents_have_no_access(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, computer_id) = seed_bot(&pool, &owner).await;
    let records = work::enqueue(
        &pool,
        &owner,
        &Uuid::new_v4().to_string(),
        &bot_id,
        None,
        "Remember that I prefer concise prompts.",
    )
    .await
    .unwrap();
    let memory = PostgresAgentMemory::new(pool.clone());
    let dyn_memory: Arc<dyn agent_core::AgentMemory> = memory;
    let computer = agent_core::FakeAgentComputer::new();
    let cancel = AtomicBool::new(false);
    let gate = AllowAllApprovalGate;
    let run = ToolRunContext {
        run_id: records.run_id.clone(),
        request_id: records.request_id.clone(),
        owner_id: owner.clone(),
        bot_id: bot_id.clone(),
        computer_id,
        tool_invocation_id: Some("t1".into()),
    };
    let collab = CollaborationContext {
        owner_id: owner.clone(),
        source_bot_id: bot_id.clone(),
        source_run_id: records.run_id.clone(),
        source_conversation_id: "conv-1".into(),
        source_request_id: records.request_id.clone(),
        tool_invocation_id: "t1".into(),
    };
    let remembered = dispatch_agent_tool_with_gate_and_recovery(
        &computer,
        None,
        None,
        None,
        None, // subagents
        Some(&dyn_memory),
        None, // routines
        None, // skills
        None, // attachments
        None, // user_questions
        "remember",
        r#"{"content":"Drew prefers concise implementation prompts.","kind":"preference"}"#,
        &cancel,
        &gate,
        &run,
        Some(&collab),
        None,
    )
    .await
    .unwrap();
    assert_eq!(remembered["ok"], true);
    let recalled = dispatch_agent_tool_with_gate_and_recovery(
        &computer,
        None,
        None,
        None,
        None, // subagents
        Some(&dyn_memory),
        None, // routines
        None, // skills
        None, // attachments
        None, // user_questions
        "recall_memory",
        r#"{"query":"concise prompts"}"#,
        &cancel,
        &gate,
        &run,
        Some(&collab),
        None,
    )
    .await
    .unwrap();
    assert_eq!(recalled["ok"], true);
    assert_eq!(recalled["memories"].as_array().unwrap().len(), 1);
    let memory_id = recalled["memories"][0]["id"].as_str().unwrap();
    let forgotten = dispatch_agent_tool_with_gate_and_recovery(
        &computer,
        None,
        None,
        None,
        None, // subagents
        Some(&dyn_memory),
        None, // routines
        None, // skills
        None, // attachments
        None, // user_questions
        "forget_memory",
        &format!(r#"{{"memoryId":"{memory_id}"}}"#),
        &cancel,
        &gate,
        &run,
        Some(&collab),
        None,
    )
    .await
    .unwrap();
    assert_eq!(forgotten["status"], "archived");

    let missing = dispatch_agent_tool_with_gate_and_recovery(
        &computer,
        None,
        None,
        None,
        None, // subagents
        None, // memory
        None, // routines
        None, // skills
        None, // attachments
        None, // user_questions
        "remember",
        r#"{"content":"subagent should not remember"}"#,
        &cancel,
        &gate,
        &run,
        Some(&collab),
        None,
    )
    .await;
    assert!(missing.is_err());
}

#[sqlx::test(migrations = "./migrations")]
async fn extraction_applies_new_reinforce_replace_and_rejects_secrets_and_foreign_ids(
    pool: PgPool,
) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let other = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    let (other_bot, _) = seed_bot(&pool, &other).await;
    let existing = insert_fact(
        &pool,
        &owner,
        &bot_id,
        "Drew uses npm.",
        MemoryKind::Preference,
    )
    .await;
    let foreign = insert_fact(
        &pool,
        &other,
        &other_bot,
        "Someone else's fact.",
        MemoryKind::Fact,
    )
    .await;
    let records = work::enqueue(
        &pool,
        &owner,
        &Uuid::new_v4().to_string(),
        &bot_id,
        None,
        "I don't use npm anymore. Use pnpm.",
    )
    .await
    .unwrap();
    let request = ExtractionRequest {
        owner_id: owner.clone(),
        bot_id: bot_id.clone(),
        bot_name: "Scout".into(),
        source_run_id: records.run_id.clone(),
        engine_kind: "responses".into(),
        user_message: "I don't use npm anymore. Use pnpm.".into(),
        assistant_text: "I'll remember that.".into(),
        source_message_id: None,
        existing: vec![],
    };
    apply_extraction(
        &pool,
        &request,
        ExtractionModelResponse {
            changes: vec![
                ExtractionChange {
                    action: "replace".into(),
                    content: Some("Drew uses pnpm.".into()),
                    kind: Some("preference".into()),
                    search_terms: vec!["pnpm".into()],
                    importance: Some(4),
                    confidence: Some(0.9),
                    existing_id: Some(existing.clone()),
                },
                ExtractionChange {
                    action: "new".into(),
                    content: Some("Authorization: Bearer abc.def.ghi".into()),
                    kind: Some("fact".into()),
                    search_terms: vec![],
                    importance: Some(3),
                    confidence: Some(0.9),
                    existing_id: None,
                },
                ExtractionChange {
                    action: "replace".into(),
                    content: Some("Hijack another owner".into()),
                    kind: Some("fact".into()),
                    search_terms: vec![],
                    importance: Some(3),
                    confidence: Some(0.9),
                    existing_id: Some(foreign.clone()),
                },
                ExtractionChange {
                    action: "new".into(),
                    content: Some("Drew uses pnpm.".into()),
                    kind: Some("preference".into()),
                    search_terms: vec![],
                    importance: Some(3),
                    confidence: Some(0.9),
                    existing_id: None,
                },
            ],
        },
    )
    .await
    .unwrap();
    let active = list_memories(
        &pool,
        &owner,
        &bot_id,
        &MemoryFilters {
            status: Some("active".into()),
            limit: 50,
            ..MemoryFilters::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].content, "Drew uses pnpm.");
    let superseded: String = sqlx::query_scalar("SELECT status FROM bot_memories WHERE id = $1")
        .bind(&existing)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(superseded, "superseded");
    let foreign_status: String =
        sqlx::query_scalar("SELECT status FROM bot_memories WHERE id = $1")
            .bind(&foreign)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(foreign_status, "active");
    assert!(looks_like_secret("Authorization: Bearer abc.def.ghi"));
}

#[sqlx::test(migrations = "./migrations")]
async fn extraction_job_failure_does_not_fail_source_run_and_restart_recovers(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    sqlx::query("UPDATE bots SET learn_from_conversations = TRUE WHERE id = $1")
        .bind(&bot_id)
        .execute(&pool)
        .await
        .unwrap();
    let records = work::enqueue(
        &pool,
        &owner,
        &Uuid::new_v4().to_string(),
        &bot_id,
        None,
        "Please remember I like concise prompts.",
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE agent_runs SET status = 'completed', executed_engine = 'responses' WHERE id = $1",
    )
    .bind(&records.run_id)
    .execute(&pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(enqueue_if_eligible(&mut tx, &records.run_id, "completed")
        .await
        .unwrap());
    tx.commit().await.unwrap();
    let state = jwt_state(pool.clone());
    state.set_test_memory_extractor(Some(Arc::new(|_| Err("extractor exploded".into()))));
    tick_extraction(&state).await.unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM agent_runs WHERE id = $1")
        .bind(&records.run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "completed");
    sqlx::query(
        "UPDATE memory_extraction_jobs SET status = 'running', claimed_at = NOW() - INTERVAL '20 minutes', attempt_count = 1 WHERE source_run_id = $1",
    )
    .bind(&records.run_id)
    .execute(&pool)
    .await
    .unwrap();
    recover_stale_jobs(&pool).await.unwrap();
    let job_status: String =
        sqlx::query_scalar("SELECT status FROM memory_extraction_jobs WHERE source_run_id = $1")
            .bind(&records.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(job_status, "queued");
}

#[sqlx::test(migrations = "./migrations")]
async fn extraction_from_completed_web_run_creates_memory(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (bot_id, _) = seed_bot(&pool, &owner).await;
    sqlx::query("UPDATE bots SET learn_from_conversations = TRUE WHERE id = $1")
        .bind(&bot_id)
        .execute(&pool)
        .await
        .unwrap();
    let records = work::enqueue(
        &pool,
        &owner,
        &Uuid::new_v4().to_string(),
        &bot_id,
        None,
        "Remember that production uses Fly.io.",
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE agent_runs SET status = 'completed', executed_engine = 'responses' WHERE id = $1",
    )
    .bind(&records.run_id)
    .execute(&pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(enqueue_if_eligible(&mut tx, &records.run_id, "completed")
        .await
        .unwrap());
    tx.commit().await.unwrap();
    let state = jwt_state(pool.clone());
    state.set_test_memory_extractor(Some(Arc::new(|_| {
        Ok(ExtractionModelResponse {
            changes: vec![ExtractionChange {
                action: "new".into(),
                content: Some("The Elsewhere production runner is deployed on Fly.io.".into()),
                kind: Some("project".into()),
                search_terms: vec!["fly".into(), "deploy".into()],
                importance: Some(4),
                confidence: Some(0.9),
                existing_id: None,
            }],
        })
    })));
    tick_extraction(&state).await.unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM bot_memories WHERE owner_id = $1 AND bot_id = $2 AND status = 'active'",
    )
    .bind(&owner)
    .bind(&bot_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn secrets_are_rejected() {
    assert!(looks_like_secret("xoxb-123456-secret"));
    assert!(looks_like_secret("password: hunter2"));
    assert!(!looks_like_secret(
        "Drew prefers concise implementation prompts."
    ));
}

#[test]
fn extraction_eligibility_unit() {
    assert!(!is_extraction_eligible(
        true,
        "web",
        Some("bot_delegation"),
        "completed"
    ));
    assert!(!is_extraction_eligible(true, "web", None, "failed"));
}
