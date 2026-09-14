use cloud_host::{db::resources, groups, work};
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
async fn direct_conversation_authorship_migration(pool: PgPool) {
    let bot = bot_with_computer(&pool, "alice", "Designer").await;
    let records = work::enqueue(&pool, "alice", &Uuid::new_v4().to_string(), &bot.id, None, "Hi")
        .await
        .unwrap();
    sqlx::query("UPDATE messages SET body = 'Hello back', status = 'complete' WHERE id = $1")
        .bind(&records.assistant_message_id)
        .execute(&pool)
        .await
        .unwrap();

    let user_kind: String = sqlx::query_scalar(
        "SELECT author_kind FROM messages WHERE conversation_id = $1 AND role = 'user'",
    )
    .bind(&records.conversation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(user_kind, "human");

    let (author_kind, author_bot_id): (String, Option<String>) = sqlx::query_as(
        "SELECT author_kind, author_bot_id FROM messages WHERE id = $1",
    )
    .bind(&records.assistant_message_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(author_kind, "bot");
    assert_eq!(author_bot_id.as_deref(), Some(bot.id.as_str()));
}

#[sqlx::test(migrations = "./migrations")]
async fn group_create_and_membership_rules(pool: PgPool) {
    let a = bot_with_computer(&pool, "alice", "A").await;
    let b = bot_with_computer(&pool, "alice", "B").await;
    let c = bot_with_computer(&pool, "bob", "Secret").await;

    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Launch".into(),
            bot_ids: vec![a.id.clone(), b.id.clone()],
        },
    )
    .await
    .unwrap();
    assert_eq!(group.participants.len(), 2);

    let err = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "One".into(),
            bot_ids: vec![a.id.clone()],
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, cloud_host::error::ApiError::Validation(_)));

    let err = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Many".into(),
            bot_ids: vec![
                a.id.clone(),
                b.id.clone(),
                a.id.clone(),
                b.id.clone(),
                a.id.clone(),
                b.id.clone(),
                a.id.clone(),
            ],
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, cloud_host::error::ApiError::Validation(_)));

    let err = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Cross".into(),
            bot_ids: vec![a.id.clone(), c.id.clone()],
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, cloud_host::error::ApiError::NotFound));
}

#[sqlx::test(migrations = "./migrations")]
async fn group_human_and_bot_authored_messages(pool: PgPool) {
    let researcher = bot_with_computer(&pool, "alice", "Researcher").await;
    let designer = bot_with_computer(&pool, "alice", "Designer").await;
    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Product launch".into(),
            bot_ids: vec![researcher.id.clone(), designer.id.clone()],
        },
    )
    .await
    .unwrap();

    let human = groups::append_human_message(
        &pool,
        "alice",
        &group.id,
        "We need to improve the landing page for launch.",
    )
    .await
    .unwrap();
    assert_eq!(human.author_kind, "human");

    let research_run = groups::enqueue_group_bot_run(
        &pool,
        "alice",
        &group.id,
        &researcher.id,
        &Uuid::new_v4().to_string(),
        "Research competitors",
    )
    .await
    .unwrap();
    sqlx::query("UPDATE messages SET body = 'Research findings', status = 'complete' WHERE id = $1")
        .bind(&research_run.assistant_message_id)
        .execute(&pool)
        .await
        .unwrap();

    let design_run = groups::enqueue_group_bot_run(
        &pool,
        "alice",
        &group.id,
        &designer.id,
        &Uuid::new_v4().to_string(),
        "Propose a design",
    )
    .await
    .unwrap();
    sqlx::query("UPDATE messages SET body = 'Design proposal', status = 'complete' WHERE id = $1")
        .bind(&design_run.assistant_message_id)
        .execute(&pool)
        .await
        .unwrap();

    let transcript = groups::list_messages(&pool, "alice", &group.id).await.unwrap();
    assert_eq!(transcript.len(), 5);
    assert!(transcript.iter().any(|m| m.body.contains("landing page")));
    assert!(
        transcript
            .iter()
            .any(|m| m.author_bot_name.as_deref() == Some("Researcher"))
    );
    assert!(
        transcript
            .iter()
            .any(|m| m.author_bot_name.as_deref() == Some("Designer"))
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn per_bot_codex_threads_are_independent(pool: PgPool) {
    let researcher = bot_with_computer(&pool, "alice", "Researcher").await;
    let designer = bot_with_computer(&pool, "alice", "Designer").await;
    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Threads".into(),
            bot_ids: vec![researcher.id.clone(), designer.id.clone()],
        },
    )
    .await
    .unwrap();

    cloud_host::conversation::set_codex_thread_id(&pool, &group.id, &researcher.id, "thread-r")
        .await
        .unwrap();
    cloud_host::conversation::set_codex_thread_id(&pool, &group.id, &designer.id, "thread-d")
        .await
        .unwrap();
    cloud_host::conversation::set_codex_compacted_through_turns(&pool, &group.id, &researcher.id, 24)
        .await
        .unwrap();

    let r = cloud_host::conversation::get_codex_thread_id(&pool, &group.id, &researcher.id)
        .await
        .unwrap();
    let d = cloud_host::conversation::get_codex_thread_id(&pool, &group.id, &designer.id)
        .await
        .unwrap();
    assert_eq!(r.as_deref(), Some("thread-r"));
    assert_eq!(d.as_deref(), Some("thread-d"));

    let compacted = cloud_host::conversation::get_codex_compacted_through_turns(
        &pool,
        &group.id,
        &designer.id,
    )
    .await
    .unwrap();
    assert_eq!(compacted, 0);
}
