//! Mutating operations: crontab rewrites, launchctl calls, plist rescheduling.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::cron::{CronExpr, DISABLED_MARKER};
use crate::discovery::DiscoveryConfig;
use crate::launchd::current_uid;
use crate::model::{CalendarInterval, Job, SourceKind};

// ---------------------------------------------------------------------------
// crontab text surgery
// ---------------------------------------------------------------------------

/// Toggle sched's disabled marker on a 1-based line of crontab text.
pub fn toggle_cron_line(content: &str, line_no: usize, disable: bool) -> Result<String> {
    rewrite_line(content, line_no, |line| {
        let trimmed = line.trim_start();
        let already = trimmed.starts_with(DISABLED_MARKER.trim_end());
        if disable && !already {
            Ok(Some(format!("{DISABLED_MARKER}{line}")))
        } else if !disable && already {
            let rest = trimmed
                .strip_prefix(DISABLED_MARKER.trim_end())
                .unwrap_or(trimmed)
                .trim_start();
            Ok(Some(rest.to_string()))
        } else {
            Ok(Some(line.to_string()))
        }
    })
}

/// Remove a 1-based line from crontab text.
pub fn delete_cron_line(content: &str, line_no: usize) -> Result<String> {
    rewrite_line(content, line_no, |_| Ok(None))
}

/// Replace the schedule portion (5 fields or @keyword) of a crontab line,
/// preserving the command and any disabled marker.
pub fn reschedule_cron_line(content: &str, line_no: usize, new_expr: &str) -> Result<String> {
    CronExpr::parse(new_expr).context("invalid cron expression")?;
    rewrite_line(content, line_no, |line| {
        let trimmed = line.trim_start();
        let (marker, body) = if trimmed.starts_with(DISABLED_MARKER.trim_end()) {
            let rest = trimmed
                .strip_prefix(DISABLED_MARKER.trim_end())
                .unwrap_or(trimmed)
                .trim_start();
            (DISABLED_MARKER, rest)
        } else {
            ("", trimmed)
        };
        let command = if body.starts_with('@') {
            skip_fields(body, 1)
        } else {
            skip_fields(body, 5)
        };
        if command.is_empty() {
            bail!("line {line_no} has no command to preserve");
        }
        Ok(Some(format!("{marker}{new_expr} {command}")))
    })
}

fn skip_fields(s: &str, n: usize) -> &str {
    let mut rest = s.trim_start();
    for _ in 0..n {
        match rest.find(char::is_whitespace) {
            Some(pos) => rest = rest[pos..].trim_start(),
            None => return "",
        }
    }
    rest
}

fn rewrite_line(
    content: &str,
    line_no: usize,
    f: impl Fn(&str) -> Result<Option<String>>,
) -> Result<String> {
    let lines: Vec<&str> = content.lines().collect();
    if line_no == 0 || line_no > lines.len() {
        bail!(
            "line {line_no} out of range (crontab has {} lines)",
            lines.len()
        );
    }
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    for (idx, line) in lines.iter().enumerate() {
        if idx + 1 == line_no {
            if let Some(replacement) = f(line)? {
                out.push(replacement);
            }
        } else {
            out.push((*line).to_string());
        }
    }
    let mut text = out.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    Ok(text)
}

// ---------------------------------------------------------------------------
// crontab install / read
// ---------------------------------------------------------------------------

pub fn read_user_crontab_text(config: &DiscoveryConfig) -> Result<String> {
    if let Some(path) = &config.crontab_file {
        return Ok(std::fs::read_to_string(path)?);
    }
    let output = Command::new("crontab")
        .arg("-l")
        .output()
        .context("failed to run crontab")?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("no crontab") {
            Ok(String::new())
        } else {
            bail!("crontab -l failed: {}", stderr.trim())
        }
    }
}

pub fn install_user_crontab(config: &DiscoveryConfig, content: &str) -> Result<()> {
    if let Some(path) = &config.crontab_file {
        std::fs::write(path, content)?;
        return Ok(());
    }
    let tmp = std::env::temp_dir().join(format!("sched-crontab-{}.tmp", std::process::id()));
    std::fs::write(&tmp, content)?;
    let output = Command::new("crontab")
        .arg(&tmp)
        .output()
        .context("failed to run crontab")?;
    let _ = std::fs::remove_file(&tmp);
    if !output.status.success() {
        bail!(
            "crontab rejected the file: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// launchctl
// ---------------------------------------------------------------------------

fn run_cmd(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("failed to run {program}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        Ok(format!("{stdout}{stderr}").trim().to_string())
    } else {
        bail!(
            "{program} {} failed: {}",
            args.join(" "),
            if stderr.trim().is_empty() {
                stdout.trim().to_string()
            } else {
                stderr.trim().to_string()
            }
        )
    }
}

/// The launchctl domain a job lives in.
pub fn domain_for(kind: SourceKind) -> String {
    if kind.is_daemon() {
        "system".to_string()
    } else {
        format!("gui/{}", current_uid())
    }
}

pub fn launchctl_bootout(job: &Job) -> Result<String> {
    let domain = domain_for(job.kind);
    run_cmd(
        "launchctl",
        &["bootout", &format!("{domain}/{}", job.label)],
    )
}

pub fn launchctl_bootstrap(job: &Job) -> Result<String> {
    let domain = domain_for(job.kind);
    run_cmd(
        "launchctl",
        &[
            "bootstrap",
            &domain,
            job.source_path.to_str().context("non-utf8 path")?,
        ],
    )
}

pub fn launchctl_enable(job: &Job) -> Result<String> {
    let domain = domain_for(job.kind);
    run_cmd("launchctl", &["enable", &format!("{domain}/{}", job.label)])
}

pub fn launchctl_disable(job: &Job) -> Result<String> {
    let domain = domain_for(job.kind);
    run_cmd(
        "launchctl",
        &["disable", &format!("{domain}/{}", job.label)],
    )
}

pub fn launchctl_kickstart(job: &Job) -> Result<String> {
    let domain = domain_for(job.kind);
    run_cmd(
        "launchctl",
        &["kickstart", "-p", &format!("{domain}/{}", job.label)],
    )
}

/// Disable a launchd job: persistently disable, then unload if loaded.
pub fn disable_launchd_job(job: &Job) -> Result<String> {
    let mut messages = Vec::new();
    messages.push(launchctl_disable(job)?);
    match launchctl_bootout(job) {
        Ok(m) => messages.push(m),
        // Booting out an already-unloaded job fails; that's fine.
        Err(e) => messages.push(format!("(bootout: {e})")),
    }
    Ok(format!("disabled {}", job.label)
        + &messages
            .iter()
            .filter(|m| !m.is_empty())
            .map(|m| format!("\n{m}"))
            .collect::<String>())
}

/// Enable a launchd job: clear the disabled flag and load it.
pub fn enable_launchd_job(job: &Job) -> Result<String> {
    let mut messages = Vec::new();
    messages.push(launchctl_enable(job)?);
    match launchctl_bootstrap(job) {
        Ok(m) => messages.push(m),
        Err(e) => messages.push(format!("(bootstrap: {e})")),
    }
    Ok(format!("enabled {}", job.label)
        + &messages
            .iter()
            .filter(|m| !m.is_empty())
            .map(|m| format!("\n{m}"))
            .collect::<String>())
}

/// Reload a launchd job after its plist changed.
pub fn reload_launchd_job(job: &Job) -> Result<String> {
    let _ = launchctl_bootout(job); // may not be loaded
    launchctl_bootstrap(job)?;
    Ok(format!("reloaded {}", job.label))
}

// ---------------------------------------------------------------------------
// plist rescheduling
// ---------------------------------------------------------------------------

/// Convert a cron expression into launchd StartCalendarInterval entries.
///
/// cron ORs day-of-month and day-of-week when both are restricted, while
/// launchd ANDs all keys within one entry — so that case is emitted as two
/// groups of entries (one per day field).
pub fn cron_to_calendar(expr: &CronExpr) -> Result<Vec<CalendarInterval>> {
    let minutes = optional_values(expr.minute_values(), 60);
    let hours = optional_values(expr.hour_values(), 24);
    let months = optional_values(expr.month_values(), 12);
    let doms = if expr.dom_star {
        None
    } else {
        Some(expr.dom_values())
    };
    let dows = if expr.dow_star {
        None
    } else {
        Some(expr.dow_values())
    };

    let mut out: Vec<CalendarInterval> = Vec::new();
    let mut push_product = |days: Option<&Vec<u32>>, weekdays: Option<&Vec<u32>>| -> Result<()> {
        for mi in axis(&minutes) {
            for h in axis(&hours) {
                for mo in axis(&months) {
                    for d in axis_ref(days) {
                        for w in axis_ref(weekdays) {
                            out.push(CalendarInterval {
                                minute: mi,
                                hour: h,
                                day: d,
                                weekday: w,
                                month: mo,
                            });
                            if out.len() > 100 {
                                bail!(
                                    "schedule expands to more than 100 launchd calendar entries; \
                                         simplify the expression or edit the plist directly"
                                );
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    };

    match (&doms, &dows) {
        (Some(doms), Some(dows)) => {
            // OR semantics: one group restricted by day, one by weekday.
            push_product(Some(doms), None)?;
            push_product(None, Some(dows))?;
        }
        (Some(doms), None) => push_product(Some(doms), None)?,
        (None, Some(dows)) => push_product(None, Some(dows))?,
        (None, None) => push_product(None, None)?,
    }
    Ok(out)
}

fn optional_values(values: Vec<u32>, full_len: usize) -> Option<Vec<u32>> {
    if values.len() == full_len {
        None
    } else {
        Some(values)
    }
}

fn axis(vals: &Option<Vec<u32>>) -> Vec<Option<u32>> {
    match vals {
        Some(v) => v.iter().map(|x| Some(*x)).collect(),
        None => vec![None],
    }
}

fn axis_ref(vals: Option<&Vec<u32>>) -> Vec<Option<u32>> {
    match vals {
        Some(v) => v.iter().map(|x| Some(*x)).collect(),
        None => vec![None],
    }
}

/// Reconstruct a cron expression string from calendar intervals, when they are
/// shaped like a clean cartesian product. Used to prefill the reschedule input.
pub fn calendar_to_cron(intervals: &[CalendarInterval]) -> Option<String> {
    if intervals.is_empty() {
        return None;
    }
    let mut minutes = collect_field(intervals, |i| i.minute)?;
    let mut hours = collect_field(intervals, |i| i.hour)?;
    let mut days = collect_field(intervals, |i| i.day)?;
    let mut weekdays = collect_field(intervals, |i| i.weekday)?;
    let mut months = collect_field(intervals, |i| i.month)?;

    // Verify the product shape: every combination must be present exactly once.
    let expected = card(&minutes) * card(&hours) * card(&days) * card(&weekdays) * card(&months);
    if expected != intervals.len() {
        return None;
    }
    for list in [
        &mut minutes,
        &mut hours,
        &mut days,
        &mut weekdays,
        &mut months,
    ]
    .into_iter()
    .flatten()
    {
        list.sort_unstable();
        list.dedup();
    }
    Some(format!(
        "{} {} {} {} {}",
        field_str(&minutes),
        field_str(&hours),
        field_str(&days),
        field_str(&months),
        field_str(&weekdays),
    ))
}

/// Collect the set of values a field takes. None result = inconsistent
/// (some entries wildcard, some not).
fn collect_field(
    intervals: &[CalendarInterval],
    get: impl Fn(&CalendarInterval) -> Option<u32>,
) -> Option<Option<Vec<u32>>> {
    let firsts: Vec<Option<u32>> = intervals.iter().map(&get).collect();
    if firsts.iter().all(|v| v.is_none()) {
        return Some(None);
    }
    if firsts.iter().any(|v| v.is_none()) {
        return None; // mixed wildcard and value: not product-shaped
    }
    let mut vals: Vec<u32> = firsts.into_iter().flatten().collect();
    vals.sort_unstable();
    vals.dedup();
    Some(Some(vals))
}

fn card(v: &Option<Vec<u32>>) -> usize {
    v.as_ref().map(|x| x.len()).unwrap_or(1)
}

fn field_str(v: &Option<Vec<u32>>) -> String {
    match v {
        None => "*".to_string(),
        Some(vals) => vals
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(","),
    }
}

/// Rewrite a plist's schedule to the given calendar intervals (removing any
/// StartInterval). Writes XML format.
pub fn set_plist_calendar_schedule(path: &Path, intervals: &[CalendarInterval]) -> Result<()> {
    let mut value =
        plist::Value::from_file(path).with_context(|| format!("parsing {}", path.display()))?;
    let dict = value
        .as_dictionary_mut()
        .context("plist root is not a dictionary")?;
    dict.remove("StartInterval");

    let to_dict = |i: &CalendarInterval| {
        let mut d = plist::Dictionary::new();
        if let Some(m) = i.minute {
            d.insert("Minute".into(), plist::Value::Integer(m.into()));
        }
        if let Some(h) = i.hour {
            d.insert("Hour".into(), plist::Value::Integer(h.into()));
        }
        if let Some(day) = i.day {
            d.insert("Day".into(), plist::Value::Integer(day.into()));
        }
        if let Some(w) = i.weekday {
            d.insert("Weekday".into(), plist::Value::Integer(w.into()));
        }
        if let Some(mo) = i.month {
            d.insert("Month".into(), plist::Value::Integer(mo.into()));
        }
        plist::Value::Dictionary(d)
    };

    let new_value = if intervals.len() == 1 {
        to_dict(&intervals[0])
    } else {
        plist::Value::Array(intervals.iter().map(to_dict).collect())
    };
    dict.insert("StartCalendarInterval".into(), new_value);
    value
        .to_file_xml(path)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Check a plist parses; on macOS also run plutil -lint for a second opinion.
pub fn validate_plist(path: &Path) -> Result<()> {
    plist::Value::from_file(path)
        .map(|_| ())
        .with_context(|| format!("{} does not parse as a plist", path.display()))?;
    if let Ok(output) = Command::new("plutil").arg("-lint").arg(path).output()
        && !output.status.success()
    {
        bail!(
            "plutil -lint: {}",
            String::from_utf8_lossy(&output.stdout).trim()
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// editor + run-now + delete
// ---------------------------------------------------------------------------

pub fn editor_command() -> String {
    std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string())
}

/// Run the user's editor on a path. The caller must have released the
/// terminal (left raw mode / alternate screen) first.
pub fn open_in_editor(path: &Path) -> Result<()> {
    let editor = editor_command();
    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\"",))
        .arg("sh")
        .arg(path)
        .status()
        .with_context(|| format!("failed to launch editor '{editor}'"))?;
    if !status.success() {
        bail!("editor exited with {status}");
    }
    Ok(())
}

/// Run a shell command, capturing combined output (used by "run now" for cron
/// entries). Returns (exit code, tail of output).
pub fn run_shell_command(command: &str, env: &[(String, String)]) -> (i32, String) {
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c").arg(command);
    for (k, v) in env {
        if let Some(name) = k.strip_prefix("env ") {
            cmd.env(name, v);
        }
    }
    match cmd.output() {
        Ok(output) => {
            let mut text = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.trim().is_empty() {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str("[stderr]\n");
                text.push_str(&stderr);
            }
            let tail: Vec<&str> = text.lines().rev().take(200).collect();
            let tail: String = tail.into_iter().rev().collect::<Vec<_>>().join("\n");
            (output.status.code().unwrap_or(-1), tail)
        }
        Err(e) => (-1, format!("failed to start: {e}")),
    }
}

/// Move a plist to the Trash (or a .removed backup when no Trash exists).
/// Returns where the file went.
pub fn trash_plist(path: &Path) -> Result<PathBuf> {
    let name = path
        .file_name()
        .context("plist has no file name")?
        .to_string_lossy()
        .to_string();
    let trash = dirs::home_dir().map(|h| h.join(".Trash"));
    let dest_dir = match trash {
        Some(t) if t.is_dir() => t,
        _ => path
            .parent()
            .context("plist has no parent dir")?
            .to_path_buf(),
    };
    let mut dest = dest_dir.join(&name);
    if dest == *path {
        dest = dest_dir.join(format!("{name}.removed"));
    }
    let mut counter = 1;
    while dest.exists() {
        dest = dest_dir.join(format!("{name}.{counter}"));
        counter += 1;
    }
    std::fs::rename(path, &dest).or_else(|_| {
        // Cross-device fallback: copy then remove.
        std::fs::copy(path, &dest)
            .and_then(|_| std::fs::remove_file(path))
            .map(|_| ())
    })?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAB: &str = "SHELL=/bin/bash\n30 2 * * * /usr/local/bin/backup.sh --full\n@daily /usr/local/bin/sync.sh\n";

    #[test]
    fn toggles_disable_marker() {
        let disabled = toggle_cron_line(TAB, 2, true).unwrap();
        assert!(disabled.contains("# [sched:off] 30 2 * * * /usr/local/bin/backup.sh --full"));
        // Idempotent.
        let again = toggle_cron_line(&disabled, 2, true).unwrap();
        assert_eq!(disabled, again);
        let restored = toggle_cron_line(&disabled, 2, false).unwrap();
        assert_eq!(restored, TAB);
    }

    #[test]
    fn deletes_line() {
        let out = delete_cron_line(TAB, 2).unwrap();
        assert!(!out.contains("backup.sh"));
        assert!(out.contains("sync.sh"));
        assert!(out.contains("SHELL="));
    }

    #[test]
    fn line_out_of_range_errors() {
        assert!(delete_cron_line(TAB, 99).is_err());
        assert!(delete_cron_line(TAB, 0).is_err());
    }

    #[test]
    fn reschedules_plain_line() {
        let out = reschedule_cron_line(TAB, 2, "0 4 * * mon").unwrap();
        assert!(out.contains("0 4 * * mon /usr/local/bin/backup.sh --full"));
    }

    #[test]
    fn reschedules_keyword_line() {
        let out = reschedule_cron_line(TAB, 3, "15 3 * * *").unwrap();
        assert!(out.contains("15 3 * * * /usr/local/bin/sync.sh"));
        assert!(!out.contains("@daily"));
    }

    #[test]
    fn reschedules_disabled_line_keeps_marker() {
        let disabled = toggle_cron_line(TAB, 2, true).unwrap();
        let out = reschedule_cron_line(&disabled, 2, "0 5 * * *").unwrap();
        assert!(out.contains("# [sched:off] 0 5 * * * /usr/local/bin/backup.sh --full"));
    }

    #[test]
    fn rejects_bad_expression() {
        assert!(reschedule_cron_line(TAB, 2, "not a cron").is_err());
    }

    #[test]
    fn cron_to_calendar_simple() {
        let expr = CronExpr::parse("30 2 * * *").unwrap();
        let cal = cron_to_calendar(&expr).unwrap();
        assert_eq!(cal.len(), 1);
        assert_eq!(cal[0].minute, Some(30));
        assert_eq!(cal[0].hour, Some(2));
        assert_eq!(cal[0].day, None);
        assert_eq!(cal[0].weekday, None);
    }

    #[test]
    fn cron_to_calendar_steps() {
        let expr = CronExpr::parse("*/15 9 * * *").unwrap();
        let cal = cron_to_calendar(&expr).unwrap();
        assert_eq!(cal.len(), 4);
        assert_eq!(cal[0].minute, Some(0));
        assert_eq!(cal[3].minute, Some(45));
    }

    #[test]
    fn cron_to_calendar_dom_dow_or() {
        let expr = CronExpr::parse("0 0 1 * mon").unwrap();
        let cal = cron_to_calendar(&expr).unwrap();
        // One entry for day 1, one for Monday.
        assert_eq!(cal.len(), 2);
        assert!(cal.iter().any(|c| c.day == Some(1) && c.weekday.is_none()));
        assert!(cal.iter().any(|c| c.weekday == Some(1) && c.day.is_none()));
    }

    #[test]
    fn cron_to_calendar_rejects_explosion() {
        let expr = CronExpr::parse("* * * * *").unwrap();
        // All wildcards is fine: one wildcard entry.
        assert_eq!(cron_to_calendar(&expr).unwrap().len(), 1);
        // But per-minute over several hours explodes.
        let expr = CronExpr::parse("0-59 0-23 * * *").unwrap();
        assert_eq!(cron_to_calendar(&expr).unwrap().len(), 1); // still wildcards
        let expr = CronExpr::parse("1-31/2 1,2,3,4,5,6,7 * * *").unwrap();
        assert!(cron_to_calendar(&expr).is_err());
    }

    #[test]
    fn calendar_round_trip() {
        let expr = CronExpr::parse("0,30 9,17 * * *").unwrap();
        let cal = cron_to_calendar(&expr).unwrap();
        assert_eq!(cal.len(), 4);
        let back = calendar_to_cron(&cal).unwrap();
        assert_eq!(back, "0,30 9,17 * * *");
    }

    #[test]
    fn calendar_to_cron_rejects_non_product() {
        let cal = vec![
            CalendarInterval {
                minute: Some(0),
                hour: Some(9),
                day: None,
                weekday: None,
                month: None,
            },
            CalendarInterval {
                minute: Some(30),
                hour: Some(17),
                day: None,
                weekday: None,
                month: None,
            },
        ];
        assert!(calendar_to_cron(&cal).is_none());
    }

    #[test]
    fn plist_schedule_rewrite_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("com.example.test.plist");
        let mut dict = plist::Dictionary::new();
        dict.insert(
            "Label".into(),
            plist::Value::String("com.example.test".into()),
        );
        dict.insert("Program".into(), plist::Value::String("/bin/echo".into()));
        dict.insert("StartInterval".into(), plist::Value::Integer(300.into()));
        plist::Value::Dictionary(dict).to_file_xml(&path).unwrap();

        let expr = CronExpr::parse("15 6 * * sat").unwrap();
        let cal = cron_to_calendar(&expr).unwrap();
        set_plist_calendar_schedule(&path, &cal).unwrap();

        let job = crate::launchd::parse_plist_job(SourceKind::UserAgent, &path).unwrap();
        assert_eq!(job.triggers.len(), 1);
        match &job.triggers[0] {
            crate::model::Trigger::Calendar(ints) => {
                assert_eq!(ints.len(), 1);
                assert_eq!(ints[0].minute, Some(15));
                assert_eq!(ints[0].hour, Some(6));
                assert_eq!(ints[0].weekday, Some(6));
            }
            other => panic!("unexpected {other:?}"),
        }
        // StartInterval must be gone and the label preserved.
        assert_eq!(job.label, "com.example.test");
        validate_plist(&path).unwrap();
    }

    #[test]
    fn run_shell_command_captures_output() {
        let (code, out) = run_shell_command("echo hello && echo err >&2", &[]);
        assert_eq!(code, 0);
        assert!(out.contains("hello"));
        assert!(out.contains("[stderr]"));
        let (code, _) = run_shell_command("exit 3", &[]);
        assert_eq!(code, 3);
    }

    #[test]
    fn trash_falls_back_to_sibling() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("com.example.gone.plist");
        std::fs::write(&path, "x").unwrap();
        let dest = trash_plist(&path).unwrap();
        assert!(!path.exists());
        assert!(dest.exists());
    }
}
