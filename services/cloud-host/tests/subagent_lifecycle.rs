use agent_core::{
    AgentSubagents, EventSink, SubagentContext, SubagentError, SubagentRequest, SubagentTurn,
};
use async_trait::async_trait;
use cloud_host::{
    db::{postgres_run_store::PostgresRunStore, queries, resources},
    events::cloud_event_sink::CloudEventSink,
    subagents::{reconcile_orphaned_subagents, PostgresAgentSubagents},
    work,
};
use sqlx::PgPool;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

struct CountingTurn {
    calls: AtomicUsize,
    delay: Duration,
    text: String,
}

#[async_trait]
impl SubagentTurn for CountingTurn {
    async fn run_toolless(
        &self,
        _model: &str,
        _developer_instructions: &str,
        _user_prompt: &str,
        cancel: &AtomicBool,
    ) -> Result<String, SubagentError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let end = tokio::time::Instant::now() + self.delay;
        while tokio::time::Instant::now() < end {
            if cancel.load(Ordering::Relaxed) {
                return Err(SubagentError::Cancelled);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Ok(self.text.clone())
    }
}

async fn seed_parent(pool: &PgPool, owner: &str) -> (SubagentContext, Arc<PostgresAgentSubagents>) {
    let computer = resources::insert_computer_placeholder(pool, owner, "Computer")
        .await
        .unwrap();
    let bot = resources::insert_bot(
        pool,
        owner,
        "Scout",
        "role",
        "gpt-5.6-luna",
        Some(computer.id.as_str()),
        "responses",
        "sky-wisp",
    )
    .await
    .unwrap();
    let source = work::enqueue(
        pool,
        owner,
        &Uuid::new_v4().to_string(),
        &bot.id,
        None,
        "Plan",
    )
    .await
    .unwrap();
    let store = Arc::new(PostgresRunStore::new(pool.clone()));
    let (events, _rx) = CloudEventSink::new();
    let cancel = Arc::new(AtomicBool::new(false));
    let service = PostgresAgentSubagents::new(
        pool.clone(),
        store,
        Arc::new(events) as Arc<dyn EventSink>,
        cancel.clone(),
        "gpt-5.6-luna",
    );
    let ctx = SubagentContext {
        owner_id: owner.into(),
        bot_id: bot.id,
        parent_run_id: source.run_id,
        parent_request_id: source.request_id,
        tool_invocation_id: "mcp:1".into(),
        model: "gpt-5.6-luna".into(),
        cancel,
    };
    (ctx, service)
}

#[sqlx::test(migrations = "./migrations")]
async fn duplicate_invocation_does_not_rerun_helper(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (ctx, service) = seed_parent(&pool, &owner).await;
    let turn = Arc::new(CountingTurn {
        calls: AtomicUsize::new(0),
        delay: Duration::from_millis(0),
        text: "findings".into(),
    });
    service.attach_turn_executor(turn.clone());
    let request = SubagentRequest {
        name: "Reviewer".into(),
        task: "review the change".into(),
        context: None,
    };
    let first = service.run_subagent(&ctx, request.clone()).await.unwrap();
    let second = service.run_subagent(&ctx, request).await.unwrap();
    assert_eq!(first.subagent_id, second.subagent_id);
    assert_eq!(second.status, "completed");
    assert_eq!(second.result.as_deref(), Some("findings"));
    assert_eq!(turn.calls.load(Ordering::SeqCst), 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn in_flight_duplicate_joins_the_same_helper(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (ctx, service) = seed_parent(&pool, &owner).await;
    let turn = Arc::new(CountingTurn {
        calls: AtomicUsize::new(0),
        delay: Duration::from_millis(250),
        text: "joined".into(),
    });
    service.attach_turn_executor(turn.clone());
    let request = SubagentRequest {
        name: "Planner".into(),
        task: "outline next steps".into(),
        context: None,
    };
    let first = tokio::spawn({
        let service = service.clone();
        let ctx = ctx.clone();
        let request = request.clone();
        async move { service.run_subagent(&ctx, request).await }
    });
    tokio::time::sleep(Duration::from_millis(40)).await;
    let second = service.run_subagent(&ctx, request).await.unwrap();
    let first = first.await.unwrap().unwrap();
    assert_eq!(first.subagent_id, second.subagent_id);
    assert_eq!(turn.calls.load(Ordering::SeqCst), 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn enforces_max_four_and_one_active(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (mut ctx, service) = seed_parent(&pool, &owner).await;
    service.attach_turn_executor(Arc::new(CountingTurn {
        calls: AtomicUsize::new(0),
        delay: Duration::from_millis(0),
        text: "ok".into(),
    }));
    for i in 0..4 {
        ctx.tool_invocation_id = format!("mcp:{i}");
        service
            .run_subagent(
                &ctx,
                SubagentRequest {
                    name: format!("Helper{i}"),
                    task: "do a focused task".into(),
                    context: None,
                },
            )
            .await
            .unwrap();
    }
    ctx.tool_invocation_id = "mcp:5".into();
    let err = service
        .run_subagent(
            &ctx,
            SubagentRequest {
                name: "Extra".into(),
                task: "one more".into(),
                context: None,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, SubagentError::LimitExceeded(_)));

    let owner2 = format!("owner-{}", Uuid::new_v4());
    let (ctx2, service2) = seed_parent(&pool, &owner2).await;
    service2.attach_turn_executor(Arc::new(CountingTurn {
        calls: AtomicUsize::new(0),
        delay: Duration::from_millis(400),
        text: "slow".into(),
    }));
    let slow = tokio::spawn({
        let service = service2.clone();
        let ctx = ctx2.clone();
        async move {
            service
                .run_subagent(
                    &ctx,
                    SubagentRequest {
                        name: "Slow".into(),
                        task: "take a moment".into(),
                        context: None,
                    },
                )
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(40)).await;
    let mut other = ctx2.clone();
    other.tool_invocation_id = "mcp:other".into();
    let err = service2
        .run_subagent(
            &other,
            SubagentRequest {
                name: "Parallel".into(),
                task: "should not start".into(),
                context: None,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, SubagentError::LimitExceeded(_)));
    let _ = slow.await.unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn parent_cancel_interrupts_active_helper(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (ctx, service) = seed_parent(&pool, &owner).await;
    service.attach_turn_executor(Arc::new(CountingTurn {
        calls: AtomicUsize::new(0),
        delay: Duration::from_secs(5),
        text: "too late".into(),
    }));
    let running = tokio::spawn({
        let service = service.clone();
        let ctx = ctx.clone();
        async move {
            service
                .run_subagent(
                    &ctx,
                    SubagentRequest {
                        name: "Research".into(),
                        task: "look this up".into(),
                        context: None,
                    },
                )
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(40)).await;
    ctx.cancel.store(true, Ordering::SeqCst);
    let err = running.await.unwrap().unwrap_err();
    assert!(matches!(err, SubagentError::Cancelled));
}

#[sqlx::test(migrations = "./migrations")]
async fn host_restart_reconciles_running_rows(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let (ctx, _service) = seed_parent(&pool, &owner).await;
    sqlx::query(
        r#"
        INSERT INTO run_subagents (
            id, owner_id, parent_run_id, bot_id, tool_invocation_id, name, task, status
        ) VALUES ($1, $2, $3, $4, $5, 'Orphan', 'left running', 'running')
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&ctx.owner_id)
    .bind(&ctx.parent_run_id)
    .bind(&ctx.bot_id)
    .bind("mcp:orphan")
    .execute(&pool)
    .await
    .unwrap();
    let _ = queries::mark_interrupted_runs(&pool).await.unwrap();
    let count = reconcile_orphaned_subagents(&pool).await.unwrap();
    assert!(count >= 1);
    let status: String = sqlx::query_scalar(
        "SELECT status FROM run_subagents WHERE parent_run_id = $1 AND tool_invocation_id = 'mcp:orphan'",
    )
    .bind(&ctx.parent_run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "interrupted");
}

#[sqlx::test(migrations = "./migrations")]
async fn helper_does_not_create_bot_conversation_computer_or_queued_run(pool: PgPool) {
    let owner = format!("owner-{}", Uuid::new_v4());
    let bots_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bots")
        .fetch_one(&pool)
        .await
        .unwrap();
    let conv_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM conversations")
        .fetch_one(&pool)
        .await
        .unwrap();
    let computers_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sandboxes")
        .fetch_one(&pool)
        .await
        .unwrap();
    let runs_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs")
        .fetch_one(&pool)
        .await
        .unwrap();
    let queue_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM work_queue")
        .fetch_one(&pool)
        .await
        .unwrap();

    let (ctx, service) = seed_parent(&pool, &owner).await;
    service.attach_turn_executor(Arc::new(CountingTurn {
        calls: AtomicUsize::new(0),
        delay: Duration::from_millis(0),
        text: "notes".into(),
    }));
    service
        .run_subagent(
            &ctx,
            SubagentRequest {
                name: "Research".into(),
                task: "summarize".into(),
                context: Some("only this snippet".into()),
            },
        )
        .await
        .unwrap();

    let bots_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bots")
        .fetch_one(&pool)
        .await
        .unwrap();
    let conv_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM conversations")
        .fetch_one(&pool)
        .await
        .unwrap();
    let computers_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sandboxes")
        .fetch_one(&pool)
        .await
        .unwrap();
    let runs_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs")
        .fetch_one(&pool)
        .await
        .unwrap();
    let queue_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM work_queue")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(bots_after, bots_before + 1);
    assert_eq!(computers_after, computers_before + 1);
    assert_eq!(runs_after, runs_before + 1);
    assert_eq!(conv_after, conv_before + 1);
    assert_eq!(queue_after, queue_before + 1);
    let helpers: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM run_subagents WHERE parent_run_id = $1")
            .bind(&ctx.parent_run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(helpers, 1);
    let started: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM run_events WHERE request_id = $1 AND event_type = 'subagent_started'",
    )
    .bind(&ctx.parent_request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let completed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM run_events WHERE request_id = $1 AND event_type = 'subagent_completed'",
    )
    .bind(&ctx.parent_request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(started, 1);
    assert_eq!(completed, 1);
}
