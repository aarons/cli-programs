# zoom-remove

Remove Zoom's unauthorized updater services from macOS LaunchAgents.

Zoom installs background updater services that run without explicit permission. This tool removes those services and can be scheduled to run daily to prevent them from returning.

## Installation

```bash
cargo install --path .
```

Or install all workspace tools:

```bash
cargo run -p update-cli-programs --release
```

## Usage

### Remove Zoom updaters now

```bash
zoom-remove
```

This scans `~/Library/LaunchAgents/` for any `us.zoom.updater*.plist` files, uses `launchctl bootout` to stop the services, and removes the plist files.

### Install daily scheduler

```bash
zoom-remove install
```

Installs a launchd agent that runs `zoom-remove` daily at 10:00 AM to automatically clean up any Zoom updaters that get reinstalled.

### Remove the scheduler

```bash
zoom-remove uninstall
```

### Check status

```bash
zoom-remove status
```

Shows currently installed Zoom updater agents and whether the daily scheduler is active.

## How it works

1. Scans `~/Library/LaunchAgents/` for files matching `us.zoom.updater*.plist`
2. For each found agent:
   - Runs `launchctl bootout gui/<uid>/<label>` to stop the service
   - Deletes the plist file
3. When scheduling is installed, creates a launchd plist at:
   `~/Library/LaunchAgents/com.cli-programs.zoom-remove.plist`

## Target files

Known Zoom updater agents:
- `us.zoom.updater.plist`
- `us.zoom.updater.login.check.plist`

Any file matching `us.zoom.updater*.plist` will be removed.
