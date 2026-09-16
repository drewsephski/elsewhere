use agent_core::{CollaborationContext, RunStore};
use cloud_host::{
    db::postgres_run_store::PostgresRunStore, db::resources, delegation, run_lifecycle, work,
};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::Barrier;
use uuid::Uuid;

async fn chief_and_researcher(
    pool: &PgPool,
    owner: &str,
) -> (resources::BotRow, resources::BotRow) {
    let chief = bot_with_computer(pool, owner, "Chief").await;
    let researcher = bot_with_computer(pool, owner, "Researcher").await;
    (chief, researcher)
}

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

async fn delegate_resume(
    pool: &PgPool,
    owner: &str,
    chief: &resources::BotRow,
    researcher: &resources::BotRow,
    invoke: &str,
) -> (String, String, String) {
    let source = work::enqueue(
        pool,
        owner,
        &Uuid::new_v4().to_string(),
        &chief.id,
        None,
        "Plan",
    )
    .await
    .unwrap();
    let ctx = CollaborationContext {
        owner_id: owner.into(),
        source_bot_id: chief.id.clone(),
        source_run_id: source.run_id.clone(),
        source_conversation_id: source.conversation_id.clone(),
        source_request_id: source.request_id.clone(),
        tool_invocation_id: invoke.into(),
    };
    let created = delegation::create_delegation(
        pool,
        &ctx,
        &researcher.id,
        "Investigate",
        None,
        "resume_source",
    )
    .await
    .unwrap();
    let target_request: String =
        sqlx::query_scalar("SELECT request_id FROM agent_runs WHERE id = $1")
            .bind(&created.target_run_id)
            .fetch_one(pool)
            .await
            .unwrap();
    (created.delegation_id, created.target_run_id, target_request)
}

#[sqlx::test(migrations = "./migrations")]
async fn boundary_a_target_run_terminal_before_delegation_sync(pool: PgPool) {
    let (chief, researcher) = chief_and_researcher(&pool, "alice").await;
    let (delegation_id, target_run_id, _source_request) =
        delegate_resume(&pool, "alice", &chief, &researcher, "boundary-a").await;

    sqlx::query("UPDATE agent_runs SET status = 'completed', finished_at = NOW() WHERE id = $1")
        .bind(&target_run_id)
        .execute(&pool)
        .await
        .unwrap();
    finalize_target_results(&pool, &target_run_id).await;

    let stale: String = sqlx::query_scalar("SELECT status FROM bot_delegations WHERE id = $1")
        .bind(&delegation_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stale, "queued");

    run_lifecycle::reconcile_collaboration_lifecycle(&pool)
        .await
        .unwrap();

    let synced: String = sqlx::query_scalar("SELECT status FROM bot_delegations WHERE id = $1")
        .bind(&delegation_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(synced, "completed");

    let resume_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_runs WHERE request_id LIKE 'delegation-return:%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(resume_count, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn boundary_b_delegation_terminal_resume_missing(pool: PgPool) {
    let (chief, researcher) = chief_and_researcher(&pool, "alice").await;
    let (delegation_id, target_run_id, target_request) =
        delegate_resume(&pool, "alice", &chief, &researcher, "boundary-b").await;

    sqlx::query("UPDATE agent_runs SET status = 'completed', finished_at = NOW() WHERE id = $1")
        .bind(&target_run_id)
        .execute(&pool)
        .await
        .unwrap();
    finalize_target_results(&pool, &target_run_id).await;

    delegation::sync_target_run_terminal(&pool, &target_request, "completed", None)
        .await
        .unwrap();
    let resume_run: Option<String> =
        sqlx::query_scalar("SELECT source_resume_run_id FROM bot_delegations WHERE id = $1")
            .bind(&delegation_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    if let Some(resume_run) = resume_run {
        sqlx::query(
            "UPDATE bot_delegations SET source_resume_run_id = NULL, resume_status = NULL WHERE id = $1",
        )
        .bind(&delegation_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("DELETE FROM work_queue WHERE run_id = $1")
            .bind(&resume_run)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM agent_runs WHERE id = $1")
            .bind(&resume_run)
            .execute(&pool)
            .await
            .unwrap();
    }

    run_lifecycle::reconcile_collaboration_lifecycle(&pool)
        .await
        .unwrap();

    let resume: Option<String> =
        sqlx::query_scalar("SELECT source_resume_run_id FROM bot_delegations WHERE id = $1")
            .bind(&delegation_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(resume.is_some());

    let dup_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_runs WHERE request_id LIKE 'delegation-return:%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(dup_count, 1);
    let _ = target_run_id;
}

#[sqlx::test(migrations = "./migrations")]
async fn boundary_d_concurrent_reconciliation_one_resume(pool: PgPool) {
    let (chief, researcher) = chief_and_researcher(&pool, "alice").await;
    let (_delegation_id, _target_run_id, target_request) =
        delegate_resume(&pool, "alice", &chief, &researcher, "boundary-d").await;

    let target_run_id: String = sqlx::query_scalar(
        "SELECT target_run_id FROM bot_delegations WHERE tool_invocation_id = 'boundary-d'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE agent_runs SET status = 'completed' WHERE request_id = $1")
        .bind(&target_request)
        .execute(&pool)
        .await
        .unwrap();
    finalize_target_results(&pool, &target_run_id).await;

    let barrier = Arc::new(Barrier::new(2));
    let pool_a = pool.clone();
    let pool_b = pool.clone();
    let barrier_b = barrier.clone();
    let first = tokio::spawn(async move {
        barrier.wait().await;
        run_lifecycle::reconcile_collaboration_lifecycle(&pool_a)
            .await
            .unwrap();
    });
    let second = tokio::spawn(async move {
        barrier_b.wait().await;
        run_lifecycle::reconcile_collaboration_lifecycle(&pool_b)
            .await
            .unwrap();
    });
    first.await.unwrap();
    second.await.unwrap();

    let resume_runs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_runs WHERE bot_id = $1 AND request_id LIKE 'delegation-return:%'",
    )
    .bind(&chief.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(resume_runs, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn boundary_e_restart_interrupted_target_wakes_source_once(pool: PgPool) {
    let (chief, researcher) = chief_and_researcher(&pool, "alice").await;
    let (delegation_id, target_run_id, _) =
        delegate_resume(&pool, "alice", &chief, &researcher, "boundary-e").await;

    sqlx::query("UPDATE agent_runs SET status = 'running', started_at = NOW() WHERE id = $1")
        .bind(&target_run_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE bot_delegations SET status = 'running' WHERE id = $1")
        .bind(&delegation_id)
        .execute(&pool)
        .await
        .unwrap();

    cloud_host::db::queries::mark_interrupted_runs(&pool)
        .await
        .unwrap();
    run_lifecycle::reconcile_collaboration_lifecycle(&pool)
        .await
        .unwrap();

    let resume_runs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_runs WHERE bot_id = $1 AND request_id LIKE 'delegation-return:%'",
    )
    .bind(&chief.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(resume_runs, 1);

    let message: String = sqlx::query_scalar(
        "SELECT user_message FROM work_queue WHERE run_id = (SELECT source_resume_run_id FROM bot_delegations WHERE id = $1)",
    )
    .bind(&delegation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(message.contains("runner restart"));
}

#[sqlx::test(migrations = "./migrations")]
async fn resume_lifecycle_queued_running_completed(pool: PgPool) {
    let (chief, researcher) = chief_and_researcher(&pool, "alice").await;
    let (delegation_id, target_run_id, target_request) =
        delegate_resume(&pool, "alice", &chief, &researcher, "resume-life").await;

    sqlx::query("UPDATE agent_runs SET status = 'completed', finished_at = NOW() WHERE id = $1")
        .bind(&target_run_id)
        .execute(&pool)
        .await
        .unwrap();
    finalize_target_results(&pool, &target_run_id).await;

    delegation::sync_target_run_terminal(&pool, &target_request, "completed", None)
        .await
        .unwrap();
    let resume_run: String = sqlx::query_scalar::<_, Option<String>>(
        "SELECT source_resume_run_id FROM bot_delegations WHERE id = $1",
    )
    .bind(&delegation_id)
    .fetch_one(&pool)
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT resume_status FROM bot_delegations WHERE id = $1"
        )
        .bind(&delegation_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        Some("queued".into())
    );

    run_lifecycle::on_delegation_return_run_claimed(&pool, &resume_run)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT resume_status FROM bot_delegations WHERE id = $1")
            .bind(&delegation_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "running"
    );

    let store = PostgresRunStore::new(pool.clone());
    let request_id: String = sqlx::query_scalar("SELECT request_id FROM agent_runs WHERE id = $1")
        .bind(&resume_run)
        .fetch_one(&pool)
        .await
        .unwrap();
    store
        .update_run(&request_id, "completed", None, 1)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT resume_status FROM bot_delegations WHERE id = $1")
            .bind(&delegation_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "completed"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn group_recipient_stale_lifecycle_repaired(pool: PgPool) {
    let bot = bot_with_computer(&pool, "alice", "Designer").await;
    let conv_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO conversations (id, owner_id, bot_id, conversation_type, name) VALUES ($1, 'alice', $2, 'group', 'Team')",
    )
    .bind(&conv_id)
    .bind(&bot.id)
    .execute(&pool)
    .await
    .unwrap();

    let message_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO messages (id, conversation_id, role, body, status, sequence, author_kind) VALUES ($1,$2,'user','@Designer hi','complete',1,'human')",
    )
    .bind(&message_id)
    .bind(&conv_id)
    .execute(&pool)
    .await
    .unwrap();

    let assistant_message_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO messages (id, conversation_id, role, body, status, sequence, model, author_kind, author_bot_id) VALUES ($1,$2,'assistant','','pending',2,$3,'bot',$4)",
    )
    .bind(&assistant_message_id)
    .bind(&conv_id)
    .bind(bot.model.clone())
    .bind(&bot.id)
    .execute(&pool)
    .await
    .unwrap();

    let run_id = Uuid::new_v4().to_string();
    let request_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO agent_runs (
            id, owner_id, request_id, bot_id, conversation_id, computer_id, model,
            status, assistant_message_id, source_message_id, finished_at
        ) VALUES ($1,'alice',$2,$3,$4,$5,$6,'completed',$7,$8,NOW())
        "#,
    )
    .bind(&run_id)
    .bind(&request_id)
    .bind(&bot.id)
    .bind(&conv_id)
    .bind(bot.computer_id.as_deref().unwrap())
    .bind(&bot.model)
    .bind(&assistant_message_id)
    .bind(&message_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO group_message_recipients (message_id, conversation_id, bot_id, run_id, routing_kind, status)
        VALUES ($1, $2, $3, $4, 'mention', 'running')
        "#,
    )
    .bind(&message_id)
    .bind(&conv_id)
    .bind(&bot.id)
    .bind(&run_id)
    .execute(&pool)
    .await
    .unwrap();

    run_lifecycle::reconcile_collaboration_lifecycle(&pool)
        .await
        .unwrap();

    let recipient_status: String =
        sqlx::query_scalar("SELECT status FROM group_message_recipients WHERE run_id = $1")
            .bind(&run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(recipient_status, "completed");
}

#[sqlx::test(migrations = "./migrations")]
async fn artifact_metadata_same_and_different_computer(pool: PgPool) {
    let chief = bot_with_computer(&pool, "alice", "Chief").await;
    let shared_computer = chief.computer_id.clone().unwrap();
    let researcher = resources::insert_bot(
        &pool,
        "alice",
        "Researcher",
        "Role",
        "gpt-5.6-luna",
        Some(&shared_computer),
        "codex",
        "sky-wisp",
    )
    .await
    .unwrap();

    let (_, target_run_id, target_request) =
        delegate_resume(&pool, "alice", &chief, &researcher, "artifacts-shared").await;
    sqlx::query(
        "INSERT INTO work_results (id, run_id, name, kind, content) VALUES ($1,$2,'report.md','file',$3)",
    )
    .bind(Uuid::new_v4())
    .bind(&target_run_id)
    .bind(b"# report".as_slice())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE agent_runs SET status = 'completed' WHERE id = $1")
        .bind(&target_run_id)
        .execute(&pool)
        .await
        .unwrap();
    finalize_target_results(&pool, &target_run_id).await;
    delegation::sync_target_run_terminal(&pool, &target_request, "completed", None)
        .await
        .unwrap();
    run_lifecycle::try_admit_delegation_return(
        &pool,
        &sqlx::query_scalar::<_, String>(
            "SELECT id FROM bot_delegations WHERE tool_invocation_id = 'artifacts-shared'",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
    )
    .await
    .unwrap();
    let shared_msg: String = sqlx::query_scalar(
        "SELECT user_message FROM work_queue WHERE run_id = (SELECT source_resume_run_id FROM bot_delegations WHERE tool_invocation_id = 'artifacts-shared')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(shared_msg.contains("report.md"));
    assert!(shared_msg.contains("same computer"));

    let chief2 = bot_with_computer(&pool, "bob", "ChiefBob").await;
    let researcher2 = bot_with_computer(&pool, "bob", "ResearchBob").await;
    let (_, target2, req2) =
        delegate_resume(&pool, "bob", &chief2, &researcher2, "artifacts-diff").await;
    sqlx::query("UPDATE agent_runs SET status = 'completed' WHERE id = $1")
        .bind(&target2)
        .execute(&pool)
        .await
        .unwrap();
    finalize_target_results(&pool, &target2).await;
    delegation::sync_target_run_terminal(&pool, &req2, "completed", None)
        .await
        .unwrap();
    run_lifecycle::try_admit_delegation_return(
        &pool,
        &sqlx::query_scalar::<_, String>(
            "SELECT id FROM bot_delegations WHERE tool_invocation_id = 'artifacts-diff'",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
    )
    .await
    .unwrap();
    let diff_msg: String = sqlx::query_scalar(
        "SELECT user_message FROM work_queue WHERE run_id = (SELECT source_resume_run_id FROM bot_delegations WHERE tool_invocation_id = 'artifacts-diff')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!diff_msg.contains("same computer"));
    let resume: Option<String> = sqlx::query_scalar(
        "SELECT source_resume_run_id FROM bot_delegations WHERE tool_invocation_id = 'artifacts-diff'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(resume.is_some());
}

#[test]
fn shared_computer_helper_is_server_derived() {
    assert!(run_lifecycle::bots_share_computer("c1", "c1"));
    assert!(!run_lifecycle::bots_share_computer("c1", "c2"));
}
