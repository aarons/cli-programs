# Changelog

## [Unreleased]

---

## [1.1.0] - 2025-10-17

### Changed
- Remote branches now evaluated against `origin/main` instead of local main for proper cleanup
- Local and remote branch cleanup now operate independently - each evaluated only against their respective main branch
- Simplified branch cleanup logic by removing interactive prompts and coordination between local/remote state

### Removed
- Interactive prompts for branch cleanup decisions
- Push and re-evaluate workflow for out-of-sync branches

---

## [1.0.0] - 2025-10-17

### Added
- Initial project scaffolding and documentation
- Core branch detection and status checking functionality
- Interactive user prompts for branch cleanup decisions with status display
- Complete branch cleanup implementation for merged local and remote branches
- Automatic deletion of branches merged into main/master
- Interactive prompts for handling branches with unpushed commits or sync conflicts
- Push and re-evaluate workflow for out-of-sync branches
- Protection for branches in use by git worktrees
