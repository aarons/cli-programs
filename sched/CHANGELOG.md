# Changelog

## [0.1.0] - 2026-07-31

### Added
- Initial release
- Unified discovery of scheduled jobs across user crontab, /etc/crontab, /etc/cron.d, LaunchAgents (user, global, Apple), and LaunchDaemons (global, Apple)
- Jobs view with filterable list, per-source colors, live launchctl status (loaded, pid, last exit), and a detail pane with human-readable schedules and next five runs
- Timeline view with visual day and week modes, per-job run markers, and a now-line
- Upcoming view listing every run in the next 7 days with countdowns
- Edit jobs in $EDITOR: crontab edits are syntax-checked before install, plist edits are validated and offered a launchd reload
- Reschedule with cron syntax and live next-run preview; launchd plists get StartCalendarInterval rewritten (cron day-of-month/day-of-week OR semantics preserved)
- Enable/disable: reversible `# [sched:off]` markers for cron lines, `launchctl disable` + `bootout` for launchd jobs
- Run now via `launchctl kickstart` or `/bin/sh -c` with captured output
- Delete with confirmation: cron lines removed, plists unloaded and moved to Trash
- Non-interactive `sched list` (with `--json`) and `sched next --hours N` subcommands
- Stale-line protection: cron mutations verify the crontab hasn't changed since discovery
