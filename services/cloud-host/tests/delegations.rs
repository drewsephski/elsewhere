use agent_core::CollaborationContext;
use cloud_host::{db::resources, delegation, work};
use sqlx::PgPool;
use uuid::Uuid;

async fn bot_with_computer(pool: &PgPool, owner: &str, name: &str) -> resources::BotRow {
    let computer_id = resources::insert_computer_placeholder(pool, owner, "Computer")
        .await
        .unwrap()
        .id;
    let instructions = format!("Role for {name}");
    resources::insert_bot(
        pool,
        owner,
        name,
        &instructions,
        "gpt-5.6-luna",
        Some(&computer_id),
        "codex",
        "sky-wisp",
    )
    .await
    .unwrap()
}

async fn enqueue_user_run(
    pool: &PgPool,
    owner: &str,
    bot_id: &str,
    message: &str,
) -> cloud_host::db::queries::BootstrapRunRecords {
    work::enqueue(pool, owner, &Uuid::new_v4().to_string(), bot_id, None, message)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn owner_can_delegate_and_idempotency_is_per_invocation(pool: PgPool) {
    let chief = bot_with_computer(&pool, "alice", "Chief").await;
    let researcher = bot_with_computer(&pool, "alice", "Researcher").await;
    let source = enqueue_user_run(&pool, "alice", &chief.id, "Plan the day").await;

    let ctx = CollaborationContext {
        owner_id: "alice".into(),
        source_bot_id: chief.id.clone(),
        source_run_id: source.run_id.clone(),
        source_conversation_id: source.conversation_id.clone(),
        source_request_id: source.request_id.clone(),
        tool_invocation_id: "invoke-1".into(),
    };

    let first = delegation::create_delegation(
        &pool,
        &ctx,
        &researcher.id,
        "Investigate pricing",
        Some("Focus on enterprise tiers"),
    )
    .await
    .unwrap();
    let second = delegation::create_delegation(&pool, &ctx, &researcher.id, "Investigate pricing", None)
        .await
        .unwrap();
    assert_eq!(first.delegation_id, second.delegation_id);
    assert_eq!(first.target_run_id, second.target_run_id);

    let ctx2 = CollaborationContext {
        tool_invocation_id: "invoke-2".into(),
        ..ctx
    };
    let another = delegation::create_delegation(
        &pool,
        &ctx2,
        &researcher.id,
        "Investigate pricing",
        None,
    )
    .await
    .unwrap();
    assert_ne!(first.delegation_id, another.delegation_id);

    let rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM bot_delegations WHERE source_run_id = $1",
    )
    .bind(&source.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rows, 2);

    let target_instructions: String = sqlx::query_scalar(
        "SELECT instructions FROM work_queue WHERE run_id = $1",
    )
    .bind(&first.target_run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(target_instructions.contains("You are \"Researcher\""));
    assert!(target_instructions.contains("Role for Researcher"));
}

#[sqlx::test(migrations = "./migrations")]
async fn cross_owner_target_is_not_found(pool: PgPool) {
    let chief = bot_with_computer(&pool, "alice", "Chief").await;
    let bob_bot = bot_with_computer(&pool, "bob", "Secret").await;
    let source = enqueue_user_run(&pool, "alice", &chief.id, "Delegate").await;
    let ctx = CollaborationContext {
        owner_id: "alice".into(),
        source_bot_id: chief.id.clone(),
        source_run_id: source.run_id.clone(),
        source_conversation_id: source.conversation_id.clone(),
        source_request_id: source.request_id.clone(),
        tool_invocation_id: "invoke-x".into(),
    };
    let err = delegation::create_delegation(&pool, &ctx, &bob_bot.id, "Spy", None)
        .await
        .unwrap_err();
    assert_eq!(err, agent_core::CollaborationError::NotFound);
}

#[sqlx::test(migrations = "./migrations")]
async fn depth_and_fanout_limits(pool: PgPool) {
    let a = bot_with_computer(&pool, "alice", "A").await;
    let b = bot_with_computer(&pool, "alice", "B").await;
    let source = enqueue_user_run(&pool, "alice", &a.id, "start").await;

    let mut current_run = source.run_id.clone();
    let mut parent_ctx = CollaborationContext {
        owner_id: "alice".into(),
        source_bot_id: a.id.clone(),
        source_run_id: source.run_id.clone(),
        source_conversation_id: source.conversation_id.clone(),
        source_request_id: source.request_id.clone(),
        tool_invocation_id: "d0".into(),
    };
    for depth in 1..=4 {
        let instruction = format!("hop {depth}");
        let target_id = if depth % 2 == 1 { &b.id } else { &a.id };
        let created = delegation::create_delegation(
            &pool,
            &parent_ctx,
            target_id,
            &instruction,
            None,
        )
        .await
        .unwrap();
        current_run = created.target_run_id.clone();
        parent_ctx = CollaborationContext {
            source_bot_id: target_id.clone(),
            source_run_id: current_run.clone(),
            source_conversation_id: sqlx::query_scalar(
                "SELECT conversation_id FROM agent_runs WHERE id = $1",
            )
            .bind(&current_run)
            .fetch_one(&pool)
            .await
            .unwrap(),
            source_request_id: format!("delegation-{}", created.delegation_id),
            tool_invocation_id: format!("d{depth}"),
            owner_id: "alice".into(),
        };
    }
    let too_deep = delegation::create_delegation(&pool, &parent_ctx, &b.id, "loop", None)
        .await
        .unwrap_err();
    assert!(matches!(
        too_deep,
        agent_core::CollaborationError::LimitExceeded(_)
    ));
}
