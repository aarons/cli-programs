# Changelog

## [0.1.0] - 2025-10-20

### Added
- Initial release of git-merge (automated git branch merging tool)
- Simple merge mode (default) - standard git merge with automatic branch cleanup
- Squash merge mode with `--squash` flag for condensing commits
- Automatic feature branch detection (uses current branch by default)
- Branch argument support for merging specific branches
- `--main-branch` flag to specify target branch (defaults to 'main')
- Integration with `gc` for AI-generated commit messages during squash merges
- Automatic push of feature branch before merging
- Automatic main branch update (fetch + pull) before merge
- Clean working tree validation after pulling main
- Local branch cleanup after successful merge (safe delete for simple merge, force delete for squash)
- Comprehensive error handling with clear messages
- Merge conflict detection with helpful resolution instructions
- Graceful handling when `gc` is not available for squash merges
