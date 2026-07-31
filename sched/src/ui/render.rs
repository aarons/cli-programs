//! All drawing code for the TUI.

use chrono::{Datelike, Duration as ChronoDuration, Local, NaiveDate, Timelike};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};

use crate::cron::CronExpr;
use crate::model::{Job, SourceKind, Trigger};
use crate::schedule;

use super::{App, Mode, StatusLevel, Tab, TimelineMode};

const DIM: Color = Color::DarkGray;

fn kind_color(kind: SourceKind) -> Color {
    match kind {
        SourceKind::UserCrontab => Color::Yellow,
        SourceKind::SystemCrontab | SourceKind::CronD => Color::LightYellow,
        SourceKind::UserAgent => Color::Cyan,
        SourceKind::GlobalAgent => Color::Blue,
        SourceKind::GlobalDaemon => Color::Magenta,
        SourceKind::SystemAgent | SourceKind::SystemDaemon => Color::DarkGray,
    }
}

fn status_symbol(job: &Job) -> (&'static str, Color) {
    if job.disabled {
        ("✗", Color::Red)
    } else if job.is_running() {
        ("▶", Color::Green)
    } else if job.status.loaded == Some(true) {
        if matches!(job.status.last_exit, Some(code) if code != 0) {
            ("●", Color::LightRed)
        } else {
            ("●", Color::Green)
        }
    } else if job.status.loaded == Some(false) {
        ("○", DIM)
    } else {
        ("·", DIM)
    }
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    draw_header(frame, app, header);
    match app.tab {
        Tab::Jobs => draw_jobs_tab(frame, app, body),
        Tab::Timeline => draw_timeline_tab(frame, app, body),
        Tab::Upcoming => draw_upcoming_tab(frame, app, body),
    }
    draw_footer(frame, app, footer);

    match &app.mode {
        Mode::Help => draw_help(frame),
        Mode::Confirm { message, .. } => draw_confirm(frame, message),
        Mode::Reschedule {
            buffer,
            cursor,
            job_id,
        } => {
            let label = app
                .jobs
                .iter()
                .find(|j| j.id == *job_id)
                .map(|j| j.label.clone())
                .unwrap_or_default();
            draw_reschedule(frame, app, &label, buffer, *cursor);
        }
        Mode::Output {
            title,
            lines,
            scroll,
        } => draw_output(frame, title, lines, *scroll),
        _ => {}
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![Span::styled(
        " sched ",
        Style::new()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )];
    for (tab, num, name) in [
        (Tab::Jobs, "1", "Jobs"),
        (Tab::Timeline, "2", "Timeline"),
        (Tab::Upcoming, "3", "Upcoming"),
    ] {
        spans.push(Span::raw(" "));
        let style = if app.tab == tab {
            Style::new()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::new().fg(DIM)
        };
        spans.push(Span::styled(format!("{num}:{name}"), style));
    }
    let left = Line::from(spans);

    let running = app
        .jobs
        .iter()
        .filter(|j| j.is_running() && (app.show_apple || !j.kind.is_apple()))
        .count();
    let right_text = format!(
        "{} jobs ({} shown, {} running)  {} ",
        app.jobs.len(),
        app.filtered.len(),
        running,
        app.now.format("%a %b %d  %H:%M"),
    );
    let right = Line::from(Span::styled(right_text, Style::new().fg(DIM))).right_aligned();

    frame.render_widget(Paragraph::new(left), area);
    frame.render_widget(Paragraph::new(right), area);
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    if let Mode::Search = app.mode {
        let line = Line::from(vec![
            Span::styled(" /", Style::new().fg(Color::Yellow)),
            Span::raw(app.filter.clone()),
            Span::styled("▌", Style::new().fg(Color::Yellow)),
            Span::styled("  (enter keep, esc clear)", Style::new().fg(DIM)),
        ]);
        frame.render_widget(Paragraph::new(line), area);
        return;
    }
    if let Some((message, level)) = &app.status {
        let color = match level {
            StatusLevel::Info => Color::Green,
            StatusLevel::Error => Color::Red,
        };
        let mut text = format!(" {message}");
        if let Some(label) = &app.running_job {
            text.push_str(&format!("  (running {label}…)"));
        }
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(text, Style::new().fg(color)))),
            area,
        );
        return;
    }
    let hints = match app.tab {
        Tab::Jobs => {
            " j/k move  / filter  e edit  s reschedule  x on/off  r run  D delete  l logs  a apple  R refresh  ? help  q quit"
        }
        Tab::Timeline => {
            " d day  w week  ←/→ shift window  t today  j/k row  enter open job  ? help  q quit"
        }
        Tab::Upcoming => " j/k move  enter open job  / filter  a apple  ? help  q quit",
    };
    let mut line = vec![Span::styled(hints, Style::new().fg(DIM))];
    if let Some(label) = &app.running_job {
        line.push(Span::styled(
            format!("  ⏳ {label}"),
            Style::new().fg(Color::Yellow),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(line)), area);
}

// ---------------------------------------------------------------------------
// Jobs tab
// ---------------------------------------------------------------------------

fn draw_jobs_tab(frame: &mut Frame, app: &mut App, area: Rect) {
    let [list_area, detail_area] =
        Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).areas(area);

    // Gather detail data first (needs &mut for the next-run cache).
    let next_runs = app
        .filtered
        .get(app.selected)
        .copied()
        .map(|idx| app.next_runs_for(idx, 5))
        .unwrap_or_default();

    draw_job_list(frame, app, list_area);
    draw_job_detail(frame, app, detail_area, &next_runs);
}

fn draw_job_list(frame: &mut Frame, app: &App, area: Rect) {
    let inner_width = area.width.saturating_sub(2) as usize;
    let tag_w = 8usize;
    let label_w = (inner_width / 2).clamp(8, 30);
    let sched_w = inner_width.saturating_sub(label_w + tag_w + 5);

    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .map(|&idx| {
            let job = &app.jobs[idx];
            let (sym, sym_color) = status_symbol(job);
            let label = truncate(&job.label, label_w);
            let label_style = if job.disabled {
                Style::new().fg(DIM).add_modifier(Modifier::CROSSED_OUT)
            } else {
                Style::new()
            };
            let pad = label_w.saturating_sub(label.chars().count());
            let mut spans = vec![
                Span::styled(format!(" {sym} "), Style::new().fg(sym_color)),
                Span::styled(label, label_style),
                Span::raw(" ".repeat(pad + 1)),
            ];
            if sched_w >= 6 {
                let sched = truncate(&job.schedule_summary(), sched_w);
                let pad = sched_w.saturating_sub(sched.chars().count());
                spans.push(Span::styled(sched, Style::new().fg(DIM)));
                spans.push(Span::raw(" ".repeat(pad + 1)));
            }
            spans.push(Span::styled(
                format!("{:>tag_w$}", job.kind.label()),
                Style::new().fg(kind_color(job.kind)),
            ));
            ListItem::new(Line::from(spans))
        })
        .collect();

    let title = if app.filter.is_empty() {
        format!(" jobs ({}) ", app.filtered.len())
    } else {
        format!(" jobs ({}) — /{} ", app.filtered.len(), app.filter)
    };
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::new()
                .bg(Color::Rgb(50, 50, 70))
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    if !app.filtered.is_empty() {
        state.select(Some(app.selected));
    }
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_job_detail(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    next_runs: &[chrono::DateTime<Local>],
) {
    let block = Block::default().borders(Borders::ALL).title(" detail ");
    let Some(job) = app.selected_job() else {
        frame.render_widget(
            Paragraph::new("no jobs match")
                .block(block)
                .style(Style::new().fg(DIM)),
            area,
        );
        return;
    };

    let key_style = Style::new().fg(DIM);
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled(
            job.label.clone(),
            Style::new()
                .add_modifier(Modifier::BOLD)
                .fg(kind_color(job.kind)),
        ),
        Span::raw("  "),
        Span::styled(
            if job.disabled { "[disabled]" } else { "" },
            Style::new().fg(Color::Red),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        job.kind.description(),
        Style::new().fg(DIM),
    )));
    lines.push(Line::default());

    let mut file = job.source_path.display().to_string();
    if let Some(line_no) = job.line_no {
        file.push_str(&format!(":{line_no}"));
    }
    lines.push(Line::from(vec![
        Span::styled("file      ", key_style),
        Span::raw(file),
    ]));

    if job.kind.is_launchd() {
        let loaded = match job.status.loaded {
            Some(true) => "loaded".to_string(),
            Some(false) => "not loaded".to_string(),
            None => "unknown".to_string(),
        };
        let mut status = loaded;
        if let Some(pid) = job.status.pid {
            status.push_str(&format!(", running (pid {pid})"));
        } else if let Some(code) = job.status.last_exit {
            status.push_str(&format!(", last exit {code}"));
        }
        let color = if job.is_running() {
            Color::Green
        } else if matches!(job.status.last_exit, Some(c) if c != 0) {
            Color::LightRed
        } else {
            Color::Reset
        };
        lines.push(Line::from(vec![
            Span::styled("status    ", key_style),
            Span::styled(status, Style::new().fg(color)),
        ]));
    }
    if let Some(user) = &job.cron_user {
        lines.push(Line::from(vec![
            Span::styled("user      ", key_style),
            Span::raw(user.clone()),
        ]));
    }
    lines.push(Line::default());

    if job.triggers.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("schedule  ", key_style),
            Span::styled("none (on-demand only)", Style::new().fg(DIM)),
        ]));
    }
    for (i, trigger) in job.triggers.iter().enumerate() {
        let prefix = if i == 0 { "schedule  " } else { "          " };
        let color = if trigger.is_timed() {
            Color::Cyan
        } else {
            Color::Reset
        };
        lines.push(Line::from(vec![
            Span::styled(prefix, key_style),
            Span::styled(trigger.describe(), Style::new().fg(color)),
        ]));
        if let Trigger::Cron(expr) = trigger {
            lines.push(Line::from(vec![
                Span::styled("          ", key_style),
                Span::styled(format!("({})", expr.raw), Style::new().fg(DIM)),
            ]));
        }
    }

    if !next_runs.is_empty() {
        lines.push(Line::default());
        for (i, dt) in next_runs.iter().enumerate() {
            let prefix = if i == 0 { "next      " } else { "          " };
            lines.push(Line::from(vec![
                Span::styled(prefix, key_style),
                Span::raw(format!("{}", dt.format("%a %b %d  %H:%M"))),
                Span::styled(
                    format!("  ({})", schedule::human_until(*dt, app.now)),
                    Style::new().fg(DIM),
                ),
            ]));
        }
    } else if job.has_timed_trigger() && !job.disabled {
        lines.push(Line::default());
        lines.push(Line::from(vec![
            Span::styled("next      ", key_style),
            Span::styled("nothing in the next 400 days", Style::new().fg(DIM)),
        ]));
    }

    lines.push(Line::default());
    lines.push(Line::from(vec![
        Span::styled("command   ", key_style),
        Span::raw(if job.command.is_empty() {
            "(none)".to_string()
        } else {
            job.command.clone()
        }),
    ]));

    if let Some(raw) = &job.raw_line {
        lines.push(Line::default());
        lines.push(Line::from(vec![
            Span::styled("raw       ", key_style),
            Span::styled(raw.clone(), Style::new().fg(DIM)),
        ]));
    }

    if !job.extras.is_empty() {
        lines.push(Line::default());
        for (k, v) in &job.extras {
            lines.push(Line::from(vec![
                Span::styled(format!("{k:<9} ",), key_style),
                Span::styled(v.clone(), Style::new().fg(DIM)),
            ]));
        }
    }

    if !job.kind.is_editable() {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            if job.kind.is_apple() {
                "read-only: Apple system job (SIP-protected)"
            } else {
                "read-only here: requires root"
            },
            Style::new().fg(Color::Yellow),
        )));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

// ---------------------------------------------------------------------------
// Timeline tab
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Cell {
    Empty,
    Grid,
    Marker(u32),
    Now,
}

fn draw_timeline_tab(frame: &mut Frame, app: &mut App, area: Rect) {
    match app.timeline.mode {
        TimelineMode::Day => draw_timeline_day(frame, app, area),
        TimelineMode::Week => draw_timeline_week(frame, app, area),
    }
}

fn timeline_layout(area: Rect) -> (usize, usize) {
    let inner_w = area.width.saturating_sub(2) as usize;
    let label_w = (inner_w / 3).clamp(12, 26);
    let grid_w = inner_w.saturating_sub(label_w + 1).max(24);
    (label_w, grid_w)
}

fn clamp_row_window(app: &mut App, rows: usize, visible: usize) {
    if app.timeline.selected >= rows {
        app.timeline.selected = rows.saturating_sub(1);
    }
    if app.timeline.selected < app.timeline.row_offset {
        app.timeline.row_offset = app.timeline.selected;
    }
    if visible > 0 && app.timeline.selected >= app.timeline.row_offset + visible {
        app.timeline.row_offset = app.timeline.selected + 1 - visible;
    }
}

fn cells_to_line(cells: &[Cell], base_color: Color) -> Vec<Span<'static>> {
    let mut spans: Vec<Span> = Vec::new();
    let mut current: Option<(Cell, String)> = None;
    for &cell in cells {
        let ch = match cell {
            Cell::Empty => ' ',
            Cell::Grid => '·',
            Cell::Now => '│',
            Cell::Marker(1) => '●',
            Cell::Marker(n) if n <= 9 => char::from_digit(n, 10).unwrap_or('+'),
            Cell::Marker(_) => '+',
        };
        match &mut current {
            Some((kind, text)) if same_style(*kind, cell) => text.push(ch),
            _ => {
                if let Some((kind, text)) = current.take() {
                    spans.push(styled_span(kind, text, base_color));
                }
                current = Some((cell, ch.to_string()));
            }
        }
    }
    if let Some((kind, text)) = current {
        spans.push(styled_span(kind, text, base_color));
    }
    spans
}

fn same_style(a: Cell, b: Cell) -> bool {
    matches!(
        (a, b),
        (Cell::Empty, Cell::Empty)
            | (Cell::Grid, Cell::Grid)
            | (Cell::Now, Cell::Now)
            | (Cell::Marker(_), Cell::Marker(_))
    )
}

fn styled_span(kind: Cell, text: String, base_color: Color) -> Span<'static> {
    match kind {
        Cell::Empty => Span::raw(text),
        Cell::Grid => Span::styled(text, Style::new().fg(Color::Rgb(60, 60, 60))),
        Cell::Now => Span::styled(text, Style::new().fg(Color::Yellow)),
        Cell::Marker(_) => Span::styled(
            text,
            Style::new().fg(base_color).add_modifier(Modifier::BOLD),
        ),
    }
}

fn draw_timeline_day(frame: &mut Frame, app: &mut App, area: Rect) {
    let date = app.now.date_naive() + ChronoDuration::days(app.timeline.offset);
    let title = format!(
        " timeline — {} {}",
        date.format("%A %b %d %Y"),
        if app.timeline.offset == 0 {
            "(today) "
        } else {
            " "
        }
    );
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let (label_w, grid_w) = timeline_layout(area);
    let rows = app.timeline_jobs();
    let visible = inner.height.saturating_sub(1) as usize; // one line for the axis
    clamp_row_window(app, rows.len(), visible);

    let mut lines: Vec<Line> = Vec::new();

    // Hour axis.
    let mut axis = vec![' '; grid_w];
    for hour in (0..24).step_by(3) {
        let col = hour * grid_w / 24;
        for (i, ch) in format!("{hour:02}").chars().enumerate() {
            if col + i < grid_w {
                axis[col + i] = ch;
            }
        }
    }
    lines.push(Line::from(vec![
        Span::raw(" ".repeat(label_w + 1)),
        Span::styled(axis.iter().collect::<String>(), Style::new().fg(DIM)),
    ]));

    let day_start = date.and_hms_opt(0, 0, 0).unwrap();
    let from = match chrono::TimeZone::from_local_datetime(&Local, &day_start) {
        chrono::LocalResult::Single(dt) => dt,
        chrono::LocalResult::Ambiguous(a, _) => a,
        chrono::LocalResult::None => app.now,
    };
    let until = from + ChronoDuration::days(1);
    let now_col = if app.timeline.offset == 0 {
        let minutes = (app.now.hour() * 60 + app.now.minute()) as usize;
        Some(minutes * grid_w / 1440)
    } else {
        None
    };

    for (row_idx, &job_idx) in rows
        .iter()
        .enumerate()
        .skip(app.timeline.row_offset)
        .take(visible)
    {
        let job = &app.jobs[job_idx];
        let runs = schedule::occurrences_in_window(&job.triggers, from, until, 500);

        let mut cells = vec![Cell::Empty; grid_w];
        for hour in 0..24 {
            let col = hour * grid_w / 24;
            if col < grid_w {
                cells[col] = Cell::Grid;
            }
        }
        if let Some(col) = now_col
            && col < grid_w
        {
            cells[col] = Cell::Now;
        }
        for run in &runs {
            let minutes = (run.hour() * 60 + run.minute()) as usize;
            let col = (minutes * grid_w / 1440).min(grid_w - 1);
            cells[col] = match cells[col] {
                Cell::Marker(n) => Cell::Marker(n + 1),
                _ => Cell::Marker(1),
            };
        }

        let selected = row_idx == app.timeline.selected;
        let mut label = job.label.clone();
        if label.chars().count() > label_w {
            label = label.chars().take(label_w - 1).collect();
            label.push('…');
        }
        let label_style = if selected {
            Style::new()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if job.disabled {
            Style::new().fg(DIM)
        } else {
            Style::new().fg(kind_color(job.kind))
        };
        let pad = label_w.saturating_sub(label.chars().count());
        let mut spans = vec![
            Span::styled(label, label_style),
            Span::raw(" ".repeat(pad + 1)),
        ];
        spans.extend(cells_to_line(&cells, kind_color(job.kind)));
        lines.push(Line::from(spans));
    }

    if rows.is_empty() {
        lines.push(Line::from(Span::styled(
            "  no jobs with time-based schedules match the current filter",
            Style::new().fg(DIM),
        )));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn week_start(date: NaiveDate) -> NaiveDate {
    // Weeks start on Monday.
    let days_back = date.weekday().num_days_from_monday() as i64;
    date - ChronoDuration::days(days_back)
}

fn draw_timeline_week(frame: &mut Frame, app: &mut App, area: Rect) {
    let start = week_start(app.now.date_naive()) + ChronoDuration::days(7 * app.timeline.offset);
    let end = start + ChronoDuration::days(6);
    let title = format!(
        " timeline — week of {} – {}{} ",
        start.format("%b %d"),
        end.format("%b %d %Y"),
        if app.timeline.offset == 0 {
            " (this week)"
        } else {
            ""
        }
    );
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let (label_w, grid_w) = timeline_layout(area);
    let cell_w = (grid_w / 7).max(4);
    let rows = app.timeline_jobs();
    let visible = inner.height.saturating_sub(1) as usize;
    clamp_row_window(app, rows.len(), visible);

    // Day header.
    let today = app.now.date_naive();
    let mut header_spans = vec![Span::raw(" ".repeat(label_w + 1))];
    for d in 0..7 {
        let date = start + ChronoDuration::days(d);
        let text = format!("{:<cell_w$}", date.format("%a %d"));
        let style = if date == today {
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(DIM)
        };
        header_spans.push(Span::styled(text, style));
    }
    let mut lines = vec![Line::from(header_spans)];

    for (row_idx, &job_idx) in rows
        .iter()
        .enumerate()
        .skip(app.timeline.row_offset)
        .take(visible)
    {
        let job = &app.jobs[job_idx];
        let selected = row_idx == app.timeline.selected;

        let mut label = job.label.clone();
        if label.chars().count() > label_w {
            label = label.chars().take(label_w - 1).collect();
            label.push('…');
        }
        let label_style = if selected {
            Style::new()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if job.disabled {
            Style::new().fg(DIM)
        } else {
            Style::new().fg(kind_color(job.kind))
        };
        let pad = label_w.saturating_sub(label.chars().count());
        let mut spans = vec![
            Span::styled(label, label_style),
            Span::raw(" ".repeat(pad + 1)),
        ];

        for d in 0..7 {
            let date = start + ChronoDuration::days(d);
            let day_start = date.and_hms_opt(0, 0, 0).unwrap();
            let from = match chrono::TimeZone::from_local_datetime(&Local, &day_start) {
                chrono::LocalResult::Single(dt) => dt,
                chrono::LocalResult::Ambiguous(a, _) => a,
                chrono::LocalResult::None => continue,
            };
            let until = from + ChronoDuration::days(1);
            let count = schedule::occurrences_in_window(&job.triggers, from, until, 1000).len();
            let text = if count == 0 {
                format!("{:<cell_w$}", "·")
            } else if count == 1 {
                // Show the time for single daily runs.
                let run = schedule::occurrences_in_window(&job.triggers, from, until, 2)[0];
                format!("{:<cell_w$}", run.format("%H:%M"))
            } else {
                format!("{:<cell_w$}", format!("×{count}"))
            };
            let style = if count == 0 {
                Style::new().fg(Color::Rgb(60, 60, 60))
            } else if date == today {
                Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(kind_color(job.kind))
            };
            spans.push(Span::styled(text, style));
        }
        lines.push(Line::from(spans));
    }

    if rows.is_empty() {
        lines.push(Line::from(Span::styled(
            "  no jobs with time-based schedules match the current filter",
            Style::new().fg(DIM),
        )));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

// ---------------------------------------------------------------------------
// Upcoming tab
// ---------------------------------------------------------------------------

fn draw_upcoming_tab(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(format!(
        " upcoming runs — next 7 days ({}) ",
        app.agenda.len()
    ));

    if app.agenda.is_empty() {
        frame.render_widget(
            Paragraph::new("nothing scheduled in the next 7 days for the current filter")
                .style(Style::new().fg(DIM))
                .block(block),
            area,
        );
        return;
    }

    let label_w = 28usize;
    let mut items: Vec<ListItem> = Vec::new();
    let mut selected_row = 0usize;
    let mut last_date: Option<NaiveDate> = None;

    for (agenda_idx, (dt, job_idx)) in app.agenda.iter().enumerate() {
        let date = dt.date_naive();
        if last_date != Some(date) {
            let day_label = if date == app.now.date_naive() {
                format!("── today, {} ", date.format("%A %b %d"))
            } else {
                format!("── {} ", date.format("%A %b %d"))
            };
            items.push(ListItem::new(Line::from(Span::styled(
                format!(
                    "{day_label}{}",
                    "─".repeat(40_usize.saturating_sub(day_label.len()))
                ),
                Style::new().fg(DIM),
            ))));
            last_date = Some(date);
        }
        if agenda_idx == app.upcoming_selected {
            selected_row = items.len();
        }
        let job = &app.jobs[*job_idx];
        let mut label = job.label.clone();
        if label.chars().count() > label_w {
            label = label.chars().take(label_w - 1).collect();
            label.push('…');
        }
        let pad = label_w.saturating_sub(label.chars().count());
        let line = Line::from(vec![
            Span::styled(
                format!(" {}  ", dt.format("%H:%M")),
                Style::new().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:>9}  ", schedule::human_until(*dt, app.now)),
                Style::new().fg(DIM),
            ),
            Span::styled(label, Style::new().fg(kind_color(job.kind))),
            Span::raw(" ".repeat(pad + 2)),
            Span::styled(truncate(&job.command, 60), Style::new().fg(DIM)),
        ]);
        items.push(ListItem::new(line));
    }

    let list = List::new(items).block(block).highlight_style(
        Style::new()
            .bg(Color::Rgb(50, 50, 70))
            .add_modifier(Modifier::BOLD),
    );
    let mut state = ListState::default();
    state.select(Some(selected_row));
    frame.render_stateful_widget(list, area, &mut state);
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max - 1).collect();
        out.push('…');
        out
    }
}

// ---------------------------------------------------------------------------
// Popups
// ---------------------------------------------------------------------------

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width.saturating_sub(2));
    let h = height.min(area.height.saturating_sub(2));
    Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    }
}

fn draw_help(frame: &mut Frame) {
    let area = centered(frame.area(), 62, 24);
    frame.render_widget(Clear, area);
    let rows: Vec<(&str, &str)> = vec![
        ("1 / 2 / 3, tab", "switch view (jobs / timeline / upcoming)"),
        ("j k ↑ ↓", "move selection"),
        ("/", "filter jobs (esc clears)"),
        ("a", "show/hide Apple system jobs"),
        ("R", "refresh from disk / launchctl"),
        ("W", "show source warnings"),
        ("", ""),
        ("e", "edit job file (crontab or plist) in $EDITOR"),
        ("s", "reschedule (cron syntax, live preview)"),
        ("x", "enable / disable job"),
        ("r", "run job now"),
        ("D", "delete job (confirm)"),
        ("l", "view job stdout/stderr logs"),
        ("", ""),
        ("d / w", "timeline: day or week view"),
        ("← → / h l", "timeline: previous / next day or week"),
        ("t", "timeline: jump to today"),
        ("enter", "timeline/upcoming: open job in Jobs view"),
        ("", ""),
        ("q, ctrl-c", "quit"),
    ];
    let lines: Vec<Line> = rows
        .into_iter()
        .map(|(key, desc)| {
            Line::from(vec![
                Span::styled(format!("  {key:<14}"), Style::new().fg(Color::Cyan)),
                Span::raw(desc),
            ])
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" help — any key to close "),
        ),
        area,
    );
}

fn draw_confirm(frame: &mut Frame, message: &str) {
    let width = (message.len() as u16 + 6).clamp(30, frame.area().width.saturating_sub(4));
    let area = centered(frame.area(), width, 5);
    frame.render_widget(Clear, area);
    let lines = vec![
        Line::from(message.to_string()),
        Line::default(),
        Line::from(vec![
            Span::styled(
                "[y]",
                Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" yes   "),
            Span::styled(
                "[n]",
                Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" no"),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title(" confirm ")),
        area,
    );
}

fn draw_reschedule(frame: &mut Frame, app: &App, label: &str, buffer: &str, cursor: usize) {
    let area = centered(frame.area(), 64, 13);
    frame.render_widget(Clear, area);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("new schedule for ", Style::new().fg(DIM)),
        Span::styled(label.to_string(), Style::new().add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::from(Span::styled(
        "cron syntax: minute hour day month weekday",
        Style::new().fg(DIM),
    )));
    lines.push(Line::default());

    // Input line with a block cursor.
    let before: String = buffer.chars().take(cursor).collect();
    let at: String = buffer
        .chars()
        .nth(cursor)
        .map(|c| c.to_string())
        .unwrap_or(" ".to_string());
    let after: String = buffer.chars().skip(cursor + 1).collect();
    lines.push(Line::from(vec![
        Span::styled(" > ", Style::new().fg(Color::Cyan)),
        Span::raw(before),
        Span::styled(at, Style::new().add_modifier(Modifier::REVERSED)),
        Span::raw(after),
    ]));
    lines.push(Line::default());

    match CronExpr::parse(buffer) {
        Ok(expr) => {
            lines.push(Line::from(vec![
                Span::styled(" → ", Style::new().fg(Color::Green)),
                Span::styled(expr.describe(), Style::new().fg(Color::Green)),
            ]));
            let runs = schedule::next_runs(&[Trigger::Cron(expr)], app.now, 3, 400);
            for dt in runs {
                lines.push(Line::from(vec![
                    Span::raw("   "),
                    Span::raw(format!("{}", dt.format("%a %b %d  %H:%M"))),
                    Span::styled(
                        format!("  ({})", schedule::human_until(dt, app.now)),
                        Style::new().fg(DIM),
                    ),
                ]));
            }
        }
        Err(e) => {
            let msg = if buffer.trim().is_empty() {
                "e.g.  30 2 * * *   (02:30 daily)   or   0 9 * * mon-fri".to_string()
            } else {
                format!("✗ {e:#}")
            };
            let color = if buffer.trim().is_empty() {
                DIM
            } else {
                Color::Red
            };
            lines.push(Line::from(Span::styled(
                format!(" {msg}"),
                Style::new().fg(color),
            )));
        }
    }

    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        " enter apply   esc cancel",
        Style::new().fg(DIM),
    )));

    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title(" reschedule ")),
        area,
    );
}

fn draw_output(frame: &mut Frame, title: &str, lines: &[String], scroll: usize) {
    let full = frame.area();
    let area = centered(
        full,
        full.width.saturating_sub(8).min(110),
        full.height.saturating_sub(4),
    );
    frame.render_widget(Clear, area);
    let text: Vec<Line> = lines.iter().map(|l| Line::from(l.clone())).collect();
    frame.render_widget(
        Paragraph::new(text).scroll((scroll as u16, 0)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {title} — j/k scroll, esc close ")),
        ),
        area,
    );
}
