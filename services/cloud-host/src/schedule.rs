//! Pure schedule evaluation (UTC persistence, IANA timezone semantics).

use std::str::FromStr;

use chrono::{
    DateTime, Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Timelike, Utc, Weekday,
};
use chrono::offset::LocalResult;
use chrono_tz::Tz;
use saffron::Cron;

use crate::error::ApiError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleKind {
    Interval,
    Daily,
    Weekly,
    Cron,
}

impl ScheduleKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Interval => "interval",
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Cron => "cron",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ApiError> {
        match value {
            "interval" => Ok(Self::Interval),
            "daily" => Ok(Self::Daily),
            "weekly" => Ok(Self::Weekly),
            "cron" => Ok(Self::Cron),
            _ => Err(ApiError::Validation("Unknown schedule type".into())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScheduleDefinition {
    pub kind: ScheduleKind,
    pub expression: String,
    pub timezone: Tz,
}

fn parse_time_hhmm(expression: &str) -> Result<NaiveTime, ApiError> {
    let parts: Vec<&str> = expression.trim().split(':').collect();
    if parts.len() != 2 {
        return Err(ApiError::Validation("Time must use HH:MM format".into()));
    }
    let hour: u32 = parts[0]
        .parse()
        .map_err(|_| ApiError::Validation("Hour must be 0–23".into()))?;
    let minute: u32 = parts[1]
        .parse()
        .map_err(|_| ApiError::Validation("Minute must be 0–59".into()))?;
    if hour > 23 || minute > 59 {
        return Err(ApiError::Validation("Time must use HH:MM format".into()));
    }
    NaiveTime::from_hms_opt(hour, minute, 0).ok_or_else(|| {
        ApiError::Validation("Time must use HH:MM format".into())
    })
}

fn parse_weekdays(token: &str) -> Result<Vec<Weekday>, ApiError> {
    if token.eq_ignore_ascii_case("weekdays") {
        return Ok(vec![
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
        ]);
    }
    let mut days = Vec::new();
    for part in token.split(',') {
        let day = match part.trim().to_ascii_uppercase().as_str() {
            "MON" => Weekday::Mon,
            "TUE" => Weekday::Tue,
            "WED" => Weekday::Wed,
            "THU" => Weekday::Thu,
            "FRI" => Weekday::Fri,
            "SAT" => Weekday::Sat,
            "SUN" => Weekday::Sun,
            _ => {
                return Err(ApiError::Validation(
                    "Weekly days must be MON,TUE,... or weekdays".into(),
                ));
            }
        };
        if !days.contains(&day) {
            days.push(day);
        }
    }
    if days.is_empty() {
        return Err(ApiError::Validation("Pick at least one weekday".into()));
    }
    days.sort_by_key(|d| d.num_days_from_monday());
    Ok(days)
}

pub fn parse_timezone(value: &str) -> Result<Tz, ApiError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return Err(ApiError::Validation("Choose a valid timezone".into()));
    }
    trimmed
        .parse()
        .map_err(|_| ApiError::Validation("Choose a valid IANA timezone".into()))
}

pub fn parse_schedule(
    kind: &str,
    expression: &str,
    timezone: &str,
    interval_minutes: Option<i32>,
) -> Result<ScheduleDefinition, ApiError> {
    let kind = ScheduleKind::parse(kind)?;
    let tz = parse_timezone(timezone)?;
    let expression = match kind {
        ScheduleKind::Interval => {
            let minutes = if expression.trim().is_empty() {
                interval_minutes.ok_or_else(|| {
                    ApiError::Validation("Interval routines need repeat minutes".into())
                })?
            } else {
                expression
                    .trim()
                    .parse()
                    .map_err(|_| ApiError::Validation("Interval must be a number of minutes".into()))?
            };
            if !(15..=43_200).contains(&minutes) {
                return Err(ApiError::Validation(
                    "Repeat intervals must be between 15 minutes and 30 days".into(),
                ));
            }
            minutes.to_string()
        }
        ScheduleKind::Daily => {
            parse_time_hhmm(expression)?;
            expression.trim().to_string()
        }
        ScheduleKind::Weekly => {
            let (days, time) = expression.split_once('|').ok_or_else(|| {
                ApiError::Validation("Weekly schedule needs DAYS|HH:MM".into())
            })?;
            parse_weekdays(days)?;
            parse_time_hhmm(time)?;
            format!("{}|{}", days.trim(), time.trim())
        }
        ScheduleKind::Cron => {
            let trimmed = expression.trim();
            if trimmed.is_empty() || trimmed.len() > 120 {
                return Err(ApiError::Validation("Cron expression is invalid".into()));
            }
            Cron::from_str(trimmed).map_err(|_| {
                ApiError::Validation("Cron expression is invalid".into())
            })?;
            trimmed.to_string()
        }
    };
    Ok(ScheduleDefinition {
        kind,
        expression,
        timezone: tz,
    })
}

/// Map a local wall-clock time to UTC. Skips nonexistent DST times; on fall-back ambiguity
/// picks the earlier UTC instant (single execution).
fn local_datetime(
    tz: Tz,
    date: NaiveDate,
    time: NaiveTime,
) -> Option<DateTime<Utc>> {
    local_datetime_after(tz, date, time, DateTime::<Utc>::MIN_UTC)
}

/// Like `local_datetime`, but when ambiguous prefers the earliest UTC instant still after `not_before`.
fn local_datetime_after(
    tz: Tz,
    date: NaiveDate,
    time: NaiveTime,
    not_before: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let threshold = not_before - Duration::seconds(1);
    match tz.from_local_datetime(&date.and_time(time)) {
        LocalResult::Single(dt) => {
            let utc = dt.with_timezone(&Utc);
            if utc > threshold {
                Some(utc)
            } else {
                None
            }
        }
        LocalResult::Ambiguous(earlier, later) => {
            let e = earlier.with_timezone(&Utc);
            if e > threshold {
                return Some(e);
            }
            let l = later.with_timezone(&Utc);
            if l > threshold {
                Some(l)
            } else {
                None
            }
        }
        LocalResult::None => None,
    }
}

pub fn is_valid_occurrence(schedule: &ScheduleDefinition, at: DateTime<Utc>) -> bool {
    match schedule.kind {
        ScheduleKind::Interval => true,
        ScheduleKind::Daily => {
            let time = parse_time_hhmm(&schedule.expression).ok();
            let time = match time {
                Some(t) => t,
                None => return false,
            };
            let local = at.with_timezone(&schedule.timezone);
            local.time() == time
        }
        ScheduleKind::Weekly => {
            let (days, time_str) = schedule.expression.split_once('|').unwrap_or(("", ""));
            let weekdays = parse_weekdays(days).ok();
            let time = parse_time_hhmm(time_str).ok();
            match (weekdays, time) {
                (Some(days), Some(time)) => {
                    let local = at.with_timezone(&schedule.timezone);
                    days.contains(&local.weekday()) && local.time() == time
                }
                _ => false,
            }
        }
        ScheduleKind::Cron => {
            let schedule_cron = Cron::from_str(&schedule.expression).ok();
            match schedule_cron {
                Some(cron) => cron_matches_local(&cron, at.with_timezone(&schedule.timezone)),
                None => false,
            }
        }
    }
}

fn next_daily(
    tz: Tz,
    time: NaiveTime,
    due: DateTime<Utc>,
    now: DateTime<Utc>,
) -> DateTime<Utc> {
    let anchor = if due > now { due } else { now };
    let local = anchor.with_timezone(&tz);
    let mut date = local.date_naive();
    for _ in 0..(366 * 2) {
        if let Some(candidate) = local_datetime_after(tz, date, time, anchor) {
            return candidate;
        }
        date = date.succ_opt().unwrap_or(date);
    }
    anchor + Duration::days(1)
}

fn next_weekly(
    tz: Tz,
    days: &[Weekday],
    time: NaiveTime,
    due: DateTime<Utc>,
    now: DateTime<Utc>,
) -> DateTime<Utc> {
    let anchor = if due > now { due } else { now };
    let local = anchor.with_timezone(&tz);
    let mut date = local.date_naive();
    for _ in 0..(366 * 2) {
        if days.contains(&date.weekday()) {
            if let Some(candidate) = local_datetime_after(tz, date, time, anchor) {
                return candidate;
            }
        }
        date = date.succ_opt().unwrap_or(date);
    }
    anchor + Duration::days(7)
}

fn cron_matches_local(schedule: &Cron, local: DateTime<Tz>) -> bool {
    let pseudo = Utc
        .with_ymd_and_hms(
            local.year(),
            local.month(),
            local.day(),
            local.hour(),
            local.minute(),
            0,
        )
        .single()
        .unwrap_or_else(Utc::now);
    schedule.contains(pseudo)
}

fn next_cron(
    tz: Tz,
    expression: &str,
    due: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<DateTime<Utc>, ApiError> {
    let schedule = Cron::from_str(expression).map_err(|_| {
        ApiError::Validation("Cron expression is invalid".into())
    })?;
    let anchor = if due > now { due } else { now };
    let mut probe = anchor.with_timezone(&tz) - Duration::minutes(1);
    for _ in 0..(366 * 24 * 60) {
        probe += Duration::minutes(1);
        if cron_matches_local(&schedule, probe) {
            let utc = probe.with_timezone(&Utc);
            if utc > anchor - Duration::seconds(1) {
                return Ok(utc);
            }
        }
    }
    Err(ApiError::Validation(
        "Could not find next cron occurrence within one year".into(),
    ))
}

/// Preserve cadence while coalescing missed occurrences into a single assignment.
pub fn next_occurrence(due: DateTime<Utc>, minutes: i32, now: DateTime<Utc>) -> DateTime<Utc> {
    let interval = i64::from(minutes) * 60;
    let missed = (now - due).num_seconds().max(0) / interval + 1;
    due + Duration::seconds(missed * interval)
}

/// Next fire time strictly after coalescing missed ticks into a single occurrence.
pub fn next_after(
    schedule: &ScheduleDefinition,
    due: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<DateTime<Utc>, ApiError> {
    match schedule.kind {
        ScheduleKind::Interval => {
            let minutes: i32 = schedule
                .expression
                .parse()
                .map_err(|_| ApiError::Internal("invalid interval".into()))?;
            Ok(next_occurrence(due, minutes, now))
        }
        ScheduleKind::Daily => {
            let time = parse_time_hhmm(&schedule.expression)?;
            let calendar_due = due + Duration::seconds(1);
            Ok(next_daily(schedule.timezone, time, calendar_due, now))
        }
        ScheduleKind::Weekly => {
            let (days, time) = schedule.expression.split_once('|').unwrap();
            let weekdays = parse_weekdays(days)?;
            let time = parse_time_hhmm(time)?;
            let calendar_due = due + Duration::seconds(1);
            Ok(next_weekly(schedule.timezone, &weekdays, time, calendar_due, now))
        }
        ScheduleKind::Cron => {
            let calendar_due = due + Duration::seconds(1);
            next_cron(schedule.timezone, &schedule.expression, calendar_due, now)
        }
    }
}

pub fn initial_next_run(
    schedule: &ScheduleDefinition,
    requested: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<DateTime<Utc>, ApiError> {
    if requested > now + Duration::days(366) {
        return Err(ApiError::Validation(
            "Choose a first run within the next year".into(),
        ));
    }
    match schedule.kind {
        ScheduleKind::Interval => {
            if requested > now {
                Ok(requested)
            } else {
                next_after(schedule, requested, now)
            }
        }
        _ => next_after(schedule, requested, now),
    }
}

/// Next fire time when resuming or refreshing a saved routine.
pub fn resume_next_run(
    schedule: &ScheduleDefinition,
    current: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<DateTime<Utc>, ApiError> {
    if current > now
        && (schedule.kind == ScheduleKind::Interval || is_valid_occurrence(schedule, current))
    {
        return Ok(current);
    }
    next_after(schedule, current, now)
}

pub fn human_schedule_label(schedule: &ScheduleDefinition) -> String {
    match schedule.kind {
        ScheduleKind::Interval => {
            let minutes: i32 = schedule.expression.parse().unwrap_or(0);
            if minutes % 1440 == 0 && minutes / 1440 == 7 {
                "Every 7 days".into()
            } else if minutes % 1440 == 0 {
                format!("Every {} days", minutes / 1440)
            } else if minutes % 60 == 0 && minutes / 60 > 1 {
                format!("Every {} hours", minutes / 60)
            } else if minutes == 60 {
                "Every hour".into()
            } else if minutes == 15 {
                "Every 15 minutes".into()
            } else {
                format!("Every {} minutes", minutes)
            }
        }
        ScheduleKind::Daily => format!("Daily at {}", schedule.expression),
        ScheduleKind::Weekly => {
            let (days, time) = schedule.expression.split_once('|').unwrap_or(("?", "?"));
            if days.eq_ignore_ascii_case("weekdays") {
                format!("Weekdays at {}", time)
            } else {
                format!("{} at {}", days, time)
            }
        }
        ScheduleKind::Cron => format!("Cron {}", schedule.expression),
    }
}
