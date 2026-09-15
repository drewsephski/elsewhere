use agent_core::{CollaborationContext, MAX_CHILD_DELEGATIONS_PER_ROOT};
use cloud_host::{db::resources, delegation, work};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::Barrier;
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
        "none",
    )
    .await
    .unwrap();
    let second = delegation::create_delegation(&pool, &ctx, &researcher.id, "Investigate pricing", None, "none")
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
        "none",
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
    let err = delegation::create_delegation(&pool, &ctx, &bob_bot.id, "Spy", None, "none")
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
            "none",
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
    let too_deep = delegation::create_delegation(&pool, &parent_ctx, &b.id, "loop", None, "none")
        .await
        .unwrap_err();
    assert!(matches!(
        too_deep,
        agent_core::CollaborationError::LimitExceeded(_)
    ));
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_same_invocation_returns_one_delegation(pool: PgPool) {
    let chief = bot_with_computer(&pool, "alice", "Chief").await;
    let researcher = bot_with_computer(&pool, "alice", "Researcher").await;
    let source = enqueue_user_run(&pool, "alice", &chief.id, "Plan").await;
    let ctx = CollaborationContext {
        owner_id: "alice".into(),
        source_bot_id: chief.id.clone(),
        source_run_id: source.run_id.clone(),
        source_conversation_id: source.conversation_id.clone(),
        source_request_id: source.request_id.clone(),
        tool_invocation_id: "invoke-race".into(),
    };

    let barrier = Arc::new(Barrier::new(2));
    let pool_a = pool.clone();
    let pool_b = pool.clone();
    let ctx_a = ctx.clone();
    let ctx_b = ctx.clone();
    let target = researcher.id.clone();

    let target_a = target.clone();
    let barrier_b = barrier.clone();
    let first = tokio::spawn(async move {
        barrier.wait().await;
        delegation::create_delegation(&pool_a, &ctx_a, &target_a, "Race", None, "none")
            .await
            .unwrap()
    });
    let second = tokio::spawn(async move {
        barrier_b.wait().await;
        delegation::create_delegation(&pool_b, &ctx_b, &target, "Race", None, "none")
            .await
            .unwrap()
    });

    let a = first.await.unwrap();
    let b = second.await.unwrap();
    assert_eq!(a.delegation_id, b.delegation_id);
    assert_eq!(a.target_run_id, b.target_run_id);

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM bot_delegations WHERE source_run_id = $1 AND tool_invocation_id = $2",
    )
    .bind(&source.run_id)
    .bind("invoke-race")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_fanout_cannot_exceed_root_limit(pool: PgPool) {
    let chief = bot_with_computer(&pool, "alice", "Chief").await;
    let helper = bot_with_computer(&pool, "alice", "Helper").await;
    let source = enqueue_user_run(&pool, "alice", &chief.id, "Fanout").await;
    let remaining = MAX_CHILD_DELEGATIONS_PER_ROOT as usize;

    for index in 0..remaining {
        let ctx = CollaborationContext {
            owner_id: "alice".into(),
            source_bot_id: chief.id.clone(),
            source_run_id: source.run_id.clone(),
            source_conversation_id: source.conversation_id.clone(),
            source_request_id: source.request_id.clone(),
            tool_invocation_id: format!("fan-{index}"),
        };
        delegation::create_delegation(&pool, &ctx, &helper.id, "task", None, "none")
            .await
            .unwrap();
    }

    let ctx_overflow = CollaborationContext {
        owner_id: "alice".into(),
        source_bot_id: chief.id.clone(),
        source_run_id: source.run_id.clone(),
        source_conversation_id: source.conversation_id.clone(),
        source_request_id: source.request_id.clone(),
        tool_invocation_id: "fan-overflow-a".into(),
    };
    let ctx_overflow_b = CollaborationContext {
        tool_invocation_id: "fan-overflow-b".into(),
        ..ctx_overflow.clone()
    };

    let barrier = Arc::new(Barrier::new(2));
    let pool_a = pool.clone();
    let pool_b = pool.clone();
    let helper_id = helper.id.clone();

    let helper_a = helper_id.clone();
    let helper_b = helper_id.clone();
    let barrier_b = barrier.clone();
    let first = tokio::spawn(async move {
        barrier.wait().await;
        delegation::create_delegation(&pool_a, &ctx_overflow, &helper_a, "one more", None, "none").await
    });
    let second = tokio::spawn(async move {
        barrier_b.wait().await;
        delegation::create_delegation(&pool_b, &ctx_overflow_b, &helper_b, "one more", None, "none").await
    });

    let outcome_a = first.await.unwrap();
    let outcome_b = second.await.unwrap();
    assert!(outcome_a.is_err());
    assert!(outcome_b.is_err());

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM bot_delegations WHERE root_run_id = $1",
    )
    .bind(&source.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(total, MAX_CHILD_DELEGATIONS_PER_ROOT);
}

#[sqlx::test(migrations = "./migrations")]
async fn resume_source_creates_one_source_continuation(pool: PgPool) {
    let chief = bot_with_computer(&pool, "alice", "Chief").await;
    let researcher = bot_with_computer(&pool, "alice", "Researcher").await;
    let source = enqueue_user_run(&pool, "alice", &chief.id, "Plan").await;
    let ctx = CollaborationContext {
        owner_id: "alice".into(),
        source_bot_id: chief.id.clone(),
        source_run_id: source.run_id.clone(),
        source_conversation_id: source.conversation_id.clone(),
        source_request_id: source.request_id.clone(),
        tool_invocation_id: "resume-1".into(),
    };
    let created = delegation::create_delegation(
        &pool,
        &ctx,
        &researcher.id,
        "Investigate pricing",
        None,
        "resume_source",
    )
    .await
    .unwrap();

    sqlx::query(
        "UPDATE agent_runs SET status = 'completed' WHERE id = $1",
    )
    .bind(&created.target_run_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE messages SET body = 'Pricing is X', status = 'complete' WHERE id = (SELECT assistant_message_id FROM agent_runs WHERE id = $1)")
        .bind(&created.target_run_id)
        .execute(&pool)
        .await
        .unwrap();

    let target_request: String = sqlx::query_scalar(
        "SELECT request_id FROM agent_runs WHERE id = $1",
    )
    .bind(&created.target_run_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    delegation::sync_target_run_terminal(&pool, &target_request, "completed", None)
        .await
        .unwrap();
    delegation::sync_target_run_terminal(&pool, &target_request, "completed", None)
        .await
        .unwrap();

    let resume_run: Option<String> = sqlx::query_scalar(
        "SELECT source_resume_run_id FROM bot_delegations WHERE id = $1",
    )
    .bind(&created.delegation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(resume_run.is_some());
    let resume_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_runs WHERE bot_id = $1 AND id <> $2",
    )
    .bind(&chief.id)
    .bind(&source.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(resume_count, 1);

    let humans: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE conversation_id = $1 AND author_kind = 'human'",
    )
    .bind(&source.conversation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(humans, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn on_complete_none_skips_source_resume(pool: PgPool) {
    let chief = bot_with_computer(&pool, "alice", "Chief").await;
    let researcher = bot_with_computer(&pool, "alice", "Researcher").await;
    let source = enqueue_user_run(&pool, "alice", &chief.id, "Hand off").await;
    let ctx = CollaborationContext {
        owner_id: "alice".into(),
        source_bot_id: chief.id.clone(),
        source_run_id: source.run_id.clone(),
        source_conversation_id: source.conversation_id.clone(),
        source_request_id: source.request_id.clone(),
        tool_invocation_id: "none-1".into(),
    };
    let created = delegation::create_delegation(
        &pool,
        &ctx,
        &researcher.id,
        "Take over",
        None,
        "none",
    )
    .await
    .unwrap();
    let target_request: String = sqlx::query_scalar(
        "SELECT request_id FROM agent_runs WHERE id = $1",
    )
    .bind(&created.target_run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    delegation::sync_target_run_terminal(&pool, &target_request, "completed", None)
        .await
        .unwrap();
    let resume: Option<String> = sqlx::query_scalar(
        "SELECT source_resume_run_id FROM bot_delegations WHERE id = $1",
    )
    .bind(&created.delegation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(resume.is_none());
}
