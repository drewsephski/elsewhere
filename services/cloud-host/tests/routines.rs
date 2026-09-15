use chrono::{Duration, TimeZone, Timelike, Utc};
use chrono_tz::America::Chicago;
use cloud_host::{
    db::resources,
    routines::{self, RoutineInput},
    schedule::parse_schedule,
    work,
};
use sqlx::PgPool;

async fn save_due_now(pool: &PgPool, owner: &str, input: &RoutineInput) -> cloud_host::routines::RoutineView {
    let view = routines::save(pool, owner, None, input).await.unwrap();
    sqlx::query("UPDATE routines SET next_run_at = NOW() - interval '1 second' WHERE id = $1")
        .bind(&view.id)
        .execute(pool)
        .await
        .unwrap();
    routines::get(pool, owner, &view.id).await.unwrap()
}

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
    let routine = save_due_now(&pool, "alice", &input).await;
    let now = Utc::now();
    let (a, b) = tokio::join!(routines::tick(&pool, now), routines::tick(&pool, now));
    assert_eq!(a.unwrap() + b.unwrap(), 1);
    let saved = routines::list(&pool, "alice").await.unwrap().remove(0);
    assert!(saved.next_run_at > now);
    let work = work::claim_next(&pool).await.unwrap().unwrap();
    assert_eq!(Some(work.records.run_id.clone()), saved.last_run_id);
    assert_eq!(work.bot_id, input.bot_id);
    assert!(work.user_message.contains(input.instructions.as_str()));
    let humans: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE conversation_id = $1 AND author_kind = 'human'",
    )
    .bind(&work.records.conversation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(humans, 0);
    let (owner,source):(String,String)=sqlx::query_as("SELECT r.owner_id,q.routine_id FROM agent_runs r JOIN work_queue q ON q.run_id=r.id WHERE r.id=$1")
        .bind(&work.records.run_id).fetch_one(&pool).await.unwrap();
    assert_eq!(owner, "alice");
    assert_eq!(source, routine.id);
}

#[sqlx::test(migrations = "./migrations")]
async fn missed_occurrences_coalesce_and_active_work_never_overlaps(pool: PgPool) {
    let mut input = input(&pool, "alice").await;
    input.next_run_at = Utc::now() - Duration::days(10);
    save_due_now(&pool, "alice", &input).await;
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
    let routine = save_due_now(&pool, "alice", &input).await;
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
    save_due_now(&pool, "alice", &input).await;
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

#[sqlx::test(migrations = "./migrations")]
async fn save_derives_daily_next_run_from_schedule(pool: PgPool) {
    let mut input = input(&pool, "alice").await;
    input.schedule_kind = Some("daily".into());
    input.schedule_expression = Some("08:00".into());
    input.timezone = Some("America/Chicago".into());
    let now = Chicago
        .with_ymd_and_hms(2026, 9, 14, 10, 0, 0)
        .unwrap()
        .with_timezone(&Utc);
    input.next_run_at = Chicago
        .with_ymd_and_hms(2026, 9, 14, 13, 17, 0)
        .unwrap()
        .with_timezone(&Utc);
    let saved = routines::save(&pool, "alice", None, &input).await.unwrap();
    let local = saved.next_run_at.with_timezone(&Chicago);
    assert_eq!(local.hour(), 8);
    assert_eq!(local.minute(), 0);
    assert!(saved.next_run_at > now);
}

#[sqlx::test(migrations = "./migrations")]
async fn resume_recomputes_stale_daily_not_plus_one_minute(pool: PgPool) {
    let mut input = input(&pool, "alice").await;
    input.schedule_kind = Some("daily".into());
    input.schedule_expression = Some("08:00".into());
    input.timezone = Some("America/Chicago".into());
    input.next_run_at = Utc::now() - Duration::days(2);
    let routine = routines::save(&pool, "alice", None, &input).await.unwrap();
    routines::set_enabled(&pool, "alice", &routine.id, false)
        .await
        .unwrap();
    let resumed = routines::set_enabled(&pool, "alice", &routine.id, true)
        .await
        .unwrap();
    let local = resumed.next_run_at.with_timezone(&Chicago);
    assert_eq!(local.hour(), 8);
    assert_eq!(local.minute(), 0);
    let diff = resumed.next_run_at - Utc::now();
    assert!(diff > Duration::minutes(2));
}

#[sqlx::test(migrations = "./migrations")]
async fn coalesced_tick_records_skipped_occurrence(pool: PgPool) {
    let input = input(&pool, "alice").await;
    let routine = save_due_now(&pool, "alice", &input).await;
    let now = Utc::now();
    routines::tick(&pool, now).await.unwrap();
    let next_due = routines::list(&pool, "alice").await.unwrap()[0].next_run_at;
    sqlx::query("UPDATE agent_runs SET status='running'")
        .execute(&pool)
        .await
        .unwrap();
    routines::tick(&pool, next_due).await.unwrap();
    let skipped: (String, Option<String>) = sqlx::query_as(
        "SELECT status, error_code FROM routine_runs WHERE routine_id = $1 AND scheduled_for = $2",
    )
    .bind(&routine.id)
    .bind(next_due)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(skipped.0, "skipped");
    assert_eq!(skipped.1.as_deref(), Some("previous_run_active"));
}

#[sqlx::test(migrations = "./migrations")]
async fn failure_policy_continue_keeps_schedule_eligible(pool: PgPool) {
    let mut input = input(&pool, "alice").await;
    input.failure_policy = Some("continue".into());
    let routine = save_due_now(&pool, "alice", &input).await;
    let before = routine.next_run_at;
    let now = Utc::now();
    routines::tick(&pool, now).await.unwrap();
    sqlx::query("UPDATE agent_runs SET status='failed'")
        .execute(&pool)
        .await
        .unwrap();
    let after_fail = routines::list(&pool, "alice").await.unwrap().remove(0);
    assert!(after_fail.enabled);
    assert_eq!(
        routines::tick(&pool, after_fail.next_run_at).await.unwrap(),
        1
    );
    let after_tick = routines::list(&pool, "alice").await.unwrap().remove(0);
    assert!(after_tick.next_run_at > before);
}

#[sqlx::test(migrations = "./migrations")]
async fn legacy_interval_minutes_row_schedules_as_interval_utc(pool: PgPool) {
    let computer = resources::insert_computer_placeholder(&pool, "alice", "Computer")
        .await
        .unwrap();
    let bot = resources::insert_bot(
        &pool,
        "alice",
        "Legacy",
        "Role",
        "gpt-5.6-luna",
        Some(&computer.id),
        "codex",
        "sky-wisp",
    )
    .await
    .unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let next = Utc::now() - Duration::minutes(5);
    sqlx::query(
        r#"
        INSERT INTO routines (
            id, owner_id, bot_id, name, instructions, interval_minutes, next_run_at, enabled,
            schedule_kind, schedule_expression, timezone
        ) VALUES ($1,$2,$3,'Legacy','Task',60,$4,true,'interval','60','UTC')
        "#,
    )
    .bind(&id)
    .bind("alice")
    .bind(&bot.id)
    .bind(next)
    .execute(&pool)
    .await
    .unwrap();
    let row = routines::get(&pool, "alice", &id).await.unwrap();
    assert_eq!(row.schedule_kind, "interval");
    assert_eq!(row.schedule_expression, "60");
    assert_eq!(row.timezone, "UTC");
    let schedule = parse_schedule(
        &row.schedule_kind,
        &row.schedule_expression,
        &row.timezone,
        Some(row.interval_minutes),
    )
    .unwrap();
    assert_eq!(schedule.expression, "60");
    assert_eq!(routines::tick(&pool, Utc::now()).await.unwrap(), 1);
}

#[test]
fn runtime_message_formats_occurrence_in_routine_timezone() {
    let utc = Utc.with_ymd_and_hms(2026, 9, 15, 13, 0, 0).unwrap();
    let msg = routines::compose_routine_runtime_message(
        "Daily brief",
        "Summarize",
        utc,
        "America/Chicago",
        "Launch Team",
    );
    assert!(msg.contains("08:00"));
    assert!(!msg.contains("13:00 (America/Chicago)"));
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
