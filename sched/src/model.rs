use std::path::PathBuf;

use serde::Serialize;

use crate::cron::CronExpr;

/// Where a job definition lives on the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum SourceKind {
    UserAgent,
    GlobalAgent,
    GlobalDaemon,
    UserCrontab,
    SystemCrontab,
    CronD,
    SystemAgent,
    SystemDaemon,
}

impl SourceKind {
    pub fn label(&self) -> &'static str {
        match self {
            SourceKind::UserCrontab => "crontab",
            SourceKind::SystemCrontab => "sys cron",
            SourceKind::CronD => "cron.d",
            SourceKind::UserAgent => "agent",
            SourceKind::GlobalAgent => "agent*",
            SourceKind::SystemAgent => "apple",
            SourceKind::GlobalDaemon => "daemon",
            SourceKind::SystemDaemon => "apple-d",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            SourceKind::UserCrontab => "User crontab",
            SourceKind::SystemCrontab => "System crontab (/etc/crontab)",
            SourceKind::CronD => "Cron drop-in (/etc/cron.d)",
            SourceKind::UserAgent => "User LaunchAgent (~/Library/LaunchAgents)",
            SourceKind::GlobalAgent => "Global LaunchAgent (/Library/LaunchAgents)",
            SourceKind::SystemAgent => "Apple LaunchAgent (/System/Library/LaunchAgents)",
            SourceKind::GlobalDaemon => "Global LaunchDaemon (/Library/LaunchDaemons)",
            SourceKind::SystemDaemon => "Apple LaunchDaemon (/System/Library/LaunchDaemons)",
        }
    }

    /// Apple-provided jobs under /System are SIP-protected and read-only.
    pub fn is_apple(&self) -> bool {
        matches!(self, SourceKind::SystemAgent | SourceKind::SystemDaemon)
    }

    pub fn is_cron(&self) -> bool {
        matches!(
            self,
            SourceKind::UserCrontab | SourceKind::SystemCrontab | SourceKind::CronD
        )
    }

    pub fn is_launchd(&self) -> bool {
        !self.is_cron()
    }

    /// Daemons live in the system launchd domain (root); agents in the gui domain.
    pub fn is_daemon(&self) -> bool {
        matches!(self, SourceKind::GlobalDaemon | SourceKind::SystemDaemon)
    }

    /// Whether sched supports modifying this job.
    pub fn is_editable(&self) -> bool {
        matches!(
            self,
            SourceKind::UserCrontab
                | SourceKind::UserAgent
                | SourceKind::GlobalAgent
                | SourceKind::GlobalDaemon
        )
    }
}

/// A launchd StartCalendarInterval entry. Missing fields are wildcards.
/// weekday: 0 and 7 are Sunday.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CalendarInterval {
    pub minute: Option<u32>,
    pub hour: Option<u32>,
    pub day: Option<u32>,
    pub weekday: Option<u32>,
    pub month: Option<u32>,
}

impl CalendarInterval {
    pub fn describe(&self) -> String {
        let time = match (self.hour, self.minute) {
            (Some(h), Some(m)) => format!("at {h:02}:{m:02}"),
            (Some(h), None) => format!("every minute of hour {h:02}"),
            (None, Some(m)) => format!("at minute {m} of every hour"),
            (None, None) => "every minute".to_string(),
        };
        let mut parts = vec![time];
        if let Some(d) = self.day {
            parts.push(format!("on day {d}"));
        }
        if let Some(w) = self.weekday {
            parts.push(format!("on {}", weekday_name(w)));
        }
        if let Some(mo) = self.month {
            parts.push(format!("in {}", month_name(mo)));
        }
        parts.join(" ")
    }
}

pub fn weekday_name(w: u32) -> &'static str {
    match w % 7 {
        0 => "Sunday",
        1 => "Monday",
        2 => "Tuesday",
        3 => "Wednesday",
        4 => "Thursday",
        5 => "Friday",
        6 => "Saturday",
        _ => unreachable!(),
    }
}

pub fn month_name(m: u32) -> &'static str {
    match m {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "?",
    }
}

/// What causes a job to run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Trigger {
    /// Classic 5-field cron schedule.
    Cron(CronExpr),
    /// launchd StartCalendarInterval (one or more entries, OR'd together).
    Calendar(Vec<CalendarInterval>),
    /// launchd StartInterval: every N seconds while loaded.
    Interval { seconds: u64 },
    /// launchd WatchPaths: runs when a path changes.
    Watch { paths: Vec<String> },
    /// launchd QueueDirectories: runs while directories are non-empty.
    Queue { paths: Vec<String> },
    /// Runs when the job is loaded (login/boot).
    RunAtLoad,
    /// launchd keeps the process alive continuously.
    KeepAlive,
    /// cron @reboot.
    Reboot,
    /// On-demand activation (sockets, mach services, launch events...).
    OnDemand(String),
}

impl Trigger {
    /// Triggers that produce plottable points in time.
    pub fn is_timed(&self) -> bool {
        matches!(
            self,
            Trigger::Cron(_) | Trigger::Calendar(_) | Trigger::Interval { .. }
        )
    }

    pub fn describe(&self) -> String {
        match self {
            Trigger::Cron(expr) => expr.describe(),
            Trigger::Calendar(intervals) => {
                let descs: Vec<String> = intervals.iter().map(|i| i.describe()).collect();
                descs.join("; ")
            }
            Trigger::Interval { seconds } => format!("every {}", human_duration(*seconds)),
            Trigger::Watch { paths } => format!("on change of {}", paths.join(", ")),
            Trigger::Queue { paths } => format!("while non-empty: {}", paths.join(", ")),
            Trigger::RunAtLoad => "at load (login/boot)".to_string(),
            Trigger::KeepAlive => "kept alive (always running)".to_string(),
            Trigger::Reboot => "at reboot".to_string(),
            Trigger::OnDemand(what) => format!("on demand ({what})"),
        }
    }

    /// Compact summary for list rows.
    pub fn summary(&self) -> String {
        match self {
            Trigger::Cron(expr) => expr.describe(),
            Trigger::Calendar(intervals) => {
                if intervals.len() == 1 {
                    intervals[0].describe()
                } else {
                    format!("{} calendar times", intervals.len())
                }
            }
            Trigger::Interval { seconds } => format!("every {}", human_duration(*seconds)),
            Trigger::Watch { .. } => "watch paths".to_string(),
            Trigger::Queue { .. } => "queue dirs".to_string(),
            Trigger::RunAtLoad => "at load".to_string(),
            Trigger::KeepAlive => "keep alive".to_string(),
            Trigger::Reboot => "@reboot".to_string(),
            Trigger::OnDemand(_) => "on demand".to_string(),
        }
    }
}

pub fn human_duration(seconds: u64) -> String {
    if seconds == 0 {
        return "0s".to_string();
    }
    if seconds.is_multiple_of(86400) {
        let d = seconds / 86400;
        return format!("{d} day{}", if d == 1 { "" } else { "s" });
    }
    if seconds.is_multiple_of(3600) {
        let h = seconds / 3600;
        return format!("{h} hour{}", if h == 1 { "" } else { "s" });
    }
    if seconds.is_multiple_of(60) {
        let m = seconds / 60;
        return format!("{m} minute{}", if m == 1 { "" } else { "s" });
    }
    format!("{seconds} seconds")
}

/// Live state reported by launchctl (launchd jobs only).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RuntimeStatus {
    /// Whether the job is loaded into launchd. None = unknown.
    pub loaded: Option<bool>,
    /// PID if currently running.
    pub pid: Option<i64>,
    /// Last exit status if known.
    pub last_exit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Job {
    /// Stable-ish identifier used to reselect after refresh.
    pub id: String,
    pub kind: SourceKind,
    pub label: String,
    pub command: String,
    pub triggers: Vec<Trigger>,
    /// Plist path, crontab file path, or "crontab:<user>" pseudo-path.
    pub source_path: PathBuf,
    /// 1-based line number for cron entries.
    pub line_no: Option<usize>,
    /// Disabled via plist key, launchctl disable, or sched's crontab marker.
    pub disabled: bool,
    pub status: RuntimeStatus,
    /// Original crontab line, for cron jobs.
    pub raw_line: Option<String>,
    /// User a system crontab entry runs as.
    pub cron_user: Option<String>,
    /// Extra key/value details for the detail pane.
    pub extras: Vec<(String, String)>,
}

impl Job {
    pub fn schedule_summary(&self) -> String {
        let timed: Vec<&Trigger> = self.triggers.iter().filter(|t| t.is_timed()).collect();
        let pick = if timed.is_empty() {
            self.triggers.first()
        } else {
            timed.first().copied().map(Some).unwrap_or(None)
        };
        match pick {
            Some(t) => {
                let extra = self.triggers.len().saturating_sub(1);
                if extra > 0 {
                    format!("{} (+{extra})", t.summary())
                } else {
                    t.summary()
                }
            }
            None => "no trigger".to_string(),
        }
    }

    pub fn has_timed_trigger(&self) -> bool {
        self.triggers.iter().any(|t| t.is_timed())
    }

    /// The cron expression, if this job is scheduled by exactly cron-style fields.
    pub fn cron_expr(&self) -> Option<&CronExpr> {
        self.triggers.iter().find_map(|t| match t {
            Trigger::Cron(e) => Some(e),
            _ => None,
        })
    }

    pub fn is_running(&self) -> bool {
        self.status.pid.is_some()
    }
}
