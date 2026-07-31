# sched

A terminal UI for browsing and managing every scheduled job on your Mac in one place.

macOS scatters scheduled work across several systems with very different UX: your crontab, system crontabs, user LaunchAgents, global LaunchAgents and LaunchDaemons, plus hundreds of Apple's own launchd jobs. `sched` discovers all of them, shows their schedules on a visual timeline, and lets you edit, reschedule, enable/disable, run, and delete jobs without memorizing `launchctl` incantations.

```
┌ timeline — Friday Jul 31 (today) ──────────────────────────────────────────────┐
│                     00     03     06     09     12     15     18     21        │
│ com.example.poller  ●  ●  ●  ●  ●  ●  ●  ●  ●  ●  ●│ ●  ●  ●  ●  ●  ●  ●  ●  │
│ backup.sh           ·      · ●   ·      ·      ·    │      ·      ·      ·     │
│ rotate-logs.sh      ●      ·     ·      ·      ·    │      ·      ·      ·     │
│ sync-docs.sh        ·      ·     ·      ·  ●●●●●●●●●│●●●●●●●  ·   ·      ·     │
└────────────────────────────────────────────────────────────────────────────────┘
```

## Features

- **Unified job list** across user crontab, `/etc/crontab`, `/etc/cron.d`, user/global LaunchAgents, and LaunchDaemons — with Apple's `/System` jobs one keypress away
- **Visual timeline** of the day or week, with a now-line and per-source colors
- **Upcoming agenda**: every run in the next 7 days, chronologically, with countdowns
- **Live status** from `launchctl`: loaded, running (pid), last exit code, disabled
- **Edit** the underlying crontab or plist in `$EDITOR`, with validation and an offer to reload the job into launchd afterwards
- **Reschedule** any job with cron syntax and a live "next runs" preview — plists get their `StartCalendarInterval` rewritten for you
- **Enable/disable** without losing the job: cron lines get a `# [sched:off]` marker, launchd jobs get `launchctl disable` + `bootout`
- **Run now** (`launchctl kickstart`, or `/bin/sh -c` for cron entries with output capture)
- **Delete** with confirmation: cron lines are removed, plists are unloaded and moved to the Trash

## Usage

```bash
# Launch the TUI
sched

# Non-interactive: list every job with its schedule and next run
sched list
sched list --json          # machine-readable, includes next_runs
sched list --all           # include Apple /System jobs

# Non-interactive: what will run in the next 24 hours?
sched next
sched next --hours 72
```

### Keys

| Key | Action |
|-----|--------|
| `1` `2` `3` / `tab` | switch view: Jobs, Timeline, Upcoming |
| `j` `k` `↑` `↓` | move selection |
| `/` | filter jobs by label/command (esc clears) |
| `a` | show/hide Apple system jobs |
| `e` | edit the job's file (crontab or plist) in `$EDITOR` |
| `s` | reschedule with cron syntax, live preview |
| `x` | enable / disable |
| `r` | run now |
| `D` | delete (with confirmation) |
| `l` | view the job's stdout/stderr logs |
| `R` | refresh from disk and launchctl |
| `W` | show source parse warnings |
| `d` / `w` | timeline: day or week view |
| `←` `→` / `t` | timeline: previous/next day or week, today |
| `enter` | timeline/upcoming: open the job in the Jobs view |
| `?` | help |
| `q` | quit |

## Job sources

| Tag | Source | Managed? |
|-----|--------|----------|
| `crontab` | user crontab (`crontab -l`) | edit, reschedule, toggle, run, delete |
| `sys cron` | `/etc/crontab` | view only (root-owned) |
| `cron.d` | `/etc/cron.d/*` | view only (root-owned) |
| `agent` | `~/Library/LaunchAgents` | full management |
| `agent*` | `/Library/LaunchAgents` | full management (may need write access) |
| `daemon` | `/Library/LaunchDaemons` | management needs root for launchctl calls |
| `apple` / `apple-d` | `/System/Library/LaunchAgents` + `LaunchDaemons` | view only (SIP-protected), hidden until `a` |

## How changes are made

- **Disabling a cron job** prefixes the line with `# [sched:off] ` and reinstalls the crontab via `crontab <file>`; enabling strips the marker. Your comments, environment variables, and formatting are otherwise untouched.
- **Disabling a launchd job** runs `launchctl disable gui/<uid>/<label>` (persists across logins) followed by `launchctl bootout`. Enabling reverses both.
- **Rescheduling a launchd job** converts your cron expression into `StartCalendarInterval` entries (removing any `StartInterval`) and rewrites the plist as XML, leaving every other key alone. Cron's day-of-month/day-of-week OR rule is preserved by emitting one entry group per day field. Expressions that would expand to more than 100 calendar entries are rejected — edit the plist directly for those.
- **Editing** happens on the real file (plists) or a temp copy that is syntax-checked before being handed to `crontab` (crontab). Plist edits are validated with the plist parser and `plutil -lint` when available.
- **Run now** uses `launchctl kickstart -p` for launchd jobs. Cron entries run via `/bin/sh -c` with the crontab's environment variables, in the background, with output shown when they finish.

Before acting on a cron line, `sched` re-reads the crontab and verifies the line still matches what it discovered, so a crontab edited elsewhere is never blindly rewritten.

## Caveats

- `StartInterval` jobs ("every N seconds") are phase-locked to when launchd loaded them, which isn't knowable from the outside — the timeline projects them from the window start as an approximation.
- Event-driven triggers (`WatchPaths`, `QueueDirectories`, sockets, mach services) are shown and labeled, but have no timeline occurrences.
- LaunchDaemons live in the `system` launchd domain; enabling/disabling/kickstarting them requires root, so those actions will report a permission error when run as a normal user. The plist file itself can still be edited if you have write access.
- During DST transitions, runs at nonexistent wall-clock times are skipped and ambiguous times use the earlier offset.

## Development

The code builds and tests on Linux too (handy for CI): all macOS specifics are runtime calls that degrade gracefully. Hidden flags let you point it at fixtures:

```bash
sched list --crontab-file fixtures/crontab --launchd-dir fixtures/LaunchAgents --no-status
```

```bash
cargo test -p sched
```
