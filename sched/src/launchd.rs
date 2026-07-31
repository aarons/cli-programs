//! launchd plist discovery and parsing, plus launchctl status integration.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use plist::Value;

use crate::model::{CalendarInterval, Job, RuntimeStatus, SourceKind, Trigger};

/// Standard launchd job directories for a given home directory.
pub fn standard_dirs(home: &Path) -> Vec<(SourceKind, PathBuf)> {
    vec![
        (
            SourceKind::UserAgent,
            home.join("Library").join("LaunchAgents"),
        ),
        (
            SourceKind::GlobalAgent,
            PathBuf::from("/Library/LaunchAgents"),
        ),
        (
            SourceKind::GlobalDaemon,
            PathBuf::from("/Library/LaunchDaemons"),
        ),
        (
            SourceKind::SystemAgent,
            PathBuf::from("/System/Library/LaunchAgents"),
        ),
        (
            SourceKind::SystemDaemon,
            PathBuf::from("/System/Library/LaunchDaemons"),
        ),
    ]
}

/// Read all plists in a directory into jobs. Unreadable files become error entries.
pub fn discover_dir(kind: SourceKind, dir: &Path) -> (Vec<Job>, Vec<(PathBuf, String)>) {
    let mut jobs = Vec::new();
    let mut errors = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return (jobs, errors), // missing dir is normal
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("plist"))
        .collect();
    paths.sort();
    for path in paths {
        match parse_plist_job(kind, &path) {
            Ok(job) => jobs.push(job),
            Err(e) => errors.push((path, format!("{e:#}"))),
        }
    }
    (jobs, errors)
}

/// Parse a single launchd plist file into a Job.
pub fn parse_plist_job(kind: SourceKind, path: &Path) -> Result<Job> {
    let value = Value::from_file(path)
        .with_context(|| format!("failed to parse plist {}", path.display()))?;
    let dict = value
        .as_dictionary()
        .context("plist root is not a dictionary")?;

    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    let label = dict
        .get("Label")
        .and_then(Value::as_string)
        .map(str::to_string)
        .unwrap_or(file_stem);

    let command = extract_command(dict);
    let triggers = extract_triggers(dict);
    let disabled = dict
        .get("Disabled")
        .and_then(Value::as_boolean)
        .unwrap_or(false);

    let mut extras: Vec<(String, String)> = Vec::new();
    for key in [
        "WorkingDirectory",
        "StandardOutPath",
        "StandardErrorPath",
        "UserName",
        "GroupName",
        "ProcessType",
        "ThrottleInterval",
    ] {
        if let Some(v) = dict.get(key) {
            extras.push((key.to_string(), plist_scalar_to_string(v)));
        }
    }
    if let Some(env) = dict
        .get("EnvironmentVariables")
        .and_then(Value::as_dictionary)
    {
        for (k, v) in env {
            extras.push((format!("env {k}"), plist_scalar_to_string(v)));
        }
    }

    Ok(Job {
        id: format!("{}", path.display()),
        kind,
        label,
        command,
        triggers,
        source_path: path.to_path_buf(),
        line_no: None,
        disabled,
        status: RuntimeStatus::default(),
        raw_line: None,
        cron_user: dict
            .get("UserName")
            .and_then(Value::as_string)
            .map(str::to_string),
        extras,
    })
}

fn extract_command(dict: &plist::Dictionary) -> String {
    if let Some(args) = dict.get("ProgramArguments").and_then(Value::as_array) {
        let parts: Vec<String> = args
            .iter()
            .filter_map(Value::as_string)
            .map(shell_quote)
            .collect();
        if !parts.is_empty() {
            // If Program is also present it overrides argv[0].
            if let Some(prog) = dict.get("Program").and_then(Value::as_string) {
                let mut out = vec![shell_quote(prog)];
                out.extend(parts.into_iter().skip(1));
                return out.join(" ");
            }
            return parts.join(" ");
        }
    }
    if let Some(prog) = dict.get("Program").and_then(Value::as_string) {
        return prog.to_string();
    }
    String::new()
}

fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    let safe = s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_./=+:@%,".contains(c));
    if safe {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

fn extract_triggers(dict: &plist::Dictionary) -> Vec<Trigger> {
    let mut out = Vec::new();

    if let Some(v) = dict.get("StartCalendarInterval") {
        let intervals = parse_calendar_intervals(v);
        if !intervals.is_empty() {
            out.push(Trigger::Calendar(intervals));
        }
    }
    if let Some(secs) = dict.get("StartInterval").and_then(as_integer)
        && secs > 0
    {
        out.push(Trigger::Interval {
            seconds: secs as u64,
        });
    }
    if let Some(paths) = dict.get("WatchPaths").and_then(Value::as_array) {
        let paths: Vec<String> = paths
            .iter()
            .filter_map(Value::as_string)
            .map(str::to_string)
            .collect();
        if !paths.is_empty() {
            out.push(Trigger::Watch { paths });
        }
    }
    if let Some(paths) = dict.get("QueueDirectories").and_then(Value::as_array) {
        let paths: Vec<String> = paths
            .iter()
            .filter_map(Value::as_string)
            .map(str::to_string)
            .collect();
        if !paths.is_empty() {
            out.push(Trigger::Queue { paths });
        }
    }
    if dict
        .get("RunAtLoad")
        .and_then(Value::as_boolean)
        .unwrap_or(false)
    {
        out.push(Trigger::RunAtLoad);
    }
    match dict.get("KeepAlive") {
        Some(Value::Boolean(true)) => out.push(Trigger::KeepAlive),
        Some(Value::Dictionary(_)) => out.push(Trigger::KeepAlive),
        _ => {}
    }
    for (key, name) in [
        ("Sockets", "sockets"),
        ("MachServices", "mach services"),
        ("LaunchEvents", "launch events"),
    ] {
        if dict.get(key).is_some() {
            out.push(Trigger::OnDemand(name.to_string()));
        }
    }
    out
}

fn as_integer(v: &Value) -> Option<i64> {
    match v {
        Value::Integer(i) => i.as_signed(),
        Value::Real(r) => Some(*r as i64),
        _ => None,
    }
}

/// StartCalendarInterval may be a single dict or an array of dicts. Values are
/// integers per the spec, but arrays of integers appear in the wild; those are
/// expanded into the cartesian product of entries.
fn parse_calendar_intervals(v: &Value) -> Vec<CalendarInterval> {
    match v {
        Value::Dictionary(d) => expand_calendar_dict(d),
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_dictionary)
            .flat_map(expand_calendar_dict)
            .collect(),
        _ => Vec::new(),
    }
}

fn expand_calendar_dict(d: &plist::Dictionary) -> Vec<CalendarInterval> {
    fn values_for(d: &plist::Dictionary, key: &str, max: u32) -> Option<Vec<u32>> {
        let v = d.get(key)?;
        let vals: Vec<u32> = match v {
            Value::Array(items) => items
                .iter()
                .filter_map(as_integer)
                .filter(|i| *i >= 0 && *i <= max as i64)
                .map(|i| i as u32)
                .collect(),
            other => as_integer(other)
                .filter(|i| *i >= 0 && *i <= max as i64)
                .map(|i| vec![i as u32])
                .unwrap_or_default(),
        };
        if vals.is_empty() { None } else { Some(vals) }
    }

    let minutes = values_for(d, "Minute", 59);
    let hours = values_for(d, "Hour", 23);
    let days = values_for(d, "Day", 31);
    let weekdays = values_for(d, "Weekday", 7);
    let months = values_for(d, "Month", 12);

    // Cartesian product over present fields; absent = wildcard (single None).
    fn axis(vals: &Option<Vec<u32>>) -> Vec<Option<u32>> {
        match vals {
            Some(v) => v.iter().map(|x| Some(*x)).collect(),
            None => vec![None],
        }
    }

    let mut out = Vec::new();
    for mi in axis(&minutes) {
        for h in axis(&hours) {
            for day in axis(&days) {
                for w in axis(&weekdays) {
                    for mo in axis(&months) {
                        out.push(CalendarInterval {
                            minute: mi,
                            hour: h,
                            day,
                            weekday: w,
                            month: mo,
                        });
                        if out.len() >= 400 {
                            return out;
                        }
                    }
                }
            }
        }
    }
    out
}

fn plist_scalar_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Boolean(b) => b.to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Real(r) => r.to_string(),
        other => format!("{other:?}"),
    }
}

/// Parsed output of `launchctl list`: label -> (pid, last exit status).
pub fn parse_launchctl_list(output: &str) -> HashMap<String, (Option<i64>, Option<i64>)> {
    let mut map = HashMap::new();
    for line in output.lines().skip(1) {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 3 {
            continue;
        }
        let pid = fields[0].trim().parse::<i64>().ok();
        let status = fields[1].trim().parse::<i64>().ok();
        let label = fields[2].trim().to_string();
        if !label.is_empty() {
            map.insert(label, (pid, status));
        }
    }
    map
}

/// Parsed output of `launchctl print-disabled <domain>`: set of disabled labels.
pub fn parse_print_disabled(output: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        // Lines look like: "com.example.foo" => disabled
        // or (newer):      "com.example.foo" => true
        if let Some((name_part, state)) = line.split_once("=>") {
            let name = name_part.trim().trim_matches('"');
            let state = state.trim().trim_end_matches(';');
            if !name.is_empty() && (state == "disabled" || state == "true") {
                out.push(name.to_string());
            }
        }
    }
    out
}

/// Query launchctl for job status. Returns None if launchctl is unavailable
/// (e.g. when developing on Linux).
pub fn load_runtime_status() -> Option<LaunchdState> {
    let list = std::process::Command::new("launchctl")
        .arg("list")
        .output()
        .ok()?;
    if !list.status.success() {
        return None;
    }
    let by_label = parse_launchctl_list(&String::from_utf8_lossy(&list.stdout));

    let mut disabled = Vec::new();
    let uid = current_uid();
    for domain in [format!("gui/{uid}"), "system".to_string()] {
        if let Ok(out) = std::process::Command::new("launchctl")
            .args(["print-disabled", &domain])
            .output()
            && out.status.success()
        {
            disabled.extend(parse_print_disabled(&String::from_utf8_lossy(&out.stdout)));
        }
    }
    Some(LaunchdState { by_label, disabled })
}

pub struct LaunchdState {
    pub by_label: HashMap<String, (Option<i64>, Option<i64>)>,
    pub disabled: Vec<String>,
}

impl LaunchdState {
    pub fn apply(&self, job: &mut Job) {
        if !job.kind.is_launchd() {
            return;
        }
        match self.by_label.get(&job.label) {
            Some((pid, status)) => {
                job.status.loaded = Some(true);
                job.status.pid = *pid;
                job.status.last_exit = *status;
            }
            None => job.status.loaded = Some(false),
        }
        if self.disabled.iter().any(|l| l == &job.label) {
            job.disabled = true;
        }
    }
}

pub fn current_uid() -> u32 {
    // Avoid a libc dependency: id -u works everywhere we care about.
    std::process::Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(501)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CALENDAR_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.example.backup</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/backup.sh</string>
        <string>--full</string>
        <string>my file.txt</string>
    </array>
    <key>StartCalendarInterval</key>
    <dict>
        <key>Hour</key>
        <integer>2</integer>
        <key>Minute</key>
        <integer>30</integer>
    </dict>
    <key>StandardOutPath</key>
    <string>/tmp/backup.log</string>
</dict>
</plist>"#;

    const INTERVAL_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.example.poller</string>
    <key>Program</key>
    <string>/usr/local/bin/poll</string>
    <key>StartInterval</key>
    <integer>300</integer>
    <key>RunAtLoad</key>
    <true/>
    <key>Disabled</key>
    <true/>
</dict>
</plist>"#;

    fn write_plist(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn parses_calendar_plist() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_plist(dir.path(), "com.example.backup.plist", CALENDAR_PLIST);
        let job = parse_plist_job(SourceKind::UserAgent, &path).unwrap();
        assert_eq!(job.label, "com.example.backup");
        assert_eq!(job.command, "/usr/local/bin/backup.sh --full 'my file.txt'");
        assert!(!job.disabled);
        assert_eq!(job.triggers.len(), 1);
        match &job.triggers[0] {
            Trigger::Calendar(ints) => {
                assert_eq!(ints.len(), 1);
                assert_eq!(ints[0].hour, Some(2));
                assert_eq!(ints[0].minute, Some(30));
                assert_eq!(ints[0].day, None);
            }
            other => panic!("unexpected trigger {other:?}"),
        }
        assert!(
            job.extras
                .iter()
                .any(|(k, v)| k == "StandardOutPath" && v == "/tmp/backup.log")
        );
    }

    #[test]
    fn parses_interval_plist() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_plist(dir.path(), "com.example.poller.plist", INTERVAL_PLIST);
        let job = parse_plist_job(SourceKind::UserAgent, &path).unwrap();
        assert_eq!(job.label, "com.example.poller");
        assert_eq!(job.command, "/usr/local/bin/poll");
        assert!(job.disabled);
        assert!(job.triggers.contains(&Trigger::Interval { seconds: 300 }));
        assert!(job.triggers.contains(&Trigger::RunAtLoad));
    }

    #[test]
    fn expands_calendar_arrays() {
        let plist_text = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
    <key>Label</key><string>com.example.multi</string>
    <key>Program</key><string>/bin/echo</string>
    <key>StartCalendarInterval</key>
    <array>
        <dict>
            <key>Minute</key><array><integer>0</integer><integer>30</integer></array>
            <key>Hour</key><integer>9</integer>
        </dict>
        <dict>
            <key>Weekday</key><integer>0</integer>
            <key>Hour</key><integer>12</integer>
            <key>Minute</key><integer>0</integer>
        </dict>
    </array>
</dict>
</plist>"#;
        let dir = tempfile::tempdir().unwrap();
        let path = write_plist(dir.path(), "com.example.multi.plist", plist_text);
        let job = parse_plist_job(SourceKind::UserAgent, &path).unwrap();
        match &job.triggers[0] {
            Trigger::Calendar(ints) => {
                assert_eq!(ints.len(), 3); // {0,30}x9:00 expanded + sunday noon
                assert_eq!(ints[0].minute, Some(0));
                assert_eq!(ints[1].minute, Some(30));
                assert_eq!(ints[2].weekday, Some(0));
            }
            other => panic!("unexpected trigger {other:?}"),
        }
    }

    #[test]
    fn discovers_directory_and_reports_errors() {
        let dir = tempfile::tempdir().unwrap();
        write_plist(dir.path(), "a.plist", CALENDAR_PLIST);
        write_plist(dir.path(), "broken.plist", "not a plist at all");
        write_plist(dir.path(), "ignored.txt", "nope");
        let (jobs, errors) = discover_dir(SourceKind::UserAgent, dir.path());
        assert_eq!(jobs.len(), 1);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].0.ends_with("broken.plist"));
    }

    #[test]
    fn parses_launchctl_list_output() {
        let output = "PID\tStatus\tLabel\n\
                      312\t0\tcom.example.running\n\
                      -\t78\tcom.example.failed\n\
                      -\t0\tcom.example.idle\n";
        let map = parse_launchctl_list(output);
        assert_eq!(map.len(), 3);
        assert_eq!(map["com.example.running"], (Some(312), Some(0)));
        assert_eq!(map["com.example.failed"], (None, Some(78)));
    }

    #[test]
    fn parses_print_disabled_output() {
        let output = r#"disabled services = {
        "com.example.one" => disabled
        "com.example.two" => enabled
        "com.example.three" => true
    }"#;
        let names = parse_print_disabled(output);
        assert_eq!(names, vec!["com.example.one", "com.example.three"]);
    }
}
