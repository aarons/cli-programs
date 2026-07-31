//! Projecting job triggers onto concrete points in time.

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveTime, TimeZone};

use crate::model::{CalendarInterval, Trigger};

impl CalendarInterval {
    pub fn matches_date(&self, date: NaiveDate) -> bool {
        if let Some(m) = self.month
            && date.month() != m
        {
            return false;
        }
        if let Some(d) = self.day
            && date.day() != d
        {
            return false;
        }
        if let Some(w) = self.weekday
            && date.weekday().num_days_from_sunday() != w % 7
        {
            return false;
        }
        true
    }

    pub fn times_of_day(&self) -> Vec<NaiveTime> {
        let hours: Vec<u32> = match self.hour {
            Some(h) => vec![h],
            None => (0..24).collect(),
        };
        let minutes: Vec<u32> = match self.minute {
            Some(m) => vec![m],
            None => (0..60).collect(),
        };
        let mut out = Vec::with_capacity(hours.len() * minutes.len());
        for h in &hours {
            for m in &minutes {
                if let Some(t) = NaiveTime::from_hms_opt(*h, *m, 0) {
                    out.push(t);
                }
            }
        }
        out
    }
}

/// Resolve a naive local datetime, skipping nonexistent times (DST spring-forward)
/// and taking the earliest of ambiguous ones.
fn resolve_local(date: NaiveDate, time: NaiveTime) -> Option<DateTime<Local>> {
    match Local.from_local_datetime(&date.and_time(time)) {
        chrono::LocalResult::Single(dt) => Some(dt),
        chrono::LocalResult::Ambiguous(a, _) => Some(a),
        chrono::LocalResult::None => None,
    }
}

/// All run times for `triggers` in the half-open window [from, until).
/// Interval triggers are projected from the window start (their true phase
/// depends on when launchd loaded the job, which we can't know).
pub fn occurrences_in_window(
    triggers: &[Trigger],
    from: DateTime<Local>,
    until: DateTime<Local>,
    cap: usize,
) -> Vec<DateTime<Local>> {
    let mut out: Vec<DateTime<Local>> = Vec::new();
    let start_date = from.date_naive();
    let end_date = until.date_naive();

    for trigger in triggers {
        match trigger {
            Trigger::Cron(expr) => {
                let mut date = start_date;
                while date <= end_date {
                    if expr.matches_date(date) {
                        for time in expr.times_of_day() {
                            if let Some(dt) = resolve_local(date, time)
                                && dt >= from
                                && dt < until
                            {
                                out.push(dt);
                            }
                        }
                    }
                    date = match date.succ_opt() {
                        Some(d) => d,
                        None => break,
                    };
                }
            }
            Trigger::Calendar(intervals) => {
                let mut date = start_date;
                while date <= end_date {
                    for interval in intervals {
                        if interval.matches_date(date) {
                            for time in interval.times_of_day() {
                                if let Some(dt) = resolve_local(date, time)
                                    && dt >= from
                                    && dt < until
                                {
                                    out.push(dt);
                                }
                            }
                        }
                    }
                    date = match date.succ_opt() {
                        Some(d) => d,
                        None => break,
                    };
                }
            }
            Trigger::Interval { seconds } => {
                let step = (*seconds).max(60); // don't flood the timeline
                let mut t = from;
                while t < until && out.len() < cap.saturating_mul(4).max(1000) {
                    out.push(t);
                    t += Duration::seconds(step as i64);
                }
            }
            _ => {}
        }
    }

    out.sort();
    out.dedup();
    out.truncate(cap);
    out
}

/// The next `count` run times strictly after `from`, looking ahead up to
/// `horizon_days`.
pub fn next_runs(
    triggers: &[Trigger],
    from: DateTime<Local>,
    count: usize,
    horizon_days: i64,
) -> Vec<DateTime<Local>> {
    let until = from + Duration::days(horizon_days);
    let start = from + Duration::seconds(1);
    let mut runs = occurrences_in_window(triggers, start, until, count.max(1) * 8);
    runs.truncate(count);
    runs
}

/// "in 5m", "in 3h 20m", "in 2d" style countdown.
pub fn human_until(target: DateTime<Local>, now: DateTime<Local>) -> String {
    let secs = (target - now).num_seconds();
    if secs < 0 {
        return "now".to_string();
    }
    let mins = secs / 60;
    if mins < 1 {
        return "<1m".to_string();
    }
    if mins < 60 {
        return format!("in {mins}m");
    }
    let hours = mins / 60;
    if hours < 24 {
        let rem = mins % 60;
        if rem == 0 {
            return format!("in {hours}h");
        }
        return format!("in {hours}h {rem}m");
    }
    let days = hours / 24;
    format!("in {days}d {}h", hours % 24)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cron::CronExpr;

    fn local(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(y, mo, d, h, mi, 0)
            .earliest()
            .unwrap()
    }

    #[test]
    fn cron_occurrences_in_a_day() {
        let trig = vec![Trigger::Cron(CronExpr::parse("0 */6 * * *").unwrap())];
        let from = local(2026, 7, 30, 0, 0);
        let until = local(2026, 7, 31, 0, 0);
        let runs = occurrences_in_window(&trig, from, until, 100);
        assert_eq!(runs.len(), 4);
        assert_eq!(runs[0], local(2026, 7, 30, 0, 0));
        assert_eq!(runs[3], local(2026, 7, 30, 18, 0));
    }

    #[test]
    fn calendar_weekly_occurrence() {
        let trig = vec![Trigger::Calendar(vec![CalendarInterval {
            minute: Some(0),
            hour: Some(12),
            day: None,
            weekday: Some(0), // Sunday
            month: None,
        }])];
        let from = local(2026, 7, 27, 0, 0); // Monday
        let until = local(2026, 8, 3, 0, 0);
        let runs = occurrences_in_window(&trig, from, until, 100);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0], local(2026, 8, 2, 12, 0)); // Sunday Aug 2
    }

    #[test]
    fn interval_projection() {
        let trig = vec![Trigger::Interval { seconds: 3600 }];
        let from = local(2026, 7, 30, 10, 0);
        let until = local(2026, 7, 30, 14, 0);
        let runs = occurrences_in_window(&trig, from, until, 100);
        assert_eq!(runs.len(), 4);
    }

    #[test]
    fn next_runs_ordering_and_count() {
        let trig = vec![Trigger::Cron(CronExpr::parse("30 2 * * *").unwrap())];
        let from = local(2026, 7, 30, 3, 0); // after today's 02:30
        let runs = next_runs(&trig, from, 3, 30);
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[0], local(2026, 7, 31, 2, 30));
        assert_eq!(runs[1], local(2026, 8, 1, 2, 30));
    }

    #[test]
    fn monthly_job_found_within_horizon() {
        let trig = vec![Trigger::Cron(CronExpr::parse("0 0 1 1 *").unwrap())]; // yearly
        let from = local(2026, 7, 30, 0, 0);
        let runs = next_runs(&trig, from, 1, 366);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0], local(2027, 1, 1, 0, 0));
    }

    #[test]
    fn untimed_triggers_produce_nothing() {
        let trig = vec![Trigger::RunAtLoad, Trigger::KeepAlive];
        let from = local(2026, 7, 30, 0, 0);
        let runs = occurrences_in_window(&trig, from, from + Duration::days(7), 100);
        assert!(runs.is_empty());
    }

    #[test]
    fn human_until_formats() {
        let now = local(2026, 7, 30, 12, 0);
        assert_eq!(human_until(local(2026, 7, 30, 12, 5), now), "in 5m");
        assert_eq!(human_until(local(2026, 7, 30, 15, 30), now), "in 3h 30m");
        assert_eq!(human_until(local(2026, 8, 2, 14, 0), now), "in 3d 2h");
        assert_eq!(human_until(now, now), "<1m");
    }
}
