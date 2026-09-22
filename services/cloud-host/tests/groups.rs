use cloud_host::{db::resources, groups, work};
use sqlx::PgPool;
use std::sync::Arc;
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
    let records = work::enqueue(
        &pool,
        "alice",
        &Uuid::new_v4().to_string(),
        &bot.id,
        None,
        "Hi",
    )
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

    let (author_kind, author_bot_id): (String, Option<String>) =
        sqlx::query_as("SELECT author_kind, author_bot_id FROM messages WHERE id = $1")
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
    sqlx::query(
        "UPDATE messages SET body = 'Research findings', status = 'complete' WHERE id = $1",
    )
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

    let transcript = groups::list_messages(&pool, "alice", &group.id)
        .await
        .unwrap();
    assert_eq!(transcript.len(), 5);
    assert!(transcript.iter().any(|m| m.body.contains("landing page")));
    assert!(transcript
        .iter()
        .any(|m| m.author_bot_name.as_deref() == Some("Researcher")));
    assert!(transcript
        .iter()
        .any(|m| m.author_bot_name.as_deref() == Some("Designer")));
}

#[sqlx::test(migrations = "./migrations")]
async fn group_transcript_excludes_structured_runner_messages(pool: PgPool) {
    let bot_a = bot_with_computer(&pool, "alice", "PM").await;
    let bot_b = bot_with_computer(&pool, "alice", "Designer").await;
    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Status noise".into(),
            bot_ids: vec![bot_a.id.clone(), bot_b.id.clone()],
        },
    )
    .await
    .unwrap();

    let human = groups::append_human_message(&pool, "alice", &group.id, "Hello team")
        .await
        .unwrap();

    for (sequence, body) in [
        (3_i64, r#"{"status":"running","detail":null}"#),
        (4_i64, r#"{"status":"completed","detail":null}"#),
    ] {
        sqlx::query(
            r#"
            INSERT INTO messages (
                id, conversation_id, role, kind, body, status, sequence, author_kind
            ) VALUES ($1, $2, 'assistant', 'agent_status', $3, 'complete', $4, 'system')
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&group.id)
        .bind(body)
        .bind(sequence)
        .execute(&pool)
        .await
        .unwrap();
    }

    let transcript = groups::list_messages(&pool, "alice", &group.id)
        .await
        .unwrap();
    assert_eq!(transcript.len(), 1);
    assert_eq!(transcript[0].id, human.id);
    assert!(!transcript[0].body.contains("\"status\""));

    let context =
        cloud_host::group_context::load_group_context_lines(&pool, &group.id, 10, &bot_a.id)
            .await
            .unwrap();
    assert_eq!(context.len(), 1);
    assert_eq!(context[0].body, "Hello team");
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
    cloud_host::conversation::set_codex_compacted_through_turns(
        &pool,
        &group.id,
        &researcher.id,
        24,
    )
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

    let compacted =
        cloud_host::conversation::get_codex_compacted_through_turns(&pool, &group.id, &designer.id)
            .await
            .unwrap();
    assert_eq!(compacted, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn membership_add_race_never_exceeds_max(pool: PgPool) {
    let mut bots = Vec::new();
    for i in 0..5 {
        bots.push(bot_with_computer(&pool, "alice", &format!("Bot{i}")).await);
    }
    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Race add".into(),
            bot_ids: bots.iter().take(2).map(|b| b.id.clone()).collect(),
        },
    )
    .await
    .unwrap();
    for bot in bots.iter().skip(2).take(3) {
        groups::add_participant(&pool, "alice", &group.id, &bot.id)
            .await
            .unwrap();
    }
    let extra = bot_with_computer(&pool, "alice", "Extra").await;
    let pool_a = pool.clone();
    let pool_b = pool.clone();
    let gid_a = group.id.clone();
    let gid_b = group.id.clone();
    let extra_a = extra.id.clone();
    let extra_b = extra.id.clone();
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let b1 = barrier.clone();
    let b2 = barrier.clone();
    let t1 = tokio::spawn(async move {
        b1.wait().await;
        groups::add_participant(&pool_a, "alice", &gid_a, &extra_a).await
    });
    let t2 = tokio::spawn(async move {
        b2.wait().await;
        groups::add_participant(&pool_b, "alice", &gid_b, &extra_b).await
    });
    let _ = t1.await.unwrap();
    let _ = t2.await.unwrap();
    let active: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM conversation_participants WHERE conversation_id = $1 AND left_at IS NULL",
    )
    .bind(&group.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(active, 6);
}

#[sqlx::test(migrations = "./migrations")]
async fn membership_remove_race_never_below_min(pool: PgPool) {
    let a = bot_with_computer(&pool, "alice", "A").await;
    let b = bot_with_computer(&pool, "alice", "B").await;
    let c = bot_with_computer(&pool, "alice", "C").await;
    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Race rm".into(),
            bot_ids: vec![a.id.clone(), b.id.clone(), c.id.clone()],
        },
    )
    .await
    .unwrap();
    let pool_a = pool.clone();
    let pool_b = pool.clone();
    let gid_a = group.id.clone();
    let gid_b = group.id.clone();
    let bid_a = b.id.clone();
    let bid_b = b.id.clone();
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let b1 = barrier.clone();
    let b2 = barrier.clone();
    let t1 = tokio::spawn(async move {
        b1.wait().await;
        groups::remove_participant(&pool_a, "alice", &gid_a, &bid_a).await
    });
    let t2 = tokio::spawn(async move {
        b2.wait().await;
        groups::remove_participant(&pool_b, "alice", &gid_b, &bid_b).await
    });
    let _ = t1.await.unwrap();
    let _ = t2.await.unwrap();
    let active: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM conversation_participants WHERE conversation_id = $1 AND left_at IS NULL",
    )
    .bind(&group.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(active >= 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn group_rename_and_delete(pool: PgPool) {
    let a = bot_with_computer(&pool, "alice", "A").await;
    let b = bot_with_computer(&pool, "alice", "B").await;
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

    let renamed = groups::rename_group(&pool, "alice", &group.id, "  Product launch  ")
        .await
        .unwrap();
    assert_eq!(renamed.name, "Product launch");

    let empty = groups::rename_group(&pool, "alice", &group.id, "   ")
        .await
        .unwrap_err();
    assert!(matches!(empty, cloud_host::error::ApiError::Validation(_)));

    let foreign = groups::rename_group(&pool, "bob", &group.id, "Nope")
        .await
        .unwrap_err();
    assert!(matches!(foreign, cloud_host::error::ApiError::NotFound));

    groups::append_human_message(&pool, "alice", &group.id, "Kickoff")
        .await
        .unwrap();

    groups::delete_group(&pool, "alice", &group.id)
        .await
        .unwrap();

    let missing = groups::get_conversation_for_owner(&pool, "alice", &group.id)
        .await
        .unwrap_err();
    assert!(matches!(missing, cloud_host::error::ApiError::NotFound));

    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM conversation_participants WHERE conversation_id = $1",
    )
    .bind(&group.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 0);

    let messages: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE conversation_id = $1")
            .bind(&group.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(messages, 0);

    let already_gone = groups::delete_group(&pool, "alice", &group.id)
        .await
        .unwrap_err();
    assert!(matches!(
        already_gone,
        cloud_host::error::ApiError::NotFound
    ));
}

#[sqlx::test(migrations = "./migrations")]
async fn deleting_a_bot_removes_group_membership_and_keeps_group_history(pool: PgPool) {
    let designer = bot_with_computer(&pool, "alice", "Designer").await;
    let researcher = bot_with_computer(&pool, "alice", "Researcher").await;
    let reviewer = bot_with_computer(&pool, "alice", "Reviewer").await;
    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Team 1".into(),
            bot_ids: vec![
                designer.id.clone(),
                researcher.id.clone(),
                reviewer.id.clone(),
            ],
        },
    )
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO messages (
            id, conversation_id, role, kind, body, status, sequence, author_kind, author_bot_id
        )
        VALUES ($1, $2, 'assistant', 'chat', 'Hello from Designer', 'complete', 1, 'bot', $3)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&group.id)
    .bind(&designer.id)
    .execute(&pool)
    .await
    .unwrap();

    let deleted = resources::delete_bot(&pool, "alice", &designer.id)
        .await
        .expect("bot in a three-member group should be deletable");
    assert!(deleted);

    let designer_left: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM bots WHERE id = $1)")
        .bind(&designer.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!designer_left);

    let group_left: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM conversations WHERE id = $1)")
            .bind(&group.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(group_left);

    let designer_member: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM conversation_participants WHERE conversation_id = $1 AND bot_id = $2)",
    )
    .bind(&group.id)
    .bind(&designer.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!designer_member);

    let researcher_member: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM conversation_participants WHERE conversation_id = $1 AND bot_id = $2 AND left_at IS NULL)",
    )
    .bind(&group.id)
    .bind(&researcher.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(researcher_member);

    let remaining_members: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM conversation_participants WHERE conversation_id = $1 AND left_at IS NULL",
    )
    .bind(&group.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining_members, 2);

    let (message_count, unattributed): (i64, i64) = sqlx::query_as(
        r#"
        SELECT COUNT(*), COUNT(*) FILTER (WHERE author_bot_id IS NULL)
        FROM messages
        WHERE conversation_id = $1
        "#,
    )
    .bind(&group.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(message_count, 1);
    assert_eq!(unattributed, 1);
}
