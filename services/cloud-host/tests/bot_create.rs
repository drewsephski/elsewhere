use agent_core::{CollaborationContext, CollaborationError, MAX_AGENT_CREATED_BOTS_PER_ROOT};
use cloud_host::{
    bot_create,
    db::{queries::BootstrapRunRecords, resources},
    delegation, run_lifecycle, work,
};
use sqlx::PgPool;
use uuid::Uuid;

async fn bot_with_computer(pool: &PgPool, owner: &str, name: &str) -> resources::BotRow {
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
        "codex",
        "sky-wisp",
    )
    .await
    .unwrap()
}

async fn enqueue_source(
    pool: &PgPool,
    owner: &str,
    bot: &resources::BotRow,
    message: &str,
) -> BootstrapRunRecords {
    work::enqueue(
        pool,
        owner,
        &Uuid::new_v4().to_string(),
        &bot.id,
        None,
        message,
    )
    .await
    .unwrap()
}

fn ctx_for(
    owner: &str,
    bot: &resources::BotRow,
    source: &BootstrapRunRecords,
    invoke: &str,
) -> CollaborationContext {
    CollaborationContext {
        owner_id: owner.into(),
        source_bot_id: bot.id.clone(),
        source_run_id: source.run_id.clone(),
        source_conversation_id: source.conversation_id.clone(),
        source_request_id: source.request_id.clone(),
        tool_invocation_id: invoke.into(),
    }
}

async fn finalize_target_results(pool: &PgPool, target_run_id: &str) {
    sqlx::query(
        r#"
        UPDATE agent_runs
        SET results_status = 'complete',
            results_finalized_at = NOW(),
            execution_released_at = COALESCE(execution_released_at, NOW())
        WHERE id = $1
        "#,
    )
    .bind(target_run_id)
    .execute(pool)
    .await
    .unwrap();
    run_lifecycle::on_target_results_finalized(pool, target_run_id)
        .await
        .unwrap();
}

async fn bot_count(pool: &PgPool, owner: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM bots WHERE owner_id = $1")
        .bind(owner)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn creation_count(pool: &PgPool, root_run_id: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM bot_creations WHERE root_run_id = $1")
        .bind(root_run_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn allowed_create_inherits_owner_model_engine_and_computer(pool: PgPool) {
    let owner = "alice";
    let source_bot = bot_with_computer(&pool, owner, "Chief").await;
    let source = enqueue_source(&pool, owner, &source_bot, "Plan").await;
    let ctx = ctx_for(owner, &source_bot, &source, "create-1");
    let created = bot_create::create_bot(&pool, &ctx, "Researcher", "Find primary sources", None)
        .await
        .unwrap();

    let row = resources::get_bot_for_owner(&pool, owner, &created.bot_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.owner_id, owner);
    assert_eq!(row.model, source_bot.model);
    assert_eq!(row.engine_preference, source_bot.engine_preference);
    assert_eq!(
        row.computer_id.as_deref(),
        source_bot.computer_id.as_deref()
    );
    assert_eq!(row.avatar_id, "sky-wisp");
    assert_eq!(created.name, "Researcher");
    assert_eq!(created.computer_id, source_bot.computer_id.clone().unwrap());

    let listed = delegation::list_teammates(&pool, owner, &source_bot.id)
        .await
        .unwrap();
    assert!(listed
        .iter()
        .any(|bot| bot.id == created.bot_id && bot.name == "Researcher"));
}

#[sqlx::test(migrations = "./migrations")]
async fn owner_isolation_rejects_foreign_run_and_hides_created_bot(pool: PgPool) {
    let alice = bot_with_computer(&pool, "alice", "Chief").await;
    let source = enqueue_source(&pool, "alice", &alice, "Plan").await;
    let created = bot_create::create_bot(
        &pool,
        &ctx_for("alice", &alice, &source, "create-alice"),
        "Researcher",
        "Find sources",
        None,
    )
    .await
    .unwrap();

    let bob = bot_with_computer(&pool, "bob", "Other").await;
    let err = bot_create::create_bot(
        &pool,
        &CollaborationContext {
            owner_id: "bob".into(),
            source_bot_id: bob.id.clone(),
            source_run_id: source.run_id.clone(),
            source_conversation_id: source.conversation_id.clone(),
            source_request_id: source.request_id.clone(),
            tool_invocation_id: "create-bob".into(),
        },
        "Spy",
        "Should not work",
        None,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        err,
        CollaborationError::NotFound | CollaborationError::Validation(_)
    ));

    let bob_list = delegation::list_teammates(&pool, "bob", &bob.id)
        .await
        .unwrap();
    assert!(!bob_list.iter().any(|bot| bot.id == created.bot_id));
}

#[sqlx::test(migrations = "./migrations")]
async fn exact_retry_returns_same_bot(pool: PgPool) {
    let owner = "alice";
    let source_bot = bot_with_computer(&pool, owner, "Chief").await;
    let source = enqueue_source(&pool, owner, &source_bot, "Plan").await;
    let ctx = ctx_for(owner, &source_bot, &source, "same-invoke");
    let first = bot_create::create_bot(&pool, &ctx, "Researcher", "Find sources", None)
        .await
        .unwrap();
    let second = bot_create::create_bot(&pool, &ctx, "Researcher", "Find sources", None)
        .await
        .unwrap();
    assert_eq!(first.bot_id, second.bot_id);
    assert_eq!(creation_count(&pool, &source.run_id).await, 1);
    assert_eq!(bot_count(&pool, owner).await, 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn same_invocation_different_args_conflicts(pool: PgPool) {
    let owner = "alice";
    let source_bot = bot_with_computer(&pool, owner, "Chief").await;
    let source = enqueue_source(&pool, owner, &source_bot, "Plan").await;
    let ctx = ctx_for(owner, &source_bot, &source, "conflict-invoke");
    let first = bot_create::create_bot(&pool, &ctx, "Researcher", "Find sources", None)
        .await
        .unwrap();
    let err = bot_create::create_bot(&pool, &ctx, "Writer", "Write the brief", None)
        .await
        .unwrap_err();
    assert!(matches!(err, CollaborationError::Conflict(_)));
    assert_eq!(creation_count(&pool, &source.run_id).await, 1);
    assert_eq!(bot_count(&pool, owner).await, 2);
    let still = resources::get_bot_for_owner(&pool, owner, &first.bot_id)
        .await
        .unwrap();
    assert!(still.is_some());
}

#[sqlx::test(migrations = "./migrations")]
async fn hard_limit_rejects_over_max(pool: PgPool) {
    let owner = "alice";
    let source_bot = bot_with_computer(&pool, owner, "Chief").await;
    let source = enqueue_source(&pool, owner, &source_bot, "Plan").await;
    for i in 0..MAX_AGENT_CREATED_BOTS_PER_ROOT {
        let ctx = ctx_for(owner, &source_bot, &source, &format!("limit-{i}"));
        bot_create::create_bot(&pool, &ctx, &format!("Bot {i}"), "Specialist", None)
            .await
            .unwrap();
    }
    let err = bot_create::create_bot(
        &pool,
        &ctx_for(owner, &source_bot, &source, "limit-over"),
        "Overflow",
        "One more",
        None,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, CollaborationError::LimitExceeded(_)));
    assert_eq!(
        creation_count(&pool, &source.run_id).await,
        MAX_AGENT_CREATED_BOTS_PER_ROOT
    );
    assert_eq!(
        bot_count(&pool, owner).await,
        1 + MAX_AGENT_CREATED_BOTS_PER_ROOT
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn missing_computer_creates_zero_bots(pool: PgPool) {
    let owner = "alice";
    let source_bot = bot_with_computer(&pool, owner, "Chief").await;
    let source = enqueue_source(&pool, owner, &source_bot, "Plan").await;
    sqlx::query("UPDATE bots SET computer_id = NULL WHERE id = $1")
        .bind(&source_bot.id)
        .execute(&pool)
        .await
        .unwrap();
    let before = bot_count(&pool, owner).await;
    let err = bot_create::create_bot(
        &pool,
        &ctx_for(owner, &source_bot, &source, "no-computer"),
        "Researcher",
        "Find sources",
        None,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, CollaborationError::Validation(_)));
    assert_eq!(bot_count(&pool, owner).await, before);
    assert_eq!(creation_count(&pool, &source.run_id).await, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn archived_computer_creates_zero_bots(pool: PgPool) {
    let owner = "alice";
    let source_bot = bot_with_computer(&pool, owner, "Chief").await;
    let source = enqueue_source(&pool, owner, &source_bot, "Plan").await;
    sqlx::query("UPDATE sandboxes SET state = 'archived' WHERE id = $1")
        .bind(source_bot.computer_id.as_ref().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let before = bot_count(&pool, owner).await;
    let err = bot_create::create_bot(
        &pool,
        &ctx_for(owner, &source_bot, &source, "archived-computer"),
        "Researcher",
        "Find sources",
        None,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, CollaborationError::Validation(_)));
    assert_eq!(bot_count(&pool, owner).await, before);
    assert_eq!(creation_count(&pool, &source.run_id).await, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn team_loop_create_list_delegate_and_resume_once(pool: PgPool) {
    let owner = "alice";
    let chief = bot_with_computer(&pool, owner, "Chief").await;
    let source = enqueue_source(&pool, owner, &chief, "Coordinate research").await;
    let created = bot_create::create_bot(
        &pool,
        &ctx_for(owner, &chief, &source, "create-researcher"),
        "Researcher",
        "Find primary sources",
        None,
    )
    .await
    .unwrap();

    let listed = delegation::list_teammates(&pool, owner, &chief.id)
        .await
        .unwrap();
    assert!(listed.iter().any(|bot| bot.name == "Researcher"));

    let delegated = delegation::create_delegation(
        &pool,
        &ctx_for(owner, &chief, &source, "delegate-researcher"),
        &created.bot_id,
        "Investigate the brief",
        None,
        "resume_source",
    )
    .await
    .unwrap();

    let assistant_id: String =
        sqlx::query_scalar("SELECT assistant_message_id FROM agent_runs WHERE id = $1")
            .bind(&delegated.target_run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "UPDATE messages SET body = 'The brief has three sources.', status = 'complete' WHERE id = $1",
    )
    .bind(&assistant_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE agent_runs SET status = 'completed', finished_at = NOW() WHERE id = $1")
        .bind(&delegated.target_run_id)
        .execute(&pool)
        .await
        .unwrap();
    finalize_target_results(&pool, &delegated.target_run_id).await;
    let target_request: String =
        sqlx::query_scalar("SELECT request_id FROM agent_runs WHERE id = $1")
            .bind(&delegated.target_run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    delegation::sync_target_run_terminal(&pool, &target_request, "completed", None)
        .await
        .unwrap();

    let resumes: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, request_id FROM agent_runs WHERE request_id LIKE 'delegation-return:%'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(resumes.len(), 1);
    assert!(resumes[0].1.starts_with("delegation-return:"));

    let message: String =
        sqlx::query_scalar("SELECT user_message FROM work_queue WHERE run_id = $1")
            .bind(&resumes[0].0)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(message.contains("Investigate the brief"));
    assert!(message.contains("Recipient status:"));
    assert!(message.contains("completed"));
    assert!(message.contains("Recipient result:"));
    assert!(message.contains("The brief has three sources."));
    assert!(message.contains(&delegated.target_run_id));
}
