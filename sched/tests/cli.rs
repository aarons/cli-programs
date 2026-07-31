use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;

const CRONTAB: &str = "\
SHELL=/bin/bash
30 2 * * * /usr/local/bin/backup.sh --full
*/5 * * * * /usr/local/bin/poll.sh
# [sched:off] 0 3 * * 0 /usr/local/bin/cleanup.sh
";

const PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.example.reporter</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/report</string>
        <string>--weekly</string>
    </array>
    <key>StartCalendarInterval</key>
    <dict>
        <key>Weekday</key>
        <integer>1</integer>
        <key>Hour</key>
        <integer>9</integer>
        <key>Minute</key>
        <integer>0</integer>
    </dict>
</dict>
</plist>"#;

struct Fixture {
    _dir: tempfile::TempDir,
    crontab: std::path::PathBuf,
    launchd: std::path::PathBuf,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let crontab = dir.path().join("crontab");
    std::fs::write(&crontab, CRONTAB).unwrap();
    let launchd = dir.path().join("LaunchAgents");
    std::fs::create_dir(&launchd).unwrap();
    std::fs::write(launchd.join("com.example.reporter.plist"), PLIST).unwrap();
    Fixture {
        _dir: dir,
        crontab,
        launchd,
    }
}

fn sched() -> Command {
    cargo_bin_cmd!("sched")
}

#[test]
fn list_shows_all_sources() {
    let fx = fixture();
    sched()
        .args(["list", "--no-status"])
        .arg("--crontab-file")
        .arg(&fx.crontab)
        .arg("--launchd-dir")
        .arg(&fx.launchd)
        .assert()
        .success()
        .stdout(predicate::str::contains("backup.sh"))
        .stdout(predicate::str::contains("poll.sh"))
        .stdout(predicate::str::contains("com.example.reporter"))
        .stdout(predicate::str::contains("daily at 02:30"))
        .stdout(predicate::str::contains("every 5 min"))
        .stdout(predicate::str::contains("at 09:00 on Monday"));
}

#[test]
fn list_marks_disabled_jobs() {
    let fx = fixture();
    sched()
        .args(["list", "--no-status"])
        .arg("--crontab-file")
        .arg(&fx.crontab)
        .arg("--launchd-dir")
        .arg(&fx.launchd)
        .assert()
        .success()
        .stdout(predicate::str::contains("cleanup.sh"))
        .stdout(predicate::str::contains("off"));
}

#[test]
fn list_json_is_parseable() {
    let fx = fixture();
    let output = sched()
        .args(["list", "--json", "--no-status"])
        .arg("--crontab-file")
        .arg(&fx.crontab)
        .arg("--launchd-dir")
        .arg(&fx.launchd)
        .output()
        .unwrap();
    assert!(output.status.success());
    let jobs: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let jobs = jobs.as_array().unwrap();
    assert_eq!(jobs.len(), 4);
    let reporter = jobs
        .iter()
        .find(|j| j["label"] == "com.example.reporter")
        .unwrap();
    assert_eq!(reporter["kind"], "UserAgent");
    assert!(!reporter["next_runs"].as_array().unwrap().is_empty());
    let poll = jobs.iter().find(|j| j["label"] == "poll.sh").unwrap();
    assert_eq!(poll["disabled"], false);
    assert_eq!(poll["line_no"], 3);
}

#[test]
fn next_lists_upcoming_runs_chronologically() {
    let fx = fixture();
    let output = sched()
        .args(["next", "--hours", "24", "--no-status"])
        .arg("--crontab-file")
        .arg(&fx.crontab)
        .arg("--launchd-dir")
        .arg(&fx.launchd)
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    // The 5-minute poller dominates the next 24h.
    assert!(stdout.contains("poll.sh"));
    // Disabled jobs don't appear.
    assert!(!stdout.contains("cleanup.sh"));
    // Timestamps are sorted.
    let stamps: Vec<&str> = stdout.lines().filter_map(|l| l.get(0..16)).collect();
    let mut sorted = stamps.clone();
    sorted.sort();
    assert_eq!(stamps, sorted);
}

#[test]
fn help_and_version_work() {
    sched()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Browse and manage scheduled jobs"));
    sched().arg("--version").assert().success();
}

#[test]
fn tui_refuses_non_terminal_stdout() {
    let fx = fixture();
    sched()
        .arg("--no-status")
        .arg("--crontab-file")
        .arg(&fx.crontab)
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a terminal"));
}
