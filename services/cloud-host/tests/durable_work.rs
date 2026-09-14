use cloud_host::{
    db::{queries, resources},
    work,
};
use sqlx::PgPool;
use uuid::Uuid;

async fn bot(pool: &PgPool, owner: &str, computer: Option<&str>) -> resources::BotRow {
    let computer_id = match computer {
        Some(id) => id.to_string(),
        None => {
            resources::insert_computer_placeholder(pool, owner, "Research computer")
                .await
                .unwrap()
                .id
        }
    };
    resources::insert_bot(
        pool,
        owner,
        "Scout",
        "Keep sources with your findings",
        "gpt-5.6-luna",
        Some(&computer_id),
        "codex",
        "sky-wisp",
    )
    .await
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn admission_is_durable_idempotent_and_owner_scoped(pool: PgPool) {
    let bot = bot(&pool, "alice", None).await;
    let key = Uuid::new_v4().to_string();
    let (a, b) = tokio::join!(
        work::enqueue(&pool, "alice", &key, &bot.id, None, "Research this"),
        work::enqueue(&pool, "alice", &key, &bot.id, None, "Research this")
    );
    let a = a.unwrap();
    assert_eq!(a.run_id, b.unwrap().run_id);
    assert_eq!(
        queries::find_run_by_id(&pool, &a.run_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        "queued"
    );
    assert!(
        work::enqueue(&pool, "bob", &key, &bot.id, None, "Research this")
            .await
            .is_err()
    );
    assert!(
        work::enqueue(&pool, "alice", &key, &bot.id, None, "Different work")
            .await
            .is_err()
    );
    assert!(
        work::enqueue(&pool, "bob", "another", &bot.id, None, "Research this")
            .await
            .is_err()
    );
    let messages: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE conversation_id = $1")
            .bind(&a.conversation_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(messages, 2);
    // A fresh dispatcher reconstructs exactly what was accepted, regardless of later bot edits.
    sqlx::query("UPDATE bots SET system_prompt = 'changed', model = 'changed', engine_preference = 'responses' WHERE id = $1").bind(&bot.id).execute(&pool).await.unwrap();
    let claimed = work::claim_next(&pool).await.unwrap().unwrap();
    assert!(
        claimed.records.instructions.contains("You are \"Scout\""),
        "queued work should snapshot bot identity at admission"
    );
    assert!(claimed.records.instructions.contains("Keep sources with your findings"));
    assert_eq!(claimed.records.model, "gpt-5.6-luna");
    assert_eq!(
        claimed.engine_mode,
        Some(cloud_host::run_engine_select::RunEngineMode::Codex)
    );
    assert_eq!(claimed.user_message, "Research this");
}

#[sqlx::test(migrations = "./migrations")]
async fn computers_serialize_work_and_queued_work_survives_restart(pool: PgPool) {
    let first = bot(&pool, "alice", None).await;
    let second = bot(&pool, "alice", first.computer_id.as_deref()).await;
    let a = work::enqueue(&pool, "alice", "one", &first.id, None, "First task")
        .await
        .unwrap();
    let b = work::enqueue(&pool, "alice", "two", &second.id, None, "Second task")
        .await
        .unwrap();
    let (claim_a, claim_b) = tokio::join!(work::claim_next(&pool), work::claim_next(&pool));
    assert_eq!(
        usize::from(claim_a.unwrap().is_some()) + usize::from(claim_b.unwrap().is_some()),
        1
    );
    assert!(
        resources::archive_computer(&pool, "alice", first.computer_id.as_deref().unwrap())
            .await
            .is_err()
    );
    // In-flight work is interrupted, never replayed. Untouched work survives.
    assert_eq!(queries::mark_interrupted_runs(&pool).await.unwrap(), 1);
    let recovered = work::claim_next(&pool).await.unwrap().unwrap();
    assert_eq!(recovered.records.run_id, b.run_id);
    assert_eq!(
        queries::find_run_by_id(&pool, &a.run_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        "interrupted"
    );
    assert!(work::claim_next(&pool).await.unwrap().is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn cancellation_prevents_dispatch_and_is_owner_scoped(pool: PgPool) {
    let bot = bot(&pool, "alice", None).await;
    let run = work::enqueue(&pool, "alice", "cancel", &bot.id, None, "Task")
        .await
        .unwrap();
    assert!(work::request_cancel(&pool, "bob", &run.run_id)
        .await
        .is_err());
    work::request_cancel(&pool, "alice", &run.run_id)
        .await
        .unwrap();
    assert!(work::claim_next(&pool).await.unwrap().is_none());
    assert_eq!(
        queries::find_run_by_id(&pool, &run.run_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        "cancelled"
    );
    let other = work::enqueue(&pool, "alice", "running-cancel", &bot.id, None, "Task")
        .await
        .unwrap();
    work::claim_next(&pool).await.unwrap().unwrap();
    work::request_cancel(&pool, "alice", &other.run_id)
        .await
        .unwrap();
    let requested: bool = sqlx::query_scalar("SELECT cancel_requested FROM agent_runs WHERE id=$1")
        .bind(&other.run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(requested);
}

#[sqlx::test(migrations = "./migrations")]
async fn archived_or_foreign_computers_cannot_accept_work(pool: PgPool) {
    let bot = bot(&pool, "alice", None).await;
    resources::archive_computer(&pool, "alice", bot.computer_id.as_deref().unwrap())
        .await
        .unwrap();
    assert!(
        work::enqueue(&pool, "alice", "archived", &bot.id, None, "Task")
            .await
            .is_err()
    );
    sqlx::query("UPDATE sandboxes SET state='active', owner_id='bob' WHERE id=$1")
        .bind(&bot.computer_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        work::enqueue(&pool, "alice", "foreign", &bot.id, None, "Task")
            .await
            .is_err()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn follow_up_messages_reuse_primary_conversation(pool: PgPool) {
    let bot = bot(&pool, "alice", None).await;
    let first = work::enqueue(&pool, "alice", "turn-one", &bot.id, None, "First message")
        .await
        .unwrap();
    let second = work::enqueue(&pool, "alice", "turn-two", &bot.id, None, "Second message")
        .await
        .unwrap();
    assert_eq!(first.conversation_id, second.conversation_id);
    let conversation_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM conversations WHERE bot_id = $1 AND owner_id = $2")
            .bind(&bot.id)
            .bind("alice")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(conversation_count, 1);
    let message_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE conversation_id = $1")
            .bind(&first.conversation_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(message_count, 4);
}

#[sqlx::test(migrations = "./migrations")]
async fn only_one_runner_can_recover_and_dispatch_a_database(pool: PgPool) {
    use sqlx::Connection;
    let mut url = reqwest::Url::parse(&std::env::var("DATABASE_URL").unwrap()).unwrap();
    url.set_path(pool.connect_options().get_database().unwrap());
    let leader = cloud_host::worker::acquire_runner(url.as_str())
        .await
        .unwrap();
    assert!(cloud_host::worker::acquire_runner(url.as_str())
        .await
        .is_err());
    leader.close().await.unwrap();
    cloud_host::worker::acquire_runner(url.as_str())
        .await
        .unwrap()
        .close()
        .await
        .unwrap();
}
