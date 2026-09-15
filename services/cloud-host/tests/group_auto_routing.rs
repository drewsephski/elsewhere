use cloud_host::{
    db::resources,
    group_router::{
        build_router_transcript, candidate_fingerprint, parse_and_validate_router_output,
        retry_auto_route, GroupRoutingMode, RouteCandidate, RouteDecisionInput,
        ValidatedRouteDecision, MAX_ROUTE_ATTEMPTS,
    },
    run_engine_select::{resolve_group_route_engine, RunEngineMode, SelectedRunEngine},
    drain_one_pending_route,
    groups::{self, SendGroupMessageRequest},
    AppState, Config,
};
use sqlx::PgPool;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

const TEST_SYNC_TIMEOUT: Duration = Duration::from_secs(10);
const TEST_DRAIN_TIMEOUT: Duration = Duration::from_secs(15);

/// Ensures a blocking test decider always unblocks and clears injected hooks.
struct TestGroupRouteDeciderGuard {
    release: Arc<AtomicBool>,
    state: AppState,
}

impl TestGroupRouteDeciderGuard {
    fn new(state: AppState) -> (Self, Arc<AtomicBool>, Arc<AtomicBool>) {
        let release = Arc::new(AtomicBool::new(false));
        let decider_started = Arc::new(AtomicBool::new(false));
        let guard = Self {
            release: release.clone(),
            state: state.clone(),
        };
        (guard, release, decider_started)
    }

    fn release_decider(&self) {
        self.release.store(true, Ordering::SeqCst);
    }
}

impl Drop for TestGroupRouteDeciderGuard {
    fn drop(&mut self) {
        self.release_decider();
        self.state.set_test_group_route_decider(None);
    }
}

async fn wait_for_flag(flag: &AtomicBool, label: &str) {
    tokio::time::timeout(TEST_SYNC_TIMEOUT, async {
        while !flag.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {label}"));
}

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
        legacy_local_approval_bypass: false,
        browser_enabled: false,
        connector_secret_key: None,
        github_client_id: None,
        github_client_secret: None,
        github_oauth_redirect_uri: None,
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

            skill_invocation: None,        },
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

    let state = AppState::new(pool.clone(), test_config());
    state.set_test_group_route_decider(Some(Arc::new(|input| {
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

            skill_invocation: None,        },
    )
    .await
    .unwrap();

    assert!(drain_one_pending_route(&state).await.unwrap());
    state.set_test_group_route_decider(None);

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

            skill_invocation: None,        },
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

            skill_invocation: None,        },
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

#[sqlx::test(migrations = "./migrations")]
async fn auto_engine_ignores_empty_provider_cache(pool: PgPool) {
    assert_eq!(
        resolve_group_route_engine(RunEngineMode::Auto, Some("sk-test")).unwrap(),
        SelectedRunEngine::CodexSubscription
    );
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
    let mut config = test_config();
    config.run_engine = RunEngineMode::Auto;
    let state = AppState::new(pool.clone(), config);
    state.set_test_group_route_decider(Some(Arc::new(|_| {
        Ok(ValidatedRouteDecision {
            mode: GroupRoutingMode::Auto,
            bot_ids: Vec::new(),
            decision_code: Some("no_fit".into()),
        })
    })));
    let send = groups::send_group_message(
        &pool,
        "alice",
        &group.id,
        &Uuid::new_v4().to_string(),
        SendGroupMessageRequest {
            body: "thanks".into(),
            recipient_bot_ids: None,
            mention_mode: None,
            routing_mode: Some("auto".into()),

            skill_invocation: None,        },
    )
    .await
    .unwrap();
    assert!(drain_one_pending_route(&state).await.unwrap());
    state.set_test_group_route_decider(None);
    let status: String = sqlx::query_scalar(
        "SELECT routing_status FROM group_message_sends WHERE message_id = $1",
    )
    .bind(&send.message.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "no_response");
}

#[sqlx::test(migrations = "./migrations")]
async fn removed_selected_bot_before_apply_requeues_route(pool: PgPool) {
    let researcher = bot(&pool, "alice", "Researcher").await;
    let designer = bot(&pool, "alice", "Designer").await;
    let coordinator = bot(&pool, "alice", "Coordinator").await;
    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Team".into(),
            bot_ids: vec![
                researcher.id.clone(),
                designer.id.clone(),
                coordinator.id.clone(),
            ],
        },
    )
    .await
    .unwrap();

    let state = AppState::new(pool.clone(), test_config());
    let (decider_guard, release_decider, decider_started) =
        TestGroupRouteDeciderGuard::new(state.clone());
    let researcher_id = researcher.id.clone();
    let release_for_decider = release_decider.clone();
    let started_for_decider = decider_started.clone();
    state.set_test_group_route_decider(Some(Arc::new(move |_| {
        started_for_decider.store(true, Ordering::SeqCst);
        let deadline = Instant::now() + TEST_SYNC_TIMEOUT;
        while !release_for_decider.load(Ordering::SeqCst) {
            if Instant::now() >= deadline {
                return Err("decider_gate_timeout".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(ValidatedRouteDecision {
            mode: GroupRoutingMode::Specific,
            bot_ids: vec![researcher_id.clone()],
            decision_code: Some("single_owner".into()),
        })
    })));

    let send = groups::send_group_message(
        &pool,
        "alice",
        &group.id,
        &Uuid::new_v4().to_string(),
        SendGroupMessageRequest {
            body: "Research this".into(),
            recipient_bot_ids: None,
            mention_mode: None,
            routing_mode: Some("auto".into()),

            skill_invocation: None,        },
    )
    .await
    .unwrap();

    let pool_for_remove = pool.clone();
    let group_id = group.id.clone();
    let researcher_id_for_remove = researcher.id.clone();
    let release_for_task = release_decider.clone();
    let state_for_drain = state.clone();
    let (drain_ok, ()) = tokio::time::timeout(
        TEST_DRAIN_TIMEOUT,
        async {
            let drain = drain_one_pending_route(&state_for_drain);
            let coord = async {
                wait_for_flag(&decider_started, "group route decider start").await;
                groups::remove_participant(
                    &pool_for_remove,
                    "alice",
                    &group_id,
                    &researcher_id_for_remove,
                )
                .await
                .unwrap();
                release_for_task.store(true, Ordering::SeqCst);
            };
            tokio::join!(drain, coord)
        },
    )
    .await
    .expect("route drain/coordination timed out");
    assert!(drain_ok.expect("drain_one_pending_route failed"));
    drop(decider_guard);

    let researcher_runs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM group_message_recipients WHERE message_id = $1 AND bot_id = $2",
    )
    .bind(&send.message.id)
    .bind(&researcher.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(researcher_runs, 0);

    let status: String = sqlx::query_scalar(
        "SELECT routing_status FROM group_message_sends WHERE message_id = $1",
    )
    .bind(&send.message.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "pending");
}

#[sqlx::test(migrations = "./migrations")]
async fn retry_resets_attempts_after_max_failures(pool: PgPool) {
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

    let state = AppState::new(pool.clone(), test_config());
    state.set_test_group_route_decider(Some(Arc::new(|_| {
        Err("codex_busy".into())
    })));

    let send = groups::send_group_message(
        &pool,
        "alice",
        &group.id,
        &Uuid::new_v4().to_string(),
        SendGroupMessageRequest {
            body: "go".into(),
            recipient_bot_ids: None,
            mention_mode: None,
            routing_mode: Some("auto".into()),

            skill_invocation: None,        },
    )
    .await
    .unwrap();

    for attempt in 1..=MAX_ROUTE_ATTEMPTS {
        assert!(
            drain_one_pending_route(&state).await.unwrap(),
            "expected pending route drain on attempt {attempt}"
        );
    }
    let attempts: i32 = sqlx::query_scalar(
        "SELECT routing_attempts FROM group_message_sends WHERE message_id = $1",
    )
    .bind(&send.message.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(attempts, MAX_ROUTE_ATTEMPTS);
    let status: String = sqlx::query_scalar(
        "SELECT routing_status FROM group_message_sends WHERE message_id = $1",
    )
    .bind(&send.message.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "failed");
    assert!(
        !drain_one_pending_route(&state).await.unwrap(),
        "failed route should not be claimed again"
    );

    retry_auto_route(&pool, "alice", &group.id, &send.message.id)
        .await
        .unwrap();

    let attempts_after_retry: i32 = sqlx::query_scalar(
        "SELECT routing_attempts FROM group_message_sends WHERE message_id = $1",
    )
    .bind(&send.message.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(attempts_after_retry, 0);
    let status_after_retry: String = sqlx::query_scalar(
        "SELECT routing_status FROM group_message_sends WHERE message_id = $1",
    )
    .bind(&send.message.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status_after_retry, "pending");

    state.set_test_group_route_decider(Some(Arc::new(|input| {
        Ok(ValidatedRouteDecision {
            mode: GroupRoutingMode::Specific,
            bot_ids: vec![input.candidates[0].bot_id.clone()],
            decision_code: Some("single_owner".into()),
        })
    })));

    assert!(drain_one_pending_route(&state).await.unwrap());
    state.set_test_group_route_decider(None);

    let recipients: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM group_message_recipients WHERE message_id = $1",
    )
    .bind(&send.message.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(recipients, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn everyone_routes_to_all_eligible_bots(pool: PgPool) {
    let bots = [
        bot(&pool, "alice", "One").await,
        bot(&pool, "alice", "Two").await,
        bot(&pool, "alice", "Three").await,
    ];
    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Team".into(),
            bot_ids: bots.iter().map(|b| b.id.clone()).collect(),
        },
    )
    .await
    .unwrap();

    let state = AppState::new(pool.clone(), test_config());
    state.set_test_group_route_decider(Some(Arc::new(|_| {
        Ok(ValidatedRouteDecision {
            mode: GroupRoutingMode::Everyone,
            bot_ids: Vec::new(),
            decision_code: Some("everyone_requested".into()),
        })
    })));

    let send = groups::send_group_message(
        &pool,
        "alice",
        &group.id,
        &Uuid::new_v4().to_string(),
        SendGroupMessageRequest {
            body: "What does everyone think?".into(),
            recipient_bot_ids: None,
            mention_mode: None,
            routing_mode: Some("auto".into()),

            skill_invocation: None,        },
    )
    .await
    .unwrap();

    assert!(drain_one_pending_route(&state).await.unwrap());
    state.set_test_group_route_decider(None);

    let recipients: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM group_message_recipients WHERE message_id = $1",
    )
    .bind(&send.message.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(recipients, 3);
}

#[sqlx::test(migrations = "./migrations")]
async fn router_transcript_prefers_newest_under_byte_cap(pool: PgPool) {
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

    let huge = "x".repeat(39_993);
    let current_id = Uuid::new_v4().to_string();
    let bodies = [huge.as_str(), "recent one", "recent two", "recent three", "current"];
    for (seq, body) in bodies.iter().enumerate() {
        let id = if *body == "current" {
            current_id.clone()
        } else {
            Uuid::new_v4().to_string()
        };
        sqlx::query(
            r#"
            INSERT INTO messages (
                id, conversation_id, sequence, body, author_kind, role, kind, status
            ) VALUES ($1, $2, $3, $4, 'human', 'user', 'chat', 'complete')
            "#,
        )
        .bind(&id)
        .bind(&group.id)
        .bind((seq + 1) as i64)
        .bind(body)
        .execute(&pool)
        .await
        .unwrap();
    }

    let transcript = build_router_transcript(&pool, &group.id, &current_id)
        .await
        .unwrap();
    assert!(!transcript.contains(&huge[..1024]));
    assert!(transcript.contains("recent three"));
    assert!(transcript.contains("recent two"));
    assert!(!transcript.contains("current"));
}

#[test]
fn invalid_router_output_does_not_enqueue() {
    let input = RouteDecisionInput {
        message_body: "x".into(),
        candidates: vec![RouteCandidate {
            bot_id: "b1".into(),
            name: "B".into(),
            role_summary: "r".into(),
            available: true,
        }],
        transcript_json: String::new(),
    };
    assert!(parse_and_validate_router_output("not json", &input).is_err());
    assert!(parse_and_validate_router_output(
        "Here: {\"mode\":\"specific\",\"botIds\":[\"b1\"]}",
        &input
    )
    .is_err());
}

#[test]
fn candidate_fingerprint_changes_when_roster_changes() {
    let a = vec![RouteCandidate {
        bot_id: "a".into(),
        name: "A".into(),
        role_summary: "r".into(),
        available: true,
    }];
    let b = vec![
        RouteCandidate {
            bot_id: "a".into(),
            name: "A".into(),
            role_summary: "r".into(),
            available: true,
        },
        RouteCandidate {
            bot_id: "b".into(),
            name: "B".into(),
            role_summary: "r".into(),
            available: true,
        },
    ];
    assert_ne!(candidate_fingerprint(&a), candidate_fingerprint(&b));
}
