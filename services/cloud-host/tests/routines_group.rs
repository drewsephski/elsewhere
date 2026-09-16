use chrono::{Duration, Utc};
use cloud_host::{
    conversation,
    db::resources,
    group_context, groups,
    routines::{self, RoutineInput},
    work,
};
use sqlx::PgPool;
async fn bot(pool: &PgPool, owner: &str, name: &str) -> resources::BotRow {
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
async fn group_destination_routine_admits_researcher_only(pool: PgPool) {
    let researcher = bot(&pool, "alice", "Researcher").await;
    let designer = bot(&pool, "alice", "Designer").await;
    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Launch Team".into(),
            bot_ids: vec![researcher.id.clone(), designer.id.clone()],
        },
    )
    .await
    .unwrap();

    let input = RoutineInput {
        bot_id: researcher.id.clone(),
        name: "Launch brief".into(),
        instructions: "Produce the scheduled launch brief.".into(),
        interval_minutes: Some(60),
        next_run_at: Utc::now() - Duration::minutes(1),
        enabled: true,
        schedule_kind: None,
        schedule_expression: None,
        timezone: None,
        destination_conversation_id: Some(group.id.clone()),
        failure_policy: None,
        skill_id: None,
        pinned_skill_version: None,
        trigger_mode: None,
    };
    let routine = routines::save(&pool, "alice", None, &input).await.unwrap();
    sqlx::query("UPDATE routines SET next_run_at = NOW() - interval '1 second' WHERE id = $1")
        .bind(&routine.id)
        .execute(&pool)
        .await
        .unwrap();
    let now = Utc::now();
    assert_eq!(routines::tick(&pool, now).await.unwrap(), 1);

    let runs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM routine_runs WHERE routine_id = $1")
        .bind(&routine.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(runs, 1);

    let agents: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(agents, 1);

    let work = work::claim_next(&pool).await.unwrap().unwrap();
    assert_eq!(work.bot_id, researcher.id);
    assert_eq!(work.records.conversation_id, group.id);
    assert!(work
        .user_message
        .contains("Produce the scheduled launch brief."));

    let (provenance, routine_id): (String, Option<String>) =
        sqlx::query_as("SELECT provenance_kind, routine_id FROM work_queue WHERE run_id = $1")
            .bind(&work.records.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(provenance, "routine");
    assert_eq!(routine_id.as_deref(), Some(routine.id.as_str()));

    let humans: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE conversation_id = $1 AND author_kind = 'human'",
    )
    .bind(&group.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(humans, 0);

    let systems: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE conversation_id = $1 AND author_kind = 'system'",
    )
    .bind(&group.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(systems, 1);

    let (author_kind, author_bot_id): (String, Option<String>) =
        sqlx::query_as("SELECT author_kind, author_bot_id FROM messages WHERE id = $1")
            .bind(&work.records.assistant_message_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(author_kind, "bot");
    assert_eq!(author_bot_id.as_deref(), Some(researcher.id.as_str()));

    let recipients: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM group_message_recipients")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(recipients, 0);

    let through: Option<i64> =
        sqlx::query_scalar("SELECT group_context_through_sequence FROM agent_runs WHERE id = $1")
            .bind(&work.records.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(through.is_some());

    let lines = group_context::load_group_context_lines(
        &pool,
        &group.id,
        through.unwrap_or(0) + 2,
        &researcher.id,
    )
    .await
    .unwrap();
    let block = group_context::format_group_context_block(&lines);
    assert!(!block.contains("You:\nRoutine"));

    let history = conversation::load_bounded_responses_history(
        &pool,
        &group.id,
        &researcher.id,
        &work.records.assistant_message_id,
    )
    .await
    .unwrap();
    assert!(history.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn group_routine_fails_when_owner_bot_removed_from_group(pool: PgPool) {
    let researcher = bot(&pool, "alice", "Researcher").await;
    let designer = bot(&pool, "alice", "Designer").await;
    let observer = bot(&pool, "alice", "Observer").await;
    let group = groups::create_group(
        &pool,
        "alice",
        groups::CreateGroupRequest {
            name: "Launch Team".into(),
            bot_ids: vec![
                researcher.id.clone(),
                designer.id.clone(),
                observer.id.clone(),
            ],
        },
    )
    .await
    .unwrap();

    let input = RoutineInput {
        bot_id: researcher.id.clone(),
        name: "Launch brief".into(),
        instructions: "Produce the scheduled launch brief.".into(),
        interval_minutes: Some(60),
        next_run_at: Utc::now() - Duration::minutes(1),
        enabled: true,
        schedule_kind: None,
        schedule_expression: None,
        timezone: None,
        destination_conversation_id: Some(group.id.clone()),
        failure_policy: None,
        skill_id: None,
        pinned_skill_version: None,
        trigger_mode: None,
    };
    let routine = routines::save(&pool, "alice", None, &input).await.unwrap();
    sqlx::query("UPDATE routines SET next_run_at = NOW() - interval '1 second' WHERE id = $1")
        .bind(&routine.id)
        .execute(&pool)
        .await
        .unwrap();

    groups::remove_participant(&pool, "alice", &group.id, &researcher.id)
        .await
        .unwrap();

    assert_eq!(routines::tick(&pool, Utc::now()).await.unwrap(), 0);
    let paused = routines::get(&pool, "alice", &routine.id).await.unwrap();
    assert!(!paused.enabled);
    let agents: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(agents, 0);
    let humans: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE conversation_id = $1 AND author_kind = 'human'",
    )
    .bind(&group.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(humans, 0);
}
