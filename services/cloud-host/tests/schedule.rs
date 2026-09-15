use chrono::{Datelike, Duration, TimeZone, Timelike, Utc};
use chrono_tz::America::Chicago;
use cloud_host::schedule::{next_after, parse_schedule, ScheduleKind};

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
