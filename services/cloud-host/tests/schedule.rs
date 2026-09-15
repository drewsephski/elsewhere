use chrono::{Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Timelike, Utc};
use chrono_tz::America::Chicago;
use cloud_host::schedule::{
    initial_next_run, is_valid_occurrence, next_after, parse_schedule, resume_next_run,
    ScheduleKind,
};

#[test]
fn weekdays_at_eight_chicago_advances_across_dst() {
    let schedule = parse_schedule("weekly", "weekdays|08:00", "America/Chicago", None).unwrap();
    let due = Chicago
        .with_ymd_and_hms(2026, 3, 6, 8, 0, 0)
        .unwrap()
        .with_timezone(&Utc);
    let after_dst = Chicago
        .with_ymd_and_hms(2026, 3, 10, 7, 0, 0)
        .unwrap()
        .with_timezone(&Utc);
    let next = next_after(&schedule, due, after_dst).unwrap();
    let local = next.with_timezone(&Chicago);
    assert_eq!(local.hour(), 8);
    assert_eq!(local.weekday(), chrono::Weekday::Tue);
}

#[test]
fn interval_coalescing_unchanged() {
    let schedule = parse_schedule("interval", "60", "UTC", Some(60)).unwrap();
    assert_eq!(schedule.kind, ScheduleKind::Interval);
    let due = Utc::now() - Duration::hours(3);
    let now = Utc::now();
    let next = next_after(&schedule, due, now).unwrap();
    assert!(next > now - Duration::minutes(1));
}

#[test]
fn daily_ignores_arbitrary_client_anchor_for_initial_next_run() {
    let schedule = parse_schedule("daily", "08:00", "America/Chicago", None).unwrap();
    let now = Chicago
        .with_ymd_and_hms(2026, 9, 14, 10, 0, 0)
        .unwrap()
        .with_timezone(&Utc);
    let requested = Chicago
        .with_ymd_and_hms(2026, 9, 14, 13, 17, 0)
        .unwrap()
        .with_timezone(&Utc);
    let next = initial_next_run(&schedule, requested, now).unwrap();
    let local = next.with_timezone(&Chicago);
    assert_eq!(local.hour(), 8);
    assert_eq!(local.minute(), 0);
    assert!(local.date_naive() >= now.with_timezone(&Chicago).date_naive());
}

#[test]
fn weekly_weekdays_initial_next_run_is_valid_slot() {
    let schedule = parse_schedule("weekly", "weekdays|08:00", "America/Chicago", None).unwrap();
    let now = Chicago
        .with_ymd_and_hms(2026, 9, 14, 9, 0, 0)
        .unwrap()
        .with_timezone(&Utc);
    let requested = now;
    let next = initial_next_run(&schedule, requested, now).unwrap();
    let local = next.with_timezone(&Chicago);
    assert_eq!(local.hour(), 8);
    assert!(local.weekday().number_from_monday() <= 5);
}

#[test]
fn cron_initial_next_run_matches_expression() {
    let schedule = parse_schedule("cron", "0 8 * * 1-5", "America/Chicago", None).unwrap();
    let now = Chicago
        .with_ymd_and_hms(2026, 9, 14, 9, 0, 0)
        .unwrap()
        .with_timezone(&Utc);
    let next = initial_next_run(&schedule, now, now).unwrap();
    let local = next.with_timezone(&Chicago);
    assert_eq!(local.hour(), 8);
    assert_eq!(local.minute(), 0);
    assert!(local.weekday().number_from_monday() <= 5);
}

#[test]
fn interval_future_anchor_preserved() {
    let schedule = parse_schedule("interval", "60", "UTC", Some(60)).unwrap();
    let now = Utc::now();
    let anchor = now + Duration::hours(2);
    let next = initial_next_run(&schedule, anchor, now).unwrap();
    assert_eq!(next, anchor);
}

#[test]
fn interval_stale_anchor_coalesces() {
    let schedule = parse_schedule("interval", "60", "UTC", Some(60)).unwrap();
    let now = Utc::now();
    let due = now - Duration::hours(5);
    let next = initial_next_run(&schedule, due, now).unwrap();
    assert!(next > now - Duration::minutes(1));
}

#[test]
fn resume_daily_after_morning_uses_today_eight() {
    let schedule = parse_schedule("daily", "08:00", "America/Chicago", None).unwrap();
    let resume_at = Chicago
        .with_ymd_and_hms(2026, 9, 15, 7, 30, 0)
        .unwrap()
        .with_timezone(&Utc);
    let stale = Chicago
        .with_ymd_and_hms(2026, 9, 14, 8, 0, 0)
        .unwrap()
        .with_timezone(&Utc);
    let next = resume_next_run(&schedule, stale, resume_at).unwrap();
    let local = next.with_timezone(&Chicago);
    assert_eq!(local.date_naive().day(), 15);
    assert_eq!(local.hour(), 8);
}

#[test]
fn resume_daily_after_nine_uses_tomorrow_eight() {
    let schedule = parse_schedule("daily", "08:00", "America/Chicago", None).unwrap();
    let resume_at = Chicago
        .with_ymd_and_hms(2026, 9, 15, 9, 0, 0)
        .unwrap()
        .with_timezone(&Utc);
    let stale = Chicago
        .with_ymd_and_hms(2026, 9, 15, 8, 0, 0)
        .unwrap()
        .with_timezone(&Utc);
    let next = resume_next_run(&schedule, stale, resume_at).unwrap();
    let local = next.with_timezone(&Chicago);
    assert_eq!(local.date_naive().day(), 16);
    assert_eq!(local.hour(), 8);
}

#[test]
fn resume_weekdays_friday_afternoon_is_monday_morning() {
    let schedule = parse_schedule("weekly", "weekdays|08:00", "America/Chicago", None).unwrap();
    let resume_at = Chicago
        .with_ymd_and_hms(2026, 9, 18, 14, 0, 0)
        .unwrap()
        .with_timezone(&Utc);
    let stale = Chicago
        .with_ymd_and_hms(2026, 9, 18, 8, 0, 0)
        .unwrap()
        .with_timezone(&Utc);
    let next = resume_next_run(&schedule, stale, resume_at).unwrap();
    let local = next.with_timezone(&Chicago);
    assert_eq!(local.weekday(), chrono::Weekday::Mon);
    assert_eq!(local.hour(), 8);
}

#[test]
fn spring_forward_skips_nonexistent_daily_slot() {
    let schedule = parse_schedule("daily", "02:30", "America/Chicago", None).unwrap();
    let now = Chicago
        .with_ymd_and_hms(2026, 3, 8, 0, 0, 0)
        .unwrap()
        .with_timezone(&Utc);
    let next = next_after(&schedule, now, now).unwrap();
    let local = next.with_timezone(&Chicago);
    assert!(
        local.date_naive() > now.with_timezone(&Chicago).date_naive()
            || local.hour() != 2
            || local.minute() != 30
    );
}

#[test]
fn fall_back_daily_ambiguous_picks_earlier_instant() {
    let schedule = parse_schedule("daily", "01:30", "America/Chicago", None).unwrap();
    let nov1 = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    let time = NaiveTime::from_hms_opt(1, 30, 0).unwrap();
    let anchor = Chicago
        .with_ymd_and_hms(2026, 11, 1, 0, 0, 0)
        .unwrap()
        .with_timezone(&Utc);
    let first = next_after(&schedule, anchor, anchor).unwrap();
    let local = first.with_timezone(&Chicago);
    assert_eq!(local.date_naive(), nov1);
    assert_eq!(local.hour(), 1);
    assert_eq!(local.minute(), 30);
    match Chicago.from_local_datetime(&nov1.and_time(time)) {
        chrono::offset::LocalResult::Ambiguous(earlier, _) => {
            assert_eq!(first, earlier.with_timezone(&Utc));
        }
        other => panic!("expected ambiguous local time, got {other:?}"),
    }
}

#[test]
fn weekly_across_dst_transition_stays_on_eight_am() {
    let schedule = parse_schedule("weekly", "weekdays|08:00", "America/Chicago", None).unwrap();
    let due = Chicago
        .with_ymd_and_hms(2026, 3, 6, 8, 0, 0)
        .unwrap()
        .with_timezone(&Utc);
    let now = Chicago
        .with_ymd_and_hms(2026, 11, 2, 12, 0, 0)
        .unwrap()
        .with_timezone(&Utc);
    let next = next_after(&schedule, due, now).unwrap();
    let local = next.with_timezone(&Chicago);
    assert_eq!(local.hour(), 8);
    assert!(local.weekday().number_from_monday() <= 5);
}

#[test]
fn valid_occurrence_rejects_wrong_daily_time() {
    let schedule = parse_schedule("daily", "08:00", "America/Chicago", None).unwrap();
    let at = Chicago
        .with_ymd_and_hms(2026, 9, 15, 13, 17, 0)
        .unwrap()
        .with_timezone(&Utc);
    assert!(!is_valid_occurrence(&schedule, at));
}
