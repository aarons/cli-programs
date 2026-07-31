mod actions;
mod cron;
mod discovery;
mod launchd;
mod model;
mod schedule;
mod ui;

use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::{Result, bail};
use chrono::{Duration, Local};
use clap::{Parser, Subcommand};
use serde::Serialize;

use discovery::DiscoveryConfig;

#[derive(Parser)]
#[command(name = "sched")]
#[command(about = "Browse and manage scheduled jobs (launchd + cron) in a TUI")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,

    /// Parse this file as the user crontab instead of `crontab -l`
    #[arg(long, global = true, hide = true)]
    crontab_file: Option<PathBuf>,

    /// Read launchd plists from this directory instead of the standard
    /// locations (repeatable)
    #[arg(long, global = true, hide = true)]
    launchd_dir: Vec<PathBuf>,

    /// Skip querying launchctl for live job status
    #[arg(long, global = true)]
    no_status: bool,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print all discovered jobs (non-interactive)
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Include Apple system jobs from /System/Library
        #[arg(long)]
        all: bool,
    },
    /// Print upcoming runs (non-interactive)
    Next {
        /// How many hours ahead to look
        #[arg(long, default_value_t = 24)]
        hours: i64,
        /// Include Apple system jobs from /System/Library
        #[arg(long)]
        all: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = DiscoveryConfig {
        crontab_file: cli.crontab_file,
        launchd_dirs: if cli.launchd_dir.is_empty() {
            None
        } else {
            Some(cli.launchd_dir)
        },
        no_status: cli.no_status,
    };

    match cli.command {
        Some(Cmd::List { json, all }) => cmd_list(&config, json, all),
        Some(Cmd::Next { hours, all }) => cmd_next(&config, hours, all),
        None => {
            if !std::io::stdout().is_terminal() {
                bail!("stdout is not a terminal; use `sched list` or `sched next` for scripting");
            }
            ui::run(config)
        }
    }
}

#[derive(Serialize)]
struct JsonJob<'a> {
    #[serde(flatten)]
    job: &'a model::Job,
    schedule: String,
    next_runs: Vec<String>,
}

/// Write to stdout, exiting quietly on a closed pipe (e.g. `sched list | head`).
fn print_out(text: &str) {
    use std::io::Write;
    let mut stdout = std::io::stdout().lock();
    if stdout.write_all(text.as_bytes()).is_err() || stdout.flush().is_err() {
        std::process::exit(0);
    }
}

fn cmd_list(config: &DiscoveryConfig, json: bool, all: bool) -> Result<()> {
    let discovered = discovery::discover(config);
    let now = Local::now();
    let jobs: Vec<&model::Job> = discovered
        .jobs
        .iter()
        .filter(|j| all || !j.kind.is_apple())
        .collect();

    if json {
        let out: Vec<JsonJob> = jobs
            .iter()
            .map(|job| JsonJob {
                job,
                schedule: job.schedule_summary(),
                next_runs: schedule::next_runs(&job.triggers, now, 3, 400)
                    .into_iter()
                    .map(|dt| dt.to_rfc3339())
                    .collect(),
            })
            .collect();
        print_out(&format!("{}\n", serde_json::to_string_pretty(&out)?));
        return Ok(());
    }

    let label_w = jobs
        .iter()
        .map(|j| j.label.chars().count())
        .max()
        .unwrap_or(10)
        .clamp(10, 40);
    let mut out = format!(
        "{:<8} {:<3} {:<label_w$}  {:<32}  {:<18}  COMMAND\n",
        "SOURCE", "", "LABEL", "SCHEDULE", "NEXT"
    );
    for job in &jobs {
        let next = schedule::next_runs(&job.triggers, now, 1, 400)
            .first()
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "-".to_string());
        let flag = if job.disabled { "off" } else { "" };
        let mut label = job.label.clone();
        if label.chars().count() > label_w {
            label = label.chars().take(label_w - 1).collect();
            label.push('…');
        }
        let mut schedule_text = job.schedule_summary();
        if schedule_text.chars().count() > 32 {
            schedule_text = schedule_text.chars().take(31).collect();
            schedule_text.push('…');
        }
        out.push_str(&format!(
            "{:<8} {:<3} {:<label_w$}  {:<32}  {:<18}  {}\n",
            job.kind.label(),
            flag,
            label,
            schedule_text,
            next,
            job.command
        ));
    }
    print_out(&out);
    for warning in &discovered.warnings {
        eprintln!("warning: {warning}");
    }
    Ok(())
}

fn cmd_next(config: &DiscoveryConfig, hours: i64, all: bool) -> Result<()> {
    let discovered = discovery::discover(config);
    let now = Local::now();
    let until = now + Duration::hours(hours.max(1));

    let mut agenda: Vec<(chrono::DateTime<Local>, &model::Job)> = Vec::new();
    for job in &discovered.jobs {
        if job.disabled || (!all && job.kind.is_apple()) {
            continue;
        }
        for dt in schedule::occurrences_in_window(&job.triggers, now, until, 500) {
            agenda.push((dt, job));
        }
    }
    agenda.sort_by_key(|(dt, _)| *dt);

    if agenda.is_empty() {
        print_out(&format!("nothing scheduled in the next {hours}h\n"));
        return Ok(());
    }
    let mut out = String::new();
    for (dt, job) in agenda {
        out.push_str(&format!(
            "{}  {:<28}  [{}]  {}\n",
            dt.format("%Y-%m-%d %H:%M"),
            job.label,
            job.kind.label(),
            job.command
        ));
    }
    print_out(&out);
    Ok(())
}
