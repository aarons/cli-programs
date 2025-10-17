Please update the version in Cargo.toml, as well as CHANGELOG.md for your recent work.

Additional Context:

The changelog is for human consumption; it's to help them understand what changed at a high level. We don't need detailed blow-by-blow implementation details.

For example, this is not what we want as it's too low level:

```
### Added
- Branch detection functions:
  - `get_worktree_branches()` - Detects branches in use by worktrees
  - `get_main_branch()` - Auto-detects main vs master branch
  - `get_merged_local_branches()` - Finds local branches merged to main
  - `get_merged_remote_branches()` - Finds remote branches merged to origin/main
- Branch status functions:
  - `has_remote_branch()` - Check if remote tracking branch exists
  - `has_local_branch()` - Check if local branch exists
  - `is_remote_merged()` - Verify remote branch merge status
  - `get_branch_ahead_behind()` - Calculate commit differences between local/remote
```

Rather we want high level accomplishments and context, like this:

```
### Added
- Automatic detection and discovery of git branches
- Branch status evaluation to determine whether work is merged

```

Be sure to run `cargo test -p changelog-validator` once finished with your changes.
