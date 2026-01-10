# Changelog

## [0.1.0] - 2026-01-09

### Added
- Initial release of zoom-remove
- Automatic detection and removal of Zoom updater LaunchAgents (`us.zoom.updater*.plist`)
- `launchctl bootout` integration to properly stop services before removal
- `install` subcommand to set up daily scheduled cleanup at 10:00 AM
- `uninstall` subcommand to remove the daily scheduler
- `status` subcommand to show current Zoom agents and scheduler status
