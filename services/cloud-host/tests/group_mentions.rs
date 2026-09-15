use cloud_host::{
    conversation::build_run_input_messages,
    db::resources,
    error::ApiError,
    groups::{self, SendGroupMessageRequest},
    work,
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
        "responses",
        "sky-wisp",
    )
    .await
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn group_send_routes_without_duplicate_human_messages(pool: PgPool) {
    let researcher = bot_with_computer(&pool, "alice", "Researcher").await;
    let designer = bot_with_computer(&pool, "alice", "Designer").await;
    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Launch".into(),
            bot_ids: vec![researcher.id.clone(), designer.id.clone()],
        },
    )
    .await
    .unwrap();

    let key = Uuid::new_v4().to_string();
    let send = groups::send_group_message(
        &pool,
        "alice",
        &group.id,
        &key,
        SendGroupMessageRequest {
            body: "@Researcher @Designer review this".into(),
            recipient_bot_ids: Some(vec![researcher.id.clone(), designer.id.clone()]),
            mention_mode: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(send.recipients.len(), 2);
    let humans: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE conversation_id = $1 AND author_kind = 'human'",
    )
    .bind(&group.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(humans, 1);

    let retry = groups::send_group_message(
        &pool,
        "alice",
        &group.id,
        &key,
        SendGroupMessageRequest {
            body: "@Researcher @Designer review this".into(),
            recipient_bot_ids: Some(vec![researcher.id.clone(), designer.id.clone()]),
            mention_mode: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(retry.message.id, send.message.id);
    let runs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs WHERE conversation_id = $1")
        .bind(&group.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(runs, 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn group_send_no_mention_persists_only_human(pool: PgPool) {
    let a = bot_with_computer(&pool, "alice", "A").await;
    let b = bot_with_computer(&pool, "alice", "B").await;
    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Note".into(),
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
            body: "This is just a note".into(),
            recipient_bot_ids: None,
            mention_mode: None,
        },
    )
    .await
    .unwrap();
    assert!(send.recipients.is_empty());
    let runs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs WHERE conversation_id = $1")
        .bind(&group.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(runs, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn designer_sees_researcher_in_group_context(pool: PgPool) {
    let researcher = bot_with_computer(&pool, "alice", "Researcher").await;
    let designer = bot_with_computer(&pool, "alice", "Designer").await;
    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Ctx".into(),
            bot_ids: vec![researcher.id.clone(), designer.id.clone()],
        },
    )
    .await
    .unwrap();

    groups::send_group_message(
        &pool,
        "alice",
        &group.id,
        &Uuid::new_v4().to_string(),
        SendGroupMessageRequest {
            body: "investigate".into(),
            recipient_bot_ids: Some(vec![researcher.id.clone()]),
            mention_mode: None,
        },
    )
    .await
    .unwrap();

    let research_run: String = sqlx::query_scalar(
        "SELECT run_id FROM group_message_recipients WHERE bot_id = $1",
    )
    .bind(&researcher.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let assistant_id: String = sqlx::query_scalar(
        "SELECT assistant_message_id FROM agent_runs WHERE id = $1",
    )
    .bind(&research_run)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE messages SET body = 'Finding ABC', status = 'complete' WHERE id = $1")
        .bind(&assistant_id)
        .execute(&pool)
        .await
        .unwrap();
    cloud_host::group_context::advance_last_seen_group_sequence(
        &pool,
        &group.id,
        &researcher.id,
        10,
    )
    .await
    .unwrap();

    let design_send = groups::send_group_message(
        &pool,
        "alice",
        &group.id,
        &Uuid::new_v4().to_string(),
        SendGroupMessageRequest {
            body: "use findings".into(),
            recipient_bot_ids: Some(vec![designer.id.clone()]),
            mention_mode: None,
        },
    )
    .await
    .unwrap();
    let design_run = design_send
        .recipients
        .iter()
        .find(|r| r.bot_id == designer.id)
        .and_then(|r| r.run_id.clone())
        .unwrap();
    let design_assistant: String = sqlx::query_scalar(
        "SELECT assistant_message_id FROM agent_runs WHERE id = $1",
    )
    .bind(&design_run)
    .fetch_one(&pool)
    .await
    .unwrap();

    let input = build_run_input_messages(
        &pool,
        &group.id,
        &designer.id,
        &design_assistant,
        "use findings",
        true,
    )
    .await
    .unwrap();
    let blob = input
        .iter()
        .filter_map(|v| v.get("content").and_then(|c| c.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(blob.contains("Researcher"));
    assert!(blob.contains("Finding ABC"));
    assert!(!blob.contains("\"role\":\"assistant\""));

    let user_turn_count = input
        .iter()
        .filter(|v| v.get("content") == Some(&serde_json::json!("use findings")))
        .count();
    assert_eq!(user_turn_count, 1, "current human turn must appear exactly once");
    assert!(
        !blob.contains("use findings\n\nuse findings"),
        "current message must not appear in group context block"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn codex_group_input_excludes_current_human_turn(pool: PgPool) {
    let researcher = bot_with_computer(&pool, "alice", "Researcher").await;
    let designer = bot_with_computer(&pool, "alice", "Designer").await;
    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Codex".into(),
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
            body: "@Designer use that finding".into(),
            recipient_bot_ids: Some(vec![designer.id.clone()]),
            mention_mode: None,
        },
    )
    .await
    .unwrap();
    let design_run = send.recipients[0].run_id.clone().unwrap();
    let design_assistant: String = sqlx::query_scalar(
        "SELECT assistant_message_id FROM agent_runs WHERE id = $1",
    )
    .bind(&design_run)
    .fetch_one(&pool)
    .await
    .unwrap();

    let input = build_run_input_messages(
        &pool,
        &group.id,
        &designer.id,
        &design_assistant,
        "@Designer use that finding",
        false,
    )
    .await
    .unwrap();
    assert_eq!(input.len(), 1);
    assert_eq!(
        input[0].get("content").and_then(|c| c.as_str()),
        Some("@Designer use that finding")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn group_idempotency_scoped_to_conversation(pool: PgPool) {
    let a = bot_with_computer(&pool, "alice", "A").await;
    let b = bot_with_computer(&pool, "alice", "B").await;
    let g1 = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "G1".into(),
            bot_ids: vec![a.id.clone(), b.id.clone()],
        },
    )
    .await
    .unwrap();
    let g2 = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "G2".into(),
            bot_ids: vec![a.id.clone(), b.id.clone()],
        },
    )
    .await
    .unwrap();
    let key = "shared-key";
    groups::send_group_message(
        &pool,
        "alice",
        &g1.id,
        key,
        SendGroupMessageRequest {
            body: "@A hello".into(),
            recipient_bot_ids: Some(vec![a.id.clone()]),
            mention_mode: None,
        },
    )
    .await
    .unwrap();
    groups::send_group_message(
        &pool,
        "alice",
        &g2.id,
        key,
        SendGroupMessageRequest {
            body: "@A hello".into(),
            recipient_bot_ids: Some(vec![a.id.clone()]),
            mention_mode: None,
        },
    )
    .await
    .unwrap();
    let sends: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM group_message_sends WHERE idempotency_key = $1")
        .bind(key)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sends, 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn group_idempotency_conflicts_on_payload_mismatch(pool: PgPool) {
    let a = bot_with_computer(&pool, "alice", "A").await;
    let b = bot_with_computer(&pool, "alice", "B").await;
    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Idem".into(),
            bot_ids: vec![a.id.clone(), b.id.clone()],
        },
    )
    .await
    .unwrap();
    let key = "same-key";
    groups::send_group_message(
        &pool,
        "alice",
        &group.id,
        key,
        SendGroupMessageRequest {
            body: "first".into(),
            recipient_bot_ids: Some(vec![a.id.clone()]),
            mention_mode: None,
        },
    )
    .await
    .unwrap();
    let err = groups::send_group_message(
        &pool,
        "alice",
        &group.id,
        key,
        SendGroupMessageRequest {
            body: "second".into(),
            recipient_bot_ids: Some(vec![a.id.clone()]),
            mention_mode: None,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ApiError::Conflict(_)));
}

#[sqlx::test(migrations = "./migrations")]
async fn direct_enqueue_still_inserts_human_message(pool: PgPool) {
    let bot = bot_with_computer(&pool, "alice", "Solo").await;
    let records = work::enqueue(
        &pool,
        "alice",
        &Uuid::new_v4().to_string(),
        &bot.id,
        None,
        "hello",
    )
    .await
    .unwrap();
    let humans: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE conversation_id = $1 AND author_kind = 'human'",
    )
    .bind(&records.conversation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(humans, 1);
}
