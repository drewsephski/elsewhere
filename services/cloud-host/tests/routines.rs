use chrono::{Duration, Utc};
use cloud_host::{
    db::resources,
    routines::{self, RoutineInput},
    work,
};
use sqlx::PgPool;

async fn input(pool: &PgPool, owner: &str) -> RoutineInput {
    let computer = resources::insert_computer_placeholder(pool, owner, "Computer")
        .await
        .unwrap();
    let bot = resources::insert_bot(
        pool,
        owner,
        "Scout",
        "Cite sources",
        "gpt-5.6-luna",
        Some(&computer.id),
        "codex",
        "sky-wisp",
    )
    .await
    .unwrap();
    RoutineInput {
        bot_id: bot.id,
        name: "Daily brief".into(),
        instructions: "Review project files".into(),
        interval_minutes: Some(60),
        next_run_at: Utc::now() - Duration::minutes(1),
        enabled: true,
        schedule_kind: None,
        schedule_expression: None,
        timezone: None,
        destination_conversation_id: None,
        failure_policy: None,
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_ticks_enqueue_one_occurrence_with_same_security_context(pool: PgPool) {
    let input = input(&pool, "alice").await;
    let routine = routines::save(&pool, "alice", None, &input).await.unwrap();
    let now = Utc::now();
    let (a, b) = tokio::join!(routines::tick(&pool, now), routines::tick(&pool, now));
    assert_eq!(a.unwrap() + b.unwrap(), 1);
    let saved = routines::list(&pool, "alice").await.unwrap().remove(0);
    assert!(saved.next_run_at > now);
    let work = work::claim_next(&pool).await.unwrap().unwrap();
    assert_eq!(Some(work.records.run_id.clone()), saved.last_run_id);
    assert_eq!(work.bot_id, input.bot_id);
    assert_eq!(work.user_message, input.instructions);
    let (owner,source):(String,String)=sqlx::query_as("SELECT r.owner_id,q.routine_id FROM agent_runs r JOIN work_queue q ON q.run_id=r.id WHERE r.id=$1")
        .bind(&work.records.run_id).fetch_one(&pool).await.unwrap();
    assert_eq!(owner, "alice");
    assert_eq!(source, routine.id);
}

#[sqlx::test(migrations = "./migrations")]
async fn missed_occurrences_coalesce_and_active_work_never_overlaps(pool: PgPool) {
    let mut input = input(&pool, "alice").await;
    input.next_run_at = Utc::now() - Duration::days(10);
    routines::save(&pool, "alice", None, &input).await.unwrap();
    let now = Utc::now();
    assert_eq!(routines::tick(&pool, now).await.unwrap(), 1);
    assert_eq!(
        routines::tick(&pool, now + Duration::days(1))
            .await
            .unwrap(),
        0
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn failures_pause_until_explicit_resume_then_can_schedule_again(pool: PgPool) {
    let input = input(&pool, "alice").await;
    let routine = routines::save(&pool, "alice", None, &input).await.unwrap();
    let now = Utc::now();
    routines::tick(&pool, now).await.unwrap();
    sqlx::query("UPDATE agent_runs SET status='failed'")
        .execute(&pool)
        .await
        .unwrap();
    routines::tick(&pool, now + Duration::hours(2))
        .await
        .unwrap();
    let paused = routines::list(&pool, "alice").await.unwrap().remove(0);
    assert!(!paused.enabled);
    assert!(paused.last_error.is_some());
    routines::set_enabled(&pool, "alice", &routine.id, true)
        .await
        .unwrap();
    assert_eq!(
        routines::tick(&pool, now + Duration::hours(3))
            .await
            .unwrap(),
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn pause_and_manual_run_are_scoped_and_idempotent(pool: PgPool) {
    let input = input(&pool, "alice").await;
    let routine = routines::save(&pool, "alice", None, &input).await.unwrap();
    assert!(routines::list(&pool, "bob").await.unwrap().is_empty());
    assert!(routines::save(&pool, "bob", None, &input).await.is_err());
    assert!(routines::save(&pool, "bob", Some(&routine.id), &input)
        .await
        .is_err());
    assert!(routines::set_enabled(&pool, "bob", &routine.id, false)
        .await
        .is_err());
    assert!(routines::run_now(&pool, "bob", &routine.id, "key")
        .await
        .is_err());
    routines::set_enabled(&pool, "alice", &routine.id, false)
        .await
        .unwrap();
    assert_eq!(routines::tick(&pool, Utc::now()).await.unwrap(), 0);
    let a = routines::run_now(&pool, "alice", &routine.id, "key")
        .await
        .unwrap();
    let b = routines::run_now(&pool, "alice", &routine.id, "key")
        .await
        .unwrap();
    assert_eq!(a, b);
    assert!(routines::run_now(&pool, "alice", &routine.id, "other")
        .await
        .is_err());
    assert!(!routines::list(&pool, "alice").await.unwrap()[0].enabled);
}

#[sqlx::test(migrations = "./migrations")]
async fn unavailable_computer_pauses_without_partial_assignment(pool: PgPool) {
    let input = input(&pool, "alice").await;
    routines::save(&pool, "alice", None, &input).await.unwrap();
    sqlx::query("UPDATE sandboxes SET state='archived'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(routines::tick(&pool, Utc::now()).await.unwrap(), 0);
    let paused = routines::list(&pool, "alice").await.unwrap().remove(0);
    assert!(!paused.enabled);
    assert!(paused.last_error.is_some());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn fixed_interval_cadence_and_input_bounds() {
    let now = Utc::now();
    let due = now - Duration::minutes(125);
    assert_eq!(
        routines::next_occurrence(due, 60, now),
        due + Duration::minutes(180)
    );
    let mut input = RoutineInput {
        bot_id: "bot".into(),
        name: "Routine".into(),
        instructions: "Task".into(),
        interval_minutes: Some(1),
        next_run_at: now,
        enabled: true,
        schedule_kind: None,
        schedule_expression: None,
        timezone: None,
        destination_conversation_id: None,
        failure_policy: None,
    };
    assert!(input.validate().is_err());
    input.interval_minutes = Some(1440);
    assert!(input.validate().is_ok());
    input.name = " ".into();
    assert!(input.validate().is_err());
}
