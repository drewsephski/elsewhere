use agent_core::CollaborationContext;
use cloud_host::db::resources;
use cloud_host::error::ApiError;
use cloud_host::{delegation, groups, work};
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

#[sqlx::test(migrations = "./migrations")]
async fn ordinary_bot_with_completed_direct_work_deletes(pool: PgPool) {
    let bot = bot_with_computer(&pool, "alice", "Scout").await;
    let run = work::enqueue(
        &pool,
        "alice",
        &Uuid::new_v4().to_string(),
        &bot.id,
        None,
        "Hello",
    )
    .await
    .unwrap();
    sqlx::query("UPDATE agent_runs SET status = 'completed', finished_at = NOW() WHERE id = $1")
        .bind(&run.run_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM work_queue WHERE run_id = $1")
        .bind(&run.run_id)
        .execute(&pool)
        .await
        .unwrap();

    assert!(resources::delete_bot(&pool, "alice", &bot.id)
        .await
        .unwrap());
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bots WHERE id = $1")
        .bind(&bot.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn active_run_blocks_delete(pool: PgPool) {
    let bot = bot_with_computer(&pool, "alice", "Scout").await;
    work::enqueue(
        &pool,
        "alice",
        &Uuid::new_v4().to_string(),
        &bot.id,
        None,
        "Hello",
    )
    .await
    .unwrap();

    let err = resources::delete_bot(&pool, "alice", &bot.id)
        .await
        .unwrap_err();
    match err {
        ApiError::Conflict(message) => {
            assert_eq!(message, "Stop this Bot's active work before deleting it.");
        }
        other => panic!("expected conflict, got {other:?}"),
    }
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bots WHERE id = $1")
        .bind(&bot.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, 1);
}

async fn complete_delegation(pool: &PgPool, target_run_id: &str, delegation_id: &str) {
    sqlx::query("UPDATE agent_runs SET status = 'completed', finished_at = NOW() WHERE id = $1")
        .bind(target_run_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM work_queue WHERE run_id = $1")
        .bind(target_run_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE bot_delegations SET status = 'completed', resume_status = 'skipped' WHERE id = $1",
    )
    .bind(delegation_id)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn completed_delegation_source_can_be_deleted(pool: PgPool) {
    let source = bot_with_computer(&pool, "alice", "Chief").await;
    let target = bot_with_computer(&pool, "alice", "Researcher").await;
    let source_run = work::enqueue(
        &pool,
        "alice",
        &Uuid::new_v4().to_string(),
        &source.id,
        None,
        "Plan",
    )
    .await
    .unwrap();
    sqlx::query("UPDATE agent_runs SET status = 'completed', finished_at = NOW() WHERE id = $1")
        .bind(&source_run.run_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM work_queue WHERE run_id = $1")
        .bind(&source_run.run_id)
        .execute(&pool)
        .await
        .unwrap();
    let created = delegation::create_delegation(
        &pool,
        &CollaborationContext {
            owner_id: "alice".into(),
            source_bot_id: source.id.clone(),
            source_run_id: source_run.run_id,
            source_conversation_id: source_run.conversation_id,
            source_request_id: source_run.request_id,
            tool_invocation_id: "src-del".into(),
        },
        &target.id,
        "Investigate",
        None,
        "none",
    )
    .await
    .unwrap();
    complete_delegation(&pool, &created.target_run_id, &created.delegation_id).await;

    assert!(resources::delete_bot(&pool, "alice", &source.id)
        .await
        .unwrap());
}

#[sqlx::test(migrations = "./migrations")]
async fn completed_delegation_target_can_be_deleted(pool: PgPool) {
    let source = bot_with_computer(&pool, "alice", "Chief").await;
    let target = bot_with_computer(&pool, "alice", "Researcher").await;
    let source_run = work::enqueue(
        &pool,
        "alice",
        &Uuid::new_v4().to_string(),
        &source.id,
        None,
        "Plan",
    )
    .await
    .unwrap();
    sqlx::query("UPDATE agent_runs SET status = 'completed', finished_at = NOW() WHERE id = $1")
        .bind(&source_run.run_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM work_queue WHERE run_id = $1")
        .bind(&source_run.run_id)
        .execute(&pool)
        .await
        .unwrap();
    let created = delegation::create_delegation(
        &pool,
        &CollaborationContext {
            owner_id: "alice".into(),
            source_bot_id: source.id.clone(),
            source_run_id: source_run.run_id,
            source_conversation_id: source_run.conversation_id,
            source_request_id: source_run.request_id,
            tool_invocation_id: "tgt-del".into(),
        },
        &target.id,
        "Investigate",
        None,
        "none",
    )
    .await
    .unwrap();
    complete_delegation(&pool, &created.target_run_id, &created.delegation_id).await;

    assert!(resources::delete_bot(&pool, "alice", &target.id)
        .await
        .unwrap());
}

#[sqlx::test(migrations = "./migrations")]
async fn two_bot_group_blocks_delete(pool: PgPool) {
    let a = bot_with_computer(&pool, "alice", "A").await;
    let b = bot_with_computer(&pool, "alice", "B").await;
    groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Pair".into(),
            bot_ids: vec![a.id.clone(), b.id.clone()],
        },
    )
    .await
    .unwrap();

    let err = resources::delete_bot(&pool, "alice", &a.id)
        .await
        .unwrap_err();
    match err {
        ApiError::Conflict(message) => {
            assert!(message.contains("group"));
        }
        other => panic!("expected conflict, got {other:?}"),
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn group_membership_and_authored_message_survive_delete(pool: PgPool) {
    let a = bot_with_computer(&pool, "alice", "A").await;
    let b = bot_with_computer(&pool, "alice", "B").await;
    let c = bot_with_computer(&pool, "alice", "C").await;
    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Trio".into(),
            bot_ids: vec![a.id.clone(), b.id.clone(), c.id.clone()],
        },
    )
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO messages (
            id, conversation_id, role, body, status, sequence, author_kind, author_bot_id
        ) VALUES ($1, $2, 'assistant', 'Prior research', 'complete', 1, 'bot', $3)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&group.id)
    .bind(&a.id)
    .execute(&pool)
    .await
    .unwrap();

    assert!(resources::delete_bot(&pool, "alice", &a.id).await.unwrap());

    let remaining_participants: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM conversation_participants WHERE conversation_id = $1 AND left_at IS NULL",
    )
    .bind(&group.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining_participants, 2);

    let (body, author_bot_id): (String, Option<String>) =
        sqlx::query_as("SELECT body, author_bot_id FROM messages WHERE conversation_id = $1")
            .bind(&group.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(body, "Prior research");
    assert!(author_bot_id.is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn channel_default_and_thread_do_not_block_delete(pool: PgPool) {
    let bot = bot_with_computer(&pool, "alice", "SlackBot").await;
    let conversation_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO conversations (id, owner_id, bot_id, conversation_type, origin_kind)
        VALUES ($1, 'alice', $2, 'direct', 'channel')
        "#,
    )
    .bind(&conversation_id)
    .bind(&bot.id)
    .execute(&pool)
    .await
    .unwrap();
    let connection_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO channel_connections (
            id, owner_id, provider, status, default_bot_id
        ) VALUES ($1, 'alice', 'slack', 'connected', $2)
        "#,
    )
    .bind(&connection_id)
    .bind(&bot.id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO channel_oauth_states (state, owner_id, provider, bot_id, expires_at)
        VALUES ('oauth-state', 'alice', 'slack', $1, NOW() + INTERVAL '10 minutes')
        "#,
    )
    .bind(&bot.id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO channel_threads (
            id, connection_id, owner_id, bot_id, provider,
            external_channel_id, external_thread_id, conversation_id
        ) VALUES ($1, $2, 'alice', $3, 'slack', 'C1', 'T1', $4)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&connection_id)
    .bind(&bot.id)
    .bind(&conversation_id)
    .execute(&pool)
    .await
    .unwrap();

    assert!(resources::delete_bot(&pool, "alice", &bot.id)
        .await
        .unwrap());

    let default_bot: Option<String> =
        sqlx::query_scalar("SELECT default_bot_id FROM channel_connections WHERE id = $1")
            .bind(&connection_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(default_bot.is_none());
    let oauth: Option<String> =
        sqlx::query_scalar("SELECT state FROM channel_oauth_states WHERE state = 'oauth-state'")
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert!(oauth.is_none());
}
