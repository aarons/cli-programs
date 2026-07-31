//! TUI application state and event loop.

mod render;

use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Duration as ChronoDuration, Local};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::actions;
use crate::cron::CronExpr;
use crate::discovery::{self, DiscoveryConfig};
use crate::model::{Job, SourceKind, Trigger};
use crate::schedule;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Jobs,
    Timeline,
    Upcoming,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineMode {
    Day,
    Week,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusLevel {
    Info,
    Error,
}

#[derive(Debug, Clone)]
pub enum PendingAction {
    Delete(String),
    Run(String),
    Reload(String),
}

#[derive(Debug)]
pub enum Mode {
    Normal,
    Search,
    Help,
    Confirm {
        message: String,
        action: PendingAction,
    },
    Reschedule {
        job_id: String,
        buffer: String,
        cursor: usize,
    },
    Output {
        title: String,
        lines: Vec<String>,
        scroll: usize,
    },
}

/// Requests the event loop must service outside of raw mode.
enum UiRequest {
    EditCrontab { job_id: String },
    EditPlist { job_id: String },
}

/// Result of a background "run now".
struct RunResult {
    label: String,
    exit_code: i32,
    output: String,
}

pub struct TimelineState {
    pub mode: TimelineMode,
    /// Day offset from today (day mode) or week offset (week mode).
    pub offset: i64,
    pub selected: usize,
    pub row_offset: usize,
}

pub struct App {
    pub config: DiscoveryConfig,
    pub jobs: Vec<Job>,
    pub warnings: Vec<String>,
    pub now: DateTime<Local>,
    pub tab: Tab,
    pub mode: Mode,
    pub filter: String,
    pub show_apple: bool,
    /// Indices into `jobs` after filtering, in display order.
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub status: Option<(String, StatusLevel)>,
    pub timeline: TimelineState,
    pub upcoming_selected: usize,
    /// job id -> upcoming run times (cached per refresh).
    pub next_cache: HashMap<String, Vec<DateTime<Local>>>,
    /// Agenda entries for the Upcoming tab: (time, index into jobs).
    pub agenda: Vec<(DateTime<Local>, usize)>,
    pub should_quit: bool,
    bg_tx: mpsc::Sender<RunResult>,
    bg_rx: mpsc::Receiver<RunResult>,
    /// Set while a background run is in flight (shown in the status bar).
    pub running_job: Option<String>,
}

impl App {
    pub fn new(config: DiscoveryConfig) -> App {
        let (bg_tx, bg_rx) = mpsc::channel();
        App {
            config,
            jobs: Vec::new(),
            warnings: Vec::new(),
            now: Local::now(),
            tab: Tab::Jobs,
            mode: Mode::Normal,
            filter: String::new(),
            show_apple: false,
            filtered: Vec::new(),
            selected: 0,
            status: None,
            timeline: TimelineState {
                mode: TimelineMode::Day,
                offset: 0,
                selected: 0,
                row_offset: 0,
            },
            upcoming_selected: 0,
            next_cache: HashMap::new(),
            agenda: Vec::new(),
            should_quit: false,
            bg_tx,
            bg_rx,
            running_job: None,
        }
    }

    pub fn reload(&mut self) {
        let prev_id = self.selected_job().map(|j| j.id.clone());
        let discovered = discovery::discover(&self.config);
        self.jobs = discovered.jobs;
        self.warnings = discovered.warnings;
        self.now = Local::now();
        self.next_cache.clear();
        self.apply_filter();
        self.rebuild_agenda();
        if let Some(id) = prev_id {
            self.select_job_id(&id);
        }
        if !self.warnings.is_empty() {
            self.set_status(
                format!(
                    "{} source warning{} (see detail with W)",
                    self.warnings.len(),
                    if self.warnings.len() == 1 { "" } else { "s" }
                ),
                StatusLevel::Info,
            );
        }
    }

    pub fn apply_filter(&mut self) {
        let needle = self.filter.to_lowercase();
        self.filtered = self
            .jobs
            .iter()
            .enumerate()
            .filter(|(_, job)| self.show_apple || !job.kind.is_apple())
            .filter(|(_, job)| {
                needle.is_empty()
                    || job.label.to_lowercase().contains(&needle)
                    || job.command.to_lowercase().contains(&needle)
                    || job.kind.label().contains(&needle)
            })
            .map(|(i, _)| i)
            .collect();
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn rebuild_agenda(&mut self) {
        let from = self.now;
        let until = from + ChronoDuration::days(7);
        let mut agenda: Vec<(DateTime<Local>, usize)> = Vec::new();
        for &idx in &self.filtered {
            let job = &self.jobs[idx];
            if job.disabled || !job.has_timed_trigger() {
                continue;
            }
            for dt in schedule::occurrences_in_window(&job.triggers, from, until, 200) {
                agenda.push((dt, idx));
            }
        }
        agenda.sort_by_key(|(dt, _)| *dt);
        agenda.truncate(1000);
        self.agenda = agenda;
        if self.upcoming_selected >= self.agenda.len() {
            self.upcoming_selected = self.agenda.len().saturating_sub(1);
        }
    }

    pub fn selected_job(&self) -> Option<&Job> {
        self.filtered.get(self.selected).map(|&i| &self.jobs[i])
    }

    pub fn select_job_id(&mut self, id: &str) {
        if let Some(pos) = self.filtered.iter().position(|&i| self.jobs[i].id == id) {
            self.selected = pos;
        }
    }

    pub fn next_runs_for(&mut self, job_idx: usize, count: usize) -> Vec<DateTime<Local>> {
        let job = &self.jobs[job_idx];
        let id = job.id.clone();
        if let Some(cached) = self.next_cache.get(&id) {
            return cached.iter().take(count).copied().collect();
        }
        let runs = schedule::next_runs(&job.triggers, self.now, 5, 400);
        let result = runs.iter().take(count).copied().collect();
        self.next_cache.insert(id, runs);
        result
    }

    pub fn set_status(&mut self, message: String, level: StatusLevel) {
        self.status = Some((message, level));
    }

    /// Jobs shown on the timeline: filtered, enabled jobs with timed triggers.
    pub fn timeline_jobs(&self) -> Vec<usize> {
        self.filtered
            .iter()
            .copied()
            .filter(|&i| self.jobs[i].has_timed_trigger() && !self.jobs[i].disabled)
            .collect()
    }

    fn drain_background(&mut self) {
        while let Ok(result) = self.bg_rx.try_recv() {
            self.running_job = None;
            let level = if result.exit_code == 0 {
                StatusLevel::Info
            } else {
                StatusLevel::Error
            };
            self.set_status(
                format!("{} exited with code {}", result.label, result.exit_code),
                level,
            );
            let mut lines: Vec<String> =
                vec![format!("exit code: {}", result.exit_code), String::new()];
            if result.output.is_empty() {
                lines.push("(no output)".to_string());
            } else {
                lines.extend(result.output.lines().map(str::to_string));
            }
            self.mode = Mode::Output {
                title: format!("run: {}", result.label),
                lines,
                scroll: 0,
            };
        }
    }

    // -- key handling -----------------------------------------------------

    fn on_key_normal(&mut self, key: KeyEvent) -> Option<UiRequest> {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Char('1') => self.tab = Tab::Jobs,
            KeyCode::Char('2') => self.tab = Tab::Timeline,
            KeyCode::Char('3') => self.tab = Tab::Upcoming,
            KeyCode::Tab => {
                self.tab = match self.tab {
                    Tab::Jobs => Tab::Timeline,
                    Tab::Timeline => Tab::Upcoming,
                    Tab::Upcoming => Tab::Jobs,
                }
            }
            KeyCode::BackTab => {
                self.tab = match self.tab {
                    Tab::Jobs => Tab::Upcoming,
                    Tab::Timeline => Tab::Jobs,
                    Tab::Upcoming => Tab::Timeline,
                }
            }
            KeyCode::Char('/') => {
                self.mode = Mode::Search;
            }
            KeyCode::Char('a') => {
                self.show_apple = !self.show_apple;
                self.apply_filter();
                self.rebuild_agenda();
                let state = if self.show_apple { "shown" } else { "hidden" };
                self.set_status(format!("Apple system jobs {state}"), StatusLevel::Info);
            }
            KeyCode::Char('R') => {
                self.reload();
                self.set_status("refreshed".to_string(), StatusLevel::Info);
            }
            KeyCode::Char('W') => {
                if self.warnings.is_empty() {
                    self.set_status("no warnings".to_string(), StatusLevel::Info);
                } else {
                    self.mode = Mode::Output {
                        title: "source warnings".to_string(),
                        lines: self.warnings.clone(),
                        scroll: 0,
                    };
                }
            }
            _ => return self.on_key_tab(key),
        }
        None
    }

    fn on_key_tab(&mut self, key: KeyEvent) -> Option<UiRequest> {
        match self.tab {
            Tab::Jobs => self.on_key_jobs(key),
            Tab::Timeline => {
                self.on_key_timeline(key);
                None
            }
            Tab::Upcoming => {
                self.on_key_upcoming(key);
                None
            }
        }
    }

    fn on_key_jobs(&mut self, key: KeyEvent) -> Option<UiRequest> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.selected + 1 < self.filtered.len() {
                    self.selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Char('g') | KeyCode::Home => self.selected = 0,
            KeyCode::Char('G') | KeyCode::End => {
                self.selected = self.filtered.len().saturating_sub(1)
            }
            KeyCode::PageDown => {
                self.selected = (self.selected + 10).min(self.filtered.len().saturating_sub(1))
            }
            KeyCode::PageUp => self.selected = self.selected.saturating_sub(10),
            KeyCode::Char('e') => return self.request_edit(),
            KeyCode::Char('s') => self.start_reschedule(),
            KeyCode::Char('x') => self.toggle_selected(),
            KeyCode::Char('r') => self.confirm_run(),
            KeyCode::Char('D') => self.confirm_delete(),
            KeyCode::Char('l') => self.view_logs(),
            _ => {}
        }
        None
    }

    fn on_key_timeline(&mut self, key: KeyEvent) {
        let rows = self.timeline_jobs().len();
        match key.code {
            KeyCode::Char('d') => self.timeline.mode = TimelineMode::Day,
            KeyCode::Char('w') => self.timeline.mode = TimelineMode::Week,
            KeyCode::Char('t') => self.timeline.offset = 0,
            KeyCode::Char('h') | KeyCode::Left => self.timeline.offset -= 1,
            KeyCode::Char('l') | KeyCode::Right => self.timeline.offset += 1,
            KeyCode::Char('j') | KeyCode::Down => {
                if self.timeline.selected + 1 < rows {
                    self.timeline.selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.timeline.selected = self.timeline.selected.saturating_sub(1)
            }
            KeyCode::Enter => {
                let rows = self.timeline_jobs();
                if let Some(&job_idx) = rows.get(self.timeline.selected) {
                    let id = self.jobs[job_idx].id.clone();
                    self.tab = Tab::Jobs;
                    self.select_job_id(&id);
                }
            }
            _ => {}
        }
    }

    fn on_key_upcoming(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.upcoming_selected + 1 < self.agenda.len() {
                    self.upcoming_selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.upcoming_selected = self.upcoming_selected.saturating_sub(1)
            }
            KeyCode::Char('g') | KeyCode::Home => self.upcoming_selected = 0,
            KeyCode::Char('G') | KeyCode::End => {
                self.upcoming_selected = self.agenda.len().saturating_sub(1)
            }
            KeyCode::PageDown => {
                self.upcoming_selected =
                    (self.upcoming_selected + 10).min(self.agenda.len().saturating_sub(1))
            }
            KeyCode::PageUp => self.upcoming_selected = self.upcoming_selected.saturating_sub(10),
            KeyCode::Enter => {
                if let Some(&(_, job_idx)) = self.agenda.get(self.upcoming_selected) {
                    let id = self.jobs[job_idx].id.clone();
                    self.tab = Tab::Jobs;
                    self.select_job_id(&id);
                }
            }
            _ => {}
        }
    }

    // -- actions ----------------------------------------------------------

    fn request_edit(&mut self) -> Option<UiRequest> {
        let job = self.selected_job()?;
        let id = job.id.clone();
        match job.kind {
            SourceKind::UserCrontab => Some(UiRequest::EditCrontab { job_id: id }),
            k if k.is_launchd() && k.is_editable() => Some(UiRequest::EditPlist { job_id: id }),
            k if k.is_apple() => {
                self.set_status(
                    "Apple system jobs are SIP-protected and read-only".to_string(),
                    StatusLevel::Error,
                );
                None
            }
            _ => {
                let path = job.source_path.display().to_string();
                self.set_status(
                    format!("{path} requires root; edit it with sudo in a shell"),
                    StatusLevel::Error,
                );
                None
            }
        }
    }

    fn start_reschedule(&mut self) {
        let Some(job) = self.selected_job() else {
            return;
        };
        if !job.kind.is_editable() {
            self.set_status(
                format!("{} jobs can't be rescheduled here", job.kind.description()),
                StatusLevel::Error,
            );
            return;
        }
        let id = job.id.clone();
        let buffer = match job.cron_expr() {
            Some(expr) => expr.raw.clone(),
            None => {
                let calendar = job.triggers.iter().find_map(|t| match t {
                    Trigger::Calendar(ints) => Some(ints.clone()),
                    _ => None,
                });
                calendar
                    .and_then(|ints| actions::calendar_to_cron(&ints))
                    .unwrap_or_default()
            }
        };
        let cursor = buffer.len();
        self.mode = Mode::Reschedule {
            job_id: id,
            buffer,
            cursor,
        };
    }

    fn toggle_selected(&mut self) {
        let Some(job) = self.selected_job() else {
            return;
        };
        let job = job.clone();
        let result = if job.kind.is_cron() {
            self.toggle_cron_job(&job)
        } else if job.kind.is_editable() {
            if job.disabled {
                actions::enable_launchd_job(&job)
                    .map(|m| m.lines().next().unwrap_or("").to_string())
            } else {
                actions::disable_launchd_job(&job)
                    .map(|m| m.lines().next().unwrap_or("").to_string())
            }
        } else {
            Err(anyhow::anyhow!(
                "{} jobs can't be toggled here",
                job.kind.description()
            ))
        };
        match result {
            Ok(msg) => {
                self.reload();
                self.set_status(msg, StatusLevel::Info);
            }
            Err(e) => self.set_status(format!("{e:#}"), StatusLevel::Error),
        }
    }

    fn toggle_cron_job(&self, job: &Job) -> Result<String> {
        if job.kind != SourceKind::UserCrontab {
            anyhow::bail!("system crontabs require root; edit them with sudo");
        }
        let line_no = job
            .line_no
            .ok_or_else(|| anyhow::anyhow!("missing line number"))?;
        let text = actions::read_user_crontab_text(&self.config)?;
        verify_line(&text, line_no, job)?;
        let updated = actions::toggle_cron_line(&text, line_no, !job.disabled)?;
        actions::install_user_crontab(&self.config, &updated)?;
        Ok(format!(
            "{} {}",
            if job.disabled { "enabled" } else { "disabled" },
            job.label
        ))
    }

    fn confirm_run(&mut self) {
        let Some(job) = self.selected_job() else {
            return;
        };
        let (id, label) = (job.id.clone(), job.label.clone());
        let how = if job.kind.is_cron() {
            "via /bin/sh"
        } else {
            "via launchctl kickstart"
        };
        self.mode = Mode::Confirm {
            message: format!("Run '{label}' now ({how})?"),
            action: PendingAction::Run(id),
        };
    }

    fn confirm_delete(&mut self) {
        let Some(job) = self.selected_job() else {
            return;
        };
        if !job.kind.is_editable() {
            self.set_status(
                format!("{} jobs can't be deleted here", job.kind.description()),
                StatusLevel::Error,
            );
            return;
        }
        let (id, label) = (job.id.clone(), job.label.clone());
        let what = if job.kind.is_cron() {
            "remove the crontab line"
        } else {
            "unload it and move the plist to Trash"
        };
        self.mode = Mode::Confirm {
            message: format!("Delete '{label}'? This will {what}."),
            action: PendingAction::Delete(id),
        };
    }

    fn view_logs(&mut self) {
        let Some(job) = self.selected_job() else {
            return;
        };
        let mut paths: Vec<(String, String)> = Vec::new();
        for (k, v) in &job.extras {
            if k == "StandardOutPath" || k == "StandardErrorPath" {
                paths.push((k.clone(), v.clone()));
            }
        }
        if paths.is_empty() {
            self.set_status(
                "job has no StandardOutPath/StandardErrorPath".to_string(),
                StatusLevel::Info,
            );
            return;
        }
        let mut lines = Vec::new();
        for (name, path) in paths {
            lines.push(format!("── {name}: {path}"));
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let tail: Vec<&str> = content.lines().rev().take(100).collect();
                    lines.extend(tail.into_iter().rev().map(str::to_string));
                }
                Err(e) => lines.push(format!("(unreadable: {e})")),
            }
            lines.push(String::new());
        }
        self.mode = Mode::Output {
            title: format!(
                "logs: {}",
                self.selected_job()
                    .map(|j| j.label.clone())
                    .unwrap_or_default()
            ),
            lines,
            scroll: 0,
        };
    }

    fn execute_pending(&mut self, action: PendingAction) {
        match action {
            PendingAction::Run(id) => self.run_job(&id),
            PendingAction::Delete(id) => self.delete_job(&id),
            PendingAction::Reload(id) => {
                let Some(job) = self.jobs.iter().find(|j| j.id == id).cloned() else {
                    return;
                };
                match actions::reload_launchd_job(&job) {
                    Ok(msg) => {
                        self.reload();
                        self.set_status(msg, StatusLevel::Info);
                    }
                    Err(e) => self.set_status(format!("{e:#}"), StatusLevel::Error),
                }
            }
        }
    }

    fn run_job(&mut self, id: &str) {
        let Some(job) = self.jobs.iter().find(|j| j.id == id).cloned() else {
            return;
        };
        if job.kind.is_cron() {
            let tx = self.bg_tx.clone();
            let command = job.command.clone();
            let env = job.extras.clone();
            let label = job.label.clone();
            self.running_job = Some(label.clone());
            std::thread::spawn(move || {
                let (exit_code, output) = actions::run_shell_command(&command, &env);
                let _ = tx.send(RunResult {
                    label,
                    exit_code,
                    output,
                });
            });
            self.set_status(format!("running {}…", job.label), StatusLevel::Info);
        } else {
            match actions::launchctl_kickstart(&job) {
                Ok(_) => self.set_status(format!("kickstarted {}", job.label), StatusLevel::Info),
                Err(e) => self.set_status(format!("{e:#}"), StatusLevel::Error),
            }
        }
    }

    fn delete_job(&mut self, id: &str) {
        let Some(job) = self.jobs.iter().find(|j| j.id == id).cloned() else {
            return;
        };
        let result: Result<String> = (|| {
            if job.kind.is_cron() {
                let line_no = job
                    .line_no
                    .ok_or_else(|| anyhow::anyhow!("missing line number"))?;
                let text = actions::read_user_crontab_text(&self.config)?;
                verify_line(&text, line_no, &job)?;
                let updated = actions::delete_cron_line(&text, line_no)?;
                actions::install_user_crontab(&self.config, &updated)?;
                Ok(format!("removed {} from crontab", job.label))
            } else {
                let _ = actions::launchctl_bootout(&job);
                let dest = actions::trash_plist(&job.source_path)?;
                Ok(format!("moved {} to {}", job.label, dest.display()))
            }
        })();
        match result {
            Ok(msg) => {
                self.reload();
                self.set_status(msg, StatusLevel::Info);
            }
            Err(e) => self.set_status(format!("{e:#}"), StatusLevel::Error),
        }
    }

    fn apply_reschedule(&mut self, job_id: &str, expr_text: &str) {
        let Some(job) = self.jobs.iter().find(|j| j.id == job_id).cloned() else {
            return;
        };
        let result: Result<String> = (|| {
            let expr = CronExpr::parse(expr_text)?;
            if job.kind.is_cron() {
                let line_no = job
                    .line_no
                    .ok_or_else(|| anyhow::anyhow!("missing line number"))?;
                let text = actions::read_user_crontab_text(&self.config)?;
                verify_line(&text, line_no, &job)?;
                let updated = actions::reschedule_cron_line(&text, line_no, expr_text)?;
                actions::install_user_crontab(&self.config, &updated)?;
                Ok(format!("rescheduled {}: {}", job.label, expr.describe()))
            } else {
                let intervals = actions::cron_to_calendar(&expr)?;
                actions::set_plist_calendar_schedule(&job.source_path, &intervals)?;
                Ok(format!(
                    "rescheduled {}: {} ({} calendar entr{})",
                    job.label,
                    expr.describe(),
                    intervals.len(),
                    if intervals.len() == 1 { "y" } else { "ies" }
                ))
            }
        })();
        match result {
            Ok(msg) => {
                let is_launchd = job.kind.is_launchd();
                self.reload();
                self.set_status(msg, StatusLevel::Info);
                if is_launchd {
                    self.mode = Mode::Confirm {
                        message: format!("Reload '{}' into launchd now?", job.label),
                        action: PendingAction::Reload(job_id.to_string()),
                    };
                }
            }
            Err(e) => self.set_status(format!("{e:#}"), StatusLevel::Error),
        }
    }

    /// Handle a key event; may return a request that needs the terminal released.
    fn on_key(&mut self, key: KeyEvent) -> Option<UiRequest> {
        if key.kind != KeyEventKind::Press {
            return None;
        }
        // Ctrl-C always quits.
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return None;
        }
        self.status = None;
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Normal => return self.on_key_normal(key),
            Mode::Help => {
                // Any key closes help.
            }
            Mode::Search => match key.code {
                KeyCode::Esc => {
                    self.filter.clear();
                    self.apply_filter();
                    self.rebuild_agenda();
                }
                KeyCode::Enter => {}
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.apply_filter();
                    self.rebuild_agenda();
                    self.mode = Mode::Search;
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                    self.apply_filter();
                    self.rebuild_agenda();
                    self.mode = Mode::Search;
                }
                _ => self.mode = Mode::Search,
            },
            Mode::Confirm { message, action } => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    self.execute_pending(action);
                }
                _ => {
                    let _ = message;
                }
            },
            Mode::Reschedule {
                job_id,
                mut buffer,
                mut cursor,
            } => {
                match key.code {
                    KeyCode::Esc => return None,
                    KeyCode::Enter => {
                        if CronExpr::parse(&buffer).is_ok() {
                            self.apply_reschedule(&job_id, &buffer.clone());
                            return None;
                        }
                        // Invalid: stay in the editor.
                        self.mode = Mode::Reschedule {
                            job_id,
                            buffer,
                            cursor,
                        };
                        return None;
                    }
                    KeyCode::Backspace => {
                        if cursor > 0 {
                            buffer.remove(cursor - 1);
                            cursor -= 1;
                        }
                    }
                    KeyCode::Delete => {
                        if cursor < buffer.len() {
                            buffer.remove(cursor);
                        }
                    }
                    KeyCode::Left => cursor = cursor.saturating_sub(1),
                    KeyCode::Right => cursor = (cursor + 1).min(buffer.len()),
                    KeyCode::Home => cursor = 0,
                    KeyCode::End => cursor = buffer.len(),
                    KeyCode::Char(c) if c.is_ascii() => {
                        buffer.insert(cursor, c);
                        cursor += 1;
                    }
                    _ => {}
                }
                self.mode = Mode::Reschedule {
                    job_id,
                    buffer,
                    cursor,
                };
            }
            Mode::Output {
                title,
                lines,
                mut scroll,
            } => match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {}
                KeyCode::Char('j') | KeyCode::Down => {
                    scroll = (scroll + 1).min(lines.len().saturating_sub(1));
                    self.mode = Mode::Output {
                        title,
                        lines,
                        scroll,
                    };
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    scroll = scroll.saturating_sub(1);
                    self.mode = Mode::Output {
                        title,
                        lines,
                        scroll,
                    };
                }
                KeyCode::PageDown => {
                    scroll = (scroll + 20).min(lines.len().saturating_sub(1));
                    self.mode = Mode::Output {
                        title,
                        lines,
                        scroll,
                    };
                }
                KeyCode::PageUp => {
                    scroll = scroll.saturating_sub(20);
                    self.mode = Mode::Output {
                        title,
                        lines,
                        scroll,
                    };
                }
                KeyCode::Char('g') => {
                    self.mode = Mode::Output {
                        title,
                        lines,
                        scroll: 0,
                    };
                }
                _ => {
                    self.mode = Mode::Output {
                        title,
                        lines,
                        scroll,
                    };
                }
            },
        }
        None
    }
}

/// Guard against acting on a stale line number: the line must still contain
/// the job's original text.
fn verify_line(text: &str, line_no: usize, job: &Job) -> Result<()> {
    let line = text
        .lines()
        .nth(line_no.saturating_sub(1))
        .unwrap_or_default();
    match &job.raw_line {
        Some(raw) if line.trim_end() == raw.trim_end() => Ok(()),
        _ => anyhow::bail!("crontab changed since last refresh; press R to refresh and retry"),
    }
}

/// Handle a request that needs the terminal released (editor sessions).
fn service_request(app: &mut App, request: UiRequest) -> Result<()> {
    match request {
        UiRequest::EditCrontab { job_id } => {
            let text = actions::read_user_crontab_text(&app.config)?;
            let tmp =
                std::env::temp_dir().join(format!("sched-edit-{}.crontab", std::process::id()));
            std::fs::write(&tmp, &text)?;
            let edit_result = actions::open_in_editor(&tmp);
            let outcome: Result<String> = edit_result.and_then(|_| {
                let edited = std::fs::read_to_string(&tmp)?;
                if edited == text {
                    return Ok("no changes".to_string());
                }
                let parsed = crate::cron::parse_crontab(&edited, false);
                if !parsed.errors.is_empty() {
                    let (line_no, _, err) = &parsed.errors[0];
                    anyhow::bail!("line {line_no}: {err} — crontab NOT installed");
                }
                actions::install_user_crontab(&app.config, &edited)?;
                Ok("crontab updated".to_string())
            });
            let _ = std::fs::remove_file(&tmp);
            app.reload();
            app.select_job_id(&job_id);
            match outcome {
                Ok(msg) => app.set_status(msg, StatusLevel::Info),
                Err(e) => app.set_status(format!("{e:#}"), StatusLevel::Error),
            }
        }
        UiRequest::EditPlist { job_id } => {
            let Some(job) = app.jobs.iter().find(|j| j.id == job_id).cloned() else {
                return Ok(());
            };
            let before = std::fs::read(&job.source_path).unwrap_or_default();
            let outcome: Result<bool> = actions::open_in_editor(&job.source_path).and_then(|_| {
                let after = std::fs::read(&job.source_path).unwrap_or_default();
                if before == after {
                    return Ok(false);
                }
                actions::validate_plist(&job.source_path)?;
                Ok(true)
            });
            app.reload();
            app.select_job_id(&job_id);
            match outcome {
                Ok(false) => app.set_status("no changes".to_string(), StatusLevel::Info),
                Ok(true) => {
                    app.set_status("plist updated".to_string(), StatusLevel::Info);
                    app.mode = Mode::Confirm {
                        message: format!("Reload '{}' into launchd now?", job.label),
                        action: PendingAction::Reload(job_id),
                    };
                }
                Err(e) => app.set_status(format!("{e:#}"), StatusLevel::Error),
            }
        }
    }
    Ok(())
}

pub fn run(config: DiscoveryConfig) -> Result<()> {
    let mut app = App::new(config);
    app.reload();

    let mut terminal = ratatui::init();
    let result = (|| -> Result<()> {
        loop {
            terminal.draw(|frame| render::draw(frame, &mut app))?;
            if event::poll(Duration::from_millis(250))?
                && let Event::Key(key) = event::read()?
                && let Some(request) = app.on_key(key)
            {
                ratatui::restore();
                let service_result = service_request(&mut app, request);
                terminal = ratatui::init();
                if let Err(e) = service_result {
                    app.set_status(format!("{e:#}"), StatusLevel::Error);
                }
            }
            let now = Local::now();
            if now.format("%H:%M").to_string() != app.now.format("%H:%M").to_string() {
                // Minute rolled over: refresh time-relative data.
                app.now = now;
                app.next_cache.clear();
                app.rebuild_agenda();
            }
            app.drain_background();
            if app.should_quit {
                return Ok(());
            }
        }
    })();
    ratatui::restore();
    result
}
