//! Cron expression parsing/matching and crontab file parsing.

use anyhow::{Context, Result, anyhow, bail};
use chrono::{Datelike, NaiveDate, NaiveTime};
use serde::Serialize;

/// Marker sched prepends to a crontab line to disable it without losing it.
pub const DISABLED_MARKER: &str = "# [sched:off] ";

/// A parsed 5-field cron expression. Sets are stored as bitmasks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CronExpr {
    pub minute: u64, // bits 0-59
    pub hour: u32,   // bits 0-23
    pub dom: u32,    // bits 1-31
    pub month: u16,  // bits 1-12
    pub dow: u8,     // bits 0-6, 0 = Sunday (7 normalized to 0)
    /// True when the field was `*` (affects cron's day-matching OR rule).
    pub dom_star: bool,
    pub dow_star: bool,
    pub raw: String,
}

const MINUTE_RANGE: (u32, u32) = (0, 59);
const HOUR_RANGE: (u32, u32) = (0, 23);
const DOM_RANGE: (u32, u32) = (1, 31);
const MONTH_RANGE: (u32, u32) = (1, 12);
const DOW_RANGE: (u32, u32) = (0, 7);

const MONTH_NAMES: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];
const DOW_NAMES: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

impl CronExpr {
    /// Parse a 5-field cron schedule like `*/15 2-4 * * mon-fri`.
    pub fn parse(expr: &str) -> Result<CronExpr> {
        let fields: Vec<&str> = expr.split_whitespace().collect();
        if fields.len() != 5 {
            bail!(
                "expected 5 fields (minute hour day month weekday), got {}",
                fields.len()
            );
        }
        let minute = parse_field(fields[0], MINUTE_RANGE, None).context("minute field")?;
        let hour = parse_field(fields[1], HOUR_RANGE, None).context("hour field")?;
        let dom = parse_field(fields[2], DOM_RANGE, None).context("day-of-month field")?;
        let month =
            parse_field(fields[3], MONTH_RANGE, Some(&MONTH_NAMES)).context("month field")?;
        let mut dow =
            parse_field(fields[4], DOW_RANGE, Some(&DOW_NAMES)).context("weekday field")?;
        // Normalize 7 (Sunday) onto bit 0.
        if dow & (1 << 7) != 0 {
            dow = (dow & !(1 << 7)) | 1;
        }
        Ok(CronExpr {
            minute,
            hour: hour as u32,
            dom: dom as u32,
            month: month as u16,
            dow: dow as u8,
            dom_star: fields[2] == "*",
            dow_star: fields[4] == "*" || fields[4] == "*/1",
            raw: fields.join(" "),
        })
    }

    /// Expand a cron @keyword. `@reboot` is not a timed schedule and returns None.
    pub fn from_keyword(word: &str) -> Option<Result<CronExpr>> {
        let expr = match word {
            "@yearly" | "@annually" => "0 0 1 1 *",
            "@monthly" => "0 0 1 * *",
            "@weekly" => "0 0 * * 0",
            "@daily" | "@midnight" => "0 0 * * *",
            "@hourly" => "0 * * * *",
            _ => return None,
        };
        Some(CronExpr::parse(expr))
    }

    pub fn matches_date(&self, date: NaiveDate) -> bool {
        if self.month & (1 << date.month()) == 0 {
            return false;
        }
        let dom_ok = self.dom & (1 << date.day()) != 0;
        let dow_ok = self.dow & (1 << (date.weekday().num_days_from_sunday())) != 0;
        // Classic cron rule: if both day fields are restricted, either may match.
        if !self.dom_star && !self.dow_star {
            dom_ok || dow_ok
        } else {
            dom_ok && dow_ok
        }
    }

    #[cfg(test)]
    pub fn matches_time(&self, time: NaiveTime) -> bool {
        use chrono::Timelike;
        self.minute & (1u64 << time.minute()) != 0 && self.hour & (1 << time.hour()) != 0
    }

    /// All times of day this expression fires at (sorted).
    pub fn times_of_day(&self) -> Vec<NaiveTime> {
        let mut out = Vec::new();
        for h in 0..24u32 {
            if self.hour & (1 << h) == 0 {
                continue;
            }
            for m in 0..60u32 {
                if self.minute & (1u64 << m) != 0 {
                    out.push(NaiveTime::from_hms_opt(h, m, 0).unwrap());
                }
            }
        }
        out
    }

    pub fn minute_values(&self) -> Vec<u32> {
        (0..60).filter(|m| self.minute & (1u64 << m) != 0).collect()
    }
    pub fn hour_values(&self) -> Vec<u32> {
        (0..24).filter(|h| self.hour & (1 << h) != 0).collect()
    }
    pub fn dom_values(&self) -> Vec<u32> {
        (1..=31).filter(|d| self.dom & (1 << d) != 0).collect()
    }
    pub fn month_values(&self) -> Vec<u32> {
        (1..=12).filter(|m| self.month & (1 << m) != 0).collect()
    }
    pub fn dow_values(&self) -> Vec<u32> {
        (0..7).filter(|d| self.dow & (1 << d) != 0).collect()
    }

    /// Best-effort human description, e.g. "daily at 02:30".
    pub fn describe(&self) -> String {
        let minutes = self.minute_values();
        let hours = self.hour_values();
        let doms = self.dom_values();
        let months = self.month_values();
        let dows = self.dow_values();

        let all_minutes = minutes.len() == 60;
        let all_hours = hours.len() == 24;
        let all_doms = doms.len() == 31;
        let all_months = months.len() == 12;
        let all_dows = dows.len() == 7;

        // Time-of-day portion.
        let time_part = if all_minutes && all_hours {
            "every minute".to_string()
        } else if all_hours && minutes.len() == 1 && minutes[0] == 0 {
            "hourly".to_string()
        } else if all_hours {
            if let Some(step) = detect_step(&minutes, 0, 59) {
                format!("every {step} min")
            } else if minutes.len() == 1 {
                format!("at :{:02} every hour", minutes[0])
            } else {
                format!("at minutes {} of every hour", join_nums(&minutes, 4))
            }
        } else if minutes.len() == 1 && hours.len() == 1 {
            format!("at {:02}:{:02}", hours[0], minutes[0])
        } else if minutes.len() == 1 {
            if let Some(step) = detect_step(&hours, 0, 23) {
                format!("every {step}h at :{:02}", minutes[0])
            } else {
                let times: Vec<String> = hours
                    .iter()
                    .take(4)
                    .map(|h| format!("{h:02}:{:02}", minutes[0]))
                    .collect();
                let more = if hours.len() > 4 { ", …" } else { "" };
                format!("at {}{more}", times.join(", "))
            }
        } else if all_minutes {
            format!("every min of hours {}", join_nums(&hours, 4))
        } else if let (Some(step), true) = (detect_step(&minutes, 0, 59), is_contiguous(&hours)) {
            format!(
                "every {step} min {:02}-{:02}h",
                hours[0],
                hours[hours.len() - 1]
            )
        } else {
            format!("min {} hr {}", join_nums(&minutes, 3), join_nums(&hours, 3))
        };

        // Date portion.
        let mut date_parts: Vec<String> = Vec::new();
        let dom_restricted = !self.dom_star && !all_doms;
        let dow_restricted = !self.dow_star && !all_dows;
        if dom_restricted {
            date_parts.push(format!("on day {}", join_nums(&doms, 4)));
        }
        if dow_restricted {
            let names: Vec<&str> = dows
                .iter()
                .map(|d| crate::model::weekday_name(*d))
                .collect();
            if names == ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday"] {
                date_parts.push("on weekdays".to_string());
            } else if names == ["Sunday", "Saturday"] {
                date_parts.push("on weekends".to_string());
            } else {
                date_parts.push(format!("on {}", names.join(", ")));
            }
        }
        if !all_months {
            let names: Vec<&str> = months
                .iter()
                .map(|m| crate::model::month_name(*m))
                .collect();
            date_parts.push(format!("in {}", names.join(", ")));
        }

        let prefix = if date_parts.is_empty()
            && !time_part.starts_with("every")
            && !time_part.starts_with("hourly")
        {
            "daily "
        } else {
            ""
        };
        let mut out = format!("{prefix}{time_part}");
        for p in &date_parts {
            out.push(' ');
            out.push_str(p);
        }
        out
    }
}

fn is_contiguous(values: &[u32]) -> bool {
    values.windows(2).all(|w| w[1] == w[0] + 1)
}

fn join_nums(nums: &[u32], max: usize) -> String {
    let shown: Vec<String> = nums.iter().take(max).map(|n| n.to_string()).collect();
    let mut s = shown.join(",");
    if nums.len() > max {
        s.push('…');
    }
    s
}

/// If `values` is exactly {min, min+step, ...} covering the range, return the step.
fn detect_step(values: &[u32], min: u32, max: u32) -> Option<u32> {
    if values.len() < 2 {
        return None;
    }
    let step = values[1] - values[0];
    if step <= 1 || values[0] != min {
        return None;
    }
    let expected: Vec<u32> = (min..=max).step_by(step as usize).collect();
    if expected == values { Some(step) } else { None }
}

/// Parse one cron field into a bitmask.
fn parse_field(field: &str, range: (u32, u32), names: Option<&[&str]>) -> Result<u64> {
    let (min, max) = range;
    let mut mask: u64 = 0;
    for part in field.split(',') {
        if part.is_empty() {
            bail!("empty list item in '{field}'");
        }
        let (body, step) = match part.split_once('/') {
            Some((b, s)) => {
                let step: u32 = s
                    .parse()
                    .map_err(|_| anyhow!("invalid step '{s}' in '{part}'"))?;
                if step == 0 {
                    bail!("step cannot be 0 in '{part}'");
                }
                (b, step)
            }
            None => (part, 1),
        };
        let (lo, hi) = if body == "*" {
            (min, max)
        } else if let Some((a, b)) = body.split_once('-') {
            (parse_value(a, range, names)?, parse_value(b, range, names)?)
        } else {
            let v = parse_value(body, range, names)?;
            if part.contains('/') {
                // e.g. "5/10" means starting at 5, every 10.
                (v, max)
            } else {
                (v, v)
            }
        };
        if lo > hi {
            // Wrap-around ranges like fri-mon or 22-2.
            let mut v = lo;
            loop {
                mask |= 1u64 << v;
                if v == max {
                    v = min;
                } else {
                    v += 1;
                }
                if v == hi + 1 || (hi == max && v == min) {
                    break;
                }
            }
            // Apply the tail bound.
            mask |= 1u64 << hi;
            if step != 1 {
                bail!("steps not supported with wrap-around ranges in '{part}'");
            }
            continue;
        }
        let mut v = lo;
        while v <= hi {
            mask |= 1u64 << v;
            v += step;
        }
    }
    if mask == 0 {
        bail!("field '{field}' matches nothing");
    }
    Ok(mask)
}

fn parse_value(s: &str, range: (u32, u32), names: Option<&[&str]>) -> Result<u32> {
    let (min, max) = range;
    if let Ok(v) = s.parse::<u32>() {
        if v < min || v > max {
            bail!("value {v} out of range {min}-{max}");
        }
        return Ok(v);
    }
    if let Some(names) = names {
        let lower = s.to_ascii_lowercase();
        let key = lower.get(..3).unwrap_or(&lower);
        if let Some(idx) = names.iter().position(|n| *n == key) {
            // Name tables are 0-based for weekdays, 1-based for months.
            let base = if names.len() == 12 { 1 } else { 0 };
            return Ok(idx as u32 + base);
        }
    }
    bail!("invalid value '{s}'")
}

/// One entry parsed out of a crontab file.
#[derive(Debug, Clone, PartialEq)]
pub struct CrontabEntry {
    /// 1-based line number in the file.
    pub line_no: usize,
    pub schedule: CronSchedule,
    /// User field (system crontabs only).
    pub user: Option<String>,
    pub command: String,
    /// Entry was disabled with sched's marker.
    pub disabled: bool,
    /// The full original line.
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CronSchedule {
    Expr(CronExpr),
    Reboot,
}

#[derive(Debug, Clone, Default)]
pub struct CrontabFile {
    pub entries: Vec<CrontabEntry>,
    /// Environment assignments found in the file (NAME, value).
    pub env: Vec<(String, String)>,
    /// Lines that look like job entries but failed to parse: (line_no, line, error).
    pub errors: Vec<(usize, String, String)>,
}

/// Parse crontab text. `system` selects the 6-token format with a user field
/// (as used by /etc/crontab and /etc/cron.d).
pub fn parse_crontab(text: &str, system: bool) -> CrontabFile {
    let mut out = CrontabFile::default();
    for (idx, line) in text.lines().enumerate() {
        let line_no = idx + 1;
        let raw = line.to_string();
        let mut disabled = false;
        let mut body = line.trim_start();

        if let Some(rest) = body.strip_prefix(DISABLED_MARKER.trim_end()) {
            // Tolerate both "# [sched:off] cmd" and marker with collapsed spaces.
            body = rest.trim_start();
            disabled = true;
        }
        if body.is_empty() || (body.starts_with('#') && !disabled) {
            continue;
        }
        // Environment assignment: NAME=value (no whitespace before '=').
        if !disabled && is_env_assignment(body) {
            if let Some((name, value)) = body.split_once('=') {
                out.env.push((
                    name.trim().to_string(),
                    value.trim().trim_matches('"').to_string(),
                ));
            }
            continue;
        }
        match parse_entry_line(body, system) {
            Ok(Some((schedule, user, command))) => out.entries.push(CrontabEntry {
                line_no,
                schedule,
                user,
                command,
                disabled,
                raw,
            }),
            Ok(None) => {}
            Err(e) => {
                // A disabled marker might wrap an arbitrary comment; only report
                // errors for lines that were plausibly job entries.
                if !disabled {
                    out.errors.push((line_no, raw, e.to_string()));
                }
            }
        }
    }
    out
}

fn is_env_assignment(line: &str) -> bool {
    match line.split_once('=') {
        Some((name, _)) => {
            let name = name.trim_end();
            !name.is_empty()
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && !name.chars().next().unwrap().is_ascii_digit()
        }
        None => false,
    }
}

fn parse_entry_line(
    body: &str,
    system: bool,
) -> Result<Option<(CronSchedule, Option<String>, String)>> {
    if let Some(rest) = body.strip_prefix('@') {
        let mut tokens = rest.split_whitespace();
        let word = format!("@{}", tokens.next().unwrap_or_default());
        let after = body[word.len()..].trim_start();
        let (user, command) = split_user(after, system);
        if command.is_empty() {
            bail!("missing command after {word}");
        }
        if word == "@reboot" {
            return Ok(Some((CronSchedule::Reboot, user, command)));
        }
        match CronExpr::from_keyword(&word) {
            Some(Ok(expr)) => return Ok(Some((CronSchedule::Expr(expr), user, command))),
            Some(Err(e)) => return Err(e),
            None => bail!("unknown keyword {word}"),
        }
    }
    let fields: Vec<&str> = body.split_whitespace().collect();
    let needed = if system { 7 } else { 6 };
    if fields.len() < needed {
        bail!("expected at least {needed} fields");
    }
    let expr = CronExpr::parse(&fields[0..5].join(" "))?;
    let after_schedule = skip_tokens(body, 5);
    let (user, command) = split_user(after_schedule, system);
    if command.is_empty() {
        bail!("missing command");
    }
    Ok(Some((CronSchedule::Expr(expr), user, command)))
}

/// Return the remainder of `s` after skipping `n` whitespace-separated tokens.
fn skip_tokens(s: &str, n: usize) -> &str {
    let mut rest = s.trim_start();
    for _ in 0..n {
        match rest.find(char::is_whitespace) {
            Some(pos) => rest = rest[pos..].trim_start(),
            None => return "",
        }
    }
    rest
}

fn split_user(s: &str, system: bool) -> (Option<String>, String) {
    if !system {
        return (None, s.trim().to_string());
    }
    let rest = s.trim_start();
    match rest.find(char::is_whitespace) {
        Some(pos) => (
            Some(rest[..pos].to_string()),
            rest[pos..].trim().to_string(),
        ),
        None => (None, rest.trim().to_string()),
    }
}

/// Derive a short display label from a shell command.
pub fn label_from_command(command: &str) -> String {
    let first = command.split_whitespace().next().unwrap_or(command);
    let base = first.rsplit('/').next().unwrap_or(first);
    // Shell wrappers aren't informative; peek at the next token.
    if matches!(base, "sh" | "bash" | "zsh" | "env" | "nice" | "sudo") {
        let mut tokens = command.split_whitespace().skip(1);
        for tok in tokens.by_ref() {
            if tok.starts_with('-') || tok.contains('=') {
                continue;
            }
            let b = tok.rsplit('/').next().unwrap_or(tok);
            if !b.is_empty() {
                return b.to_string();
            }
        }
    }
    base.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expr(s: &str) -> CronExpr {
        CronExpr::parse(s).unwrap()
    }

    #[test]
    fn parses_wildcards() {
        let e = expr("* * * * *");
        assert_eq!(e.minute_values().len(), 60);
        assert_eq!(e.hour_values().len(), 24);
        assert!(e.dom_star && e.dow_star);
    }

    #[test]
    fn parses_steps_ranges_lists() {
        let e = expr("*/15 2-4 1,15 * mon-fri");
        assert_eq!(e.minute_values(), vec![0, 15, 30, 45]);
        assert_eq!(e.hour_values(), vec![2, 3, 4]);
        assert_eq!(e.dom_values(), vec![1, 15]);
        assert_eq!(e.dow_values(), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn parses_names_and_sunday_seven() {
        let e = expr("0 0 * jan,jul sun");
        assert_eq!(e.month_values(), vec![1, 7]);
        assert_eq!(e.dow_values(), vec![0]);
        let e7 = expr("0 0 * * 7");
        assert_eq!(e7.dow_values(), vec![0]);
    }

    #[test]
    fn parses_wraparound_range() {
        let e = expr("0 22-2 * * *");
        assert_eq!(e.hour_values(), vec![0, 1, 2, 22, 23]);
        let e = expr("0 0 * * fri-mon");
        assert_eq!(e.dow_values(), vec![0, 1, 5, 6]);
    }

    #[test]
    fn rejects_bad_fields() {
        assert!(CronExpr::parse("60 * * * *").is_err());
        assert!(CronExpr::parse("* * * * * *").is_err());
        assert!(CronExpr::parse("*/0 * * * *").is_err());
        assert!(CronExpr::parse("a * * * *").is_err());
        assert!(CronExpr::parse("").is_err());
    }

    #[test]
    fn date_matching_or_rule() {
        // Both dom and dow restricted: OR semantics.
        let e = expr("0 0 13 * fri");
        // 2026-02-13 is a Friday and the 13th.
        assert!(e.matches_date(NaiveDate::from_ymd_opt(2026, 2, 13).unwrap()));
        // 2026-03-13 is a Friday.
        assert!(e.matches_date(NaiveDate::from_ymd_opt(2026, 3, 13).unwrap()));
        // 2026-02-06 is a Friday but not the 13th: still matches (OR).
        assert!(e.matches_date(NaiveDate::from_ymd_opt(2026, 2, 6).unwrap()));
        // 2026-02-14 is a Saturday, not the 13th.
        assert!(!e.matches_date(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap()));

        // Only dom restricted: AND semantics with wildcard dow.
        let e = expr("0 0 13 * *");
        assert!(!e.matches_date(NaiveDate::from_ymd_opt(2026, 2, 6).unwrap()));
        assert!(e.matches_date(NaiveDate::from_ymd_opt(2026, 2, 13).unwrap()));
    }

    #[test]
    fn time_matching() {
        let e = expr("30 14 * * *");
        assert!(e.matches_time(NaiveTime::from_hms_opt(14, 30, 0).unwrap()));
        assert!(!e.matches_time(NaiveTime::from_hms_opt(14, 31, 0).unwrap()));
        assert_eq!(e.times_of_day().len(), 1);
    }

    #[test]
    fn describes_common_patterns() {
        assert_eq!(expr("* * * * *").describe(), "every minute");
        assert_eq!(expr("*/5 * * * *").describe(), "every 5 min");
        assert_eq!(expr("0 * * * *").describe(), "hourly");
        assert_eq!(expr("30 2 * * *").describe(), "daily at 02:30");
        assert_eq!(expr("0 9 * * mon-fri").describe(), "at 09:00 on weekdays");
        assert_eq!(expr("0 0 1 * *").describe(), "at 00:00 on day 1");
        assert_eq!(expr("0 12 * * sat,sun").describe(), "at 12:00 on weekends");
        assert_eq!(expr("0 */6 * * *").describe(), "every 6h at :00");
        assert_eq!(
            expr("*/15 9-17 * * mon-fri").describe(),
            "every 15 min 09-17h on weekdays"
        );
    }

    #[test]
    fn keywords_expand() {
        assert!(CronExpr::from_keyword("@daily").unwrap().is_ok());
        assert!(CronExpr::from_keyword("@reboot").is_none());
        let weekly = CronExpr::from_keyword("@weekly").unwrap().unwrap();
        assert_eq!(weekly.dow_values(), vec![0]);
    }

    #[test]
    fn parses_user_crontab() {
        let text = "\
# backup things
SHELL=/bin/bash
MAILTO=me@example.com

30 2 * * * /usr/local/bin/backup.sh --full
@hourly /usr/local/bin/sync.sh
# [sched:off] 0 3 * * 0 /usr/local/bin/cleanup.sh
bad line here
";
        let tab = parse_crontab(text, false);
        assert_eq!(tab.entries.len(), 3);
        assert_eq!(tab.env.len(), 2);
        assert_eq!(tab.env[0], ("SHELL".to_string(), "/bin/bash".to_string()));
        assert_eq!(tab.errors.len(), 1);

        let e = &tab.entries[0];
        assert_eq!(e.line_no, 5);
        assert_eq!(e.command, "/usr/local/bin/backup.sh --full");
        assert!(!e.disabled);

        let disabled = &tab.entries[2];
        assert!(disabled.disabled);
        assert_eq!(disabled.command, "/usr/local/bin/cleanup.sh");
    }

    #[test]
    fn parses_system_crontab_with_user_field() {
        let text = "15 3 * * * root /usr/sbin/periodic daily\n@daily www /usr/local/bin/rotate\n";
        let tab = parse_crontab(text, true);
        assert_eq!(tab.entries.len(), 2);
        assert_eq!(tab.entries[0].user.as_deref(), Some("root"));
        assert_eq!(tab.entries[0].command, "/usr/sbin/periodic daily");
        assert_eq!(tab.entries[1].user.as_deref(), Some("www"));
    }

    #[test]
    fn reboot_entries() {
        let tab = parse_crontab("@reboot /usr/local/bin/start-agent\n", false);
        assert_eq!(tab.entries.len(), 1);
        assert_eq!(tab.entries[0].schedule, CronSchedule::Reboot);
    }

    #[test]
    fn command_labels() {
        assert_eq!(
            label_from_command("/usr/local/bin/backup.sh --full"),
            "backup.sh"
        );
        assert_eq!(label_from_command("sh -c 'echo hi'"), "'echo");
        assert_eq!(
            label_from_command("/usr/bin/env PATH=/x python3 /opt/x/job.py"),
            "python3"
        );
        assert_eq!(label_from_command("backup"), "backup");
    }
}
