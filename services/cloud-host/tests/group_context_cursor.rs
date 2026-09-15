use agent_core::{RunStore, DEFAULT_MODEL};
use cloud_host::{
    conversation, db::postgres_run_store::PostgresRunStore, db::resources, group_context, work,
};
use sqlx::PgPool;
use uuid::Uuid;

async fn setup_group_run(
    pool: &PgPool,
    owner: &str,
    through_sequence: i64,
) -> (String, String, String, String) {
    let bot_a = bot(pool, owner, "A").await;
    let bot_b = bot(pool, owner, "B").await;
    let conv = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO conversations (id, owner_id, bot_id, conversation_type, name) VALUES ($1,$2,$3,'group','G')",
    )
    .bind(&conv)
    .bind(owner)
    .bind(&bot_a.id)
    .execute(pool)
    .await
    .unwrap();
    for (bot_id, ord) in [(&bot_a.id, 0i32), (&bot_b.id, 1i32)] {
        sqlx::query(
            "INSERT INTO conversation_participants (conversation_id, bot_id, owner_id, ordinal) VALUES ($1,$2,$3,$4)",
        )
        .bind(&conv)
        .bind(bot_id)
        .bind(owner)
        .bind(ord)
        .execute(pool)
        .await
        .unwrap();
    }
    let human_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO messages (id, conversation_id, role, body, status, sequence, author_kind) VALUES ($1,$2,'user','@A hi', 'complete', 1, 'human')",
    )
    .bind(&human_id)
    .bind(&conv)
    .execute(pool)
    .await
    .unwrap();
    let bot_msg = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO messages (id, conversation_id, role, body, status, sequence, author_kind, author_bot_id) VALUES ($1,$2,'assistant','peer context', 'complete', 2, 'bot', $3)",
    )
    .bind(&bot_msg)
    .bind(&conv)
    .bind(&bot_b.id)
    .execute(pool)
    .await
    .unwrap();

    let run = work::enqueue(
        pool,
        owner,
        &Uuid::new_v4().to_string(),
        &bot_a.id,
        Some(&conv),
        "follow up",
    )
    .await
    .unwrap();
    conversation::record_pending_group_context_boundary(pool, &run.run_id, through_sequence)
        .await
        .unwrap();
    (conv, bot_a.id, run.run_id, run.request_id)
}

async fn bot(pool: &PgPool, owner: &str, name: &str) -> resources::BotRow {
    let computer_id = resources::insert_computer_placeholder(pool, owner, "C")
        .await
        .unwrap()
        .id;
    resources::insert_bot(
        pool,
        owner,
        name,
        "role",
        DEFAULT_MODEL,
        Some(&computer_id),
        "codex",
        "sky-wisp",
    )
    .await
    .unwrap()
}

async fn cursor(pool: &PgPool, conv: &str, bot: &str) -> i64 {
    group_context::get_last_seen_group_sequence(pool, conv, bot)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn cursor_unchanged_on_non_completed_terminal_states(pool: PgPool) {
    let (conv, bot_id, _run_id, request_id) = setup_group_run(&pool, "alice", 2).await;
    let before = cursor(&pool, &conv, &bot_id).await;
    let store = PostgresRunStore::new(pool.clone());

    for status in ["failed", "cancelled", "interrupted"] {
        store
            .update_run(&request_id, status, Some("test"), 0)
            .await
            .unwrap();
        assert_eq!(cursor(&pool, &conv, &bot_id).await, before);
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn cursor_advances_only_on_successful_completion(pool: PgPool) {
    let (conv, bot_id, _run_id, request_id) = setup_group_run(&pool, "alice", 2).await;
    let before = cursor(&pool, &conv, &bot_id).await;
    let store = PostgresRunStore::new(pool.clone());
    store
        .update_run(&request_id, "completed", None, 1)
        .await
        .unwrap();
    assert_eq!(cursor(&pool, &conv, &bot_id).await, 2);
    assert!(before < 2);
}
