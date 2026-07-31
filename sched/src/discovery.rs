//! Assembling jobs from every source on the system.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cron::{self, CronSchedule, CrontabFile};
use crate::launchd;
use crate::model::{Job, RuntimeStatus, SourceKind, Trigger};

#[derive(Debug, Clone, Default)]
pub struct DiscoveryConfig {
    /// Parse this file as the user crontab instead of running `crontab -l`.
    pub crontab_file: Option<PathBuf>,
    /// Use these directories (treated as user LaunchAgents) instead of the
    /// standard launchd locations.
    pub launchd_dirs: Option<Vec<PathBuf>>,
    /// Skip querying launchctl for live status.
    pub no_status: bool,
}

pub struct Discovered {
    pub jobs: Vec<Job>,
    pub warnings: Vec<String>,
}

pub fn discover(config: &DiscoveryConfig) -> Discovered {
    let mut jobs: Vec<Job> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // --- user crontab ---
    match read_user_crontab(config) {
        Ok(Some((source, text))) => {
            let tab = cron::parse_crontab(&text, false);
            collect_cron_jobs(&mut jobs, &tab, SourceKind::UserCrontab, &source, None);
            for (line_no, line, err) in &tab.errors {
                warnings.push(format!("{}:{line_no}: {err} ({line})", source.display()));
            }
        }
        Ok(None) => {}
        Err(e) => warnings.push(format!("user crontab: {e:#}")),
    }

    // --- system crontab + cron.d ---
    if config.crontab_file.is_none() {
        let etc_crontab = Path::new("/etc/crontab");
        if let Ok(text) = std::fs::read_to_string(etc_crontab) {
            let tab = cron::parse_crontab(&text, true);
            collect_cron_jobs(
                &mut jobs,
                &tab,
                SourceKind::SystemCrontab,
                etc_crontab,
                None,
            );
        }
        if let Ok(entries) = std::fs::read_dir("/etc/cron.d") {
            let mut paths: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect();
            paths.sort();
            for path in paths {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    let tab = cron::parse_crontab(&text, true);
                    collect_cron_jobs(&mut jobs, &tab, SourceKind::CronD, &path, None);
                }
            }
        }
    }

    // --- launchd plists ---
    let dirs: Vec<(SourceKind, PathBuf)> = match &config.launchd_dirs {
        Some(dirs) => dirs
            .iter()
            .map(|d| (SourceKind::UserAgent, d.clone()))
            .collect(),
        None => {
            let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
            launchd::standard_dirs(&home)
        }
    };
    for (kind, dir) in dirs {
        let (found, errors) = launchd::discover_dir(kind, &dir);
        jobs.extend(found);
        for (path, err) in errors {
            warnings.push(format!("{}: {err}", path.display()));
        }
    }

    // --- live status ---
    if !config.no_status
        && let Some(state) = launchd::load_runtime_status()
    {
        for job in &mut jobs {
            state.apply(job);
        }
    }

    jobs.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
    });

    Discovered { jobs, warnings }
}

fn collect_cron_jobs(
    jobs: &mut Vec<Job>,
    tab: &CrontabFile,
    kind: SourceKind,
    source: &Path,
    _user: Option<&str>,
) {
    let env_extras: Vec<(String, String)> = tab
        .env
        .iter()
        .map(|(k, v)| (format!("env {k}"), v.clone()))
        .collect();
    for entry in &tab.entries {
        let triggers = match &entry.schedule {
            CronSchedule::Expr(expr) => vec![Trigger::Cron(expr.clone())],
            CronSchedule::Reboot => vec![Trigger::Reboot],
        };
        jobs.push(Job {
            id: format!("cron:{}:{}", source.display(), entry.command),
            kind,
            label: cron::label_from_command(&entry.command),
            command: entry.command.clone(),
            triggers,
            source_path: source.to_path_buf(),
            line_no: Some(entry.line_no),
            disabled: entry.disabled,
            status: RuntimeStatus::default(),
            raw_line: Some(entry.raw.clone()),
            cron_user: entry.user.clone(),
            extras: env_extras.clone(),
        });
    }
}

/// Read the user's crontab. Returns (display path, content), or None when the
/// user has no crontab.
fn read_user_crontab(config: &DiscoveryConfig) -> anyhow::Result<Option<(PathBuf, String)>> {
    if let Some(path) = &config.crontab_file {
        let text = std::fs::read_to_string(path)?;
        return Ok(Some((path.clone(), text)));
    }
    let output = match Command::new("crontab").arg("-l").output() {
        Ok(o) => o,
        Err(_) => return Ok(None), // no crontab binary on this system
    };
    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(Some((PathBuf::from("crontab (user)"), text)))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("no crontab") {
            Ok(None)
        } else {
            anyhow::bail!("crontab -l failed: {}", stderr.trim())
        }
    }
}
