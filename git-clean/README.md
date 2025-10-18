# git-clean - Git Branch Cleanup Tool

Clean up merged local and remote git branches automatically.

## Overview

`git-clean` automates the cleanup of git branches that have been merged into the main branch. It safely identifies and deletes both local and remote branches while protecting important branches.

## Features

- **Automatic Detection**: Identifies branches merged into main (or master)
- **Safe Deletion**: Uses git's safe delete (`-d`) for local branches (requires fully merged)
- **Smart Protection**: Excludes current branch, main/master/develop, and worktree branches
- **Dual Cleanup**: Handles both local and remote branches in a single run
- **Non-interactive**: Automatically deletes merged branches without prompts

## How It Works

1. **Fetch & Prune**: Updates remote references and prunes stale remote branches
2. **Detect Main Branch**: Automatically detects whether you use `main` or `master`
3. **Find Merged Local Branches**: Identifies local branches fully merged into local main
4. **Protected Exclusions**:
   - Current branch (marked with `*`)
   - Protected branches: `main`, `master`, `develop`
   - Branches used by worktrees
5. **Delete Local Branches**: Safely deletes merged local branches using `git branch -d`
6. **Delete Remote Branches**: Deletes remote branches merged into origin/main

## Usage

```bash
# Run from any git repository
git-clean

# The tool will:
# 1. Fetch and prune remote references
# 2. Evaluate all branches for merge status
# 3. Delete merged local branches
# 4. Delete merged remote branches
```

## Protected Branches

The following branches are always protected from deletion:

- `main`
- `master`
- `develop`
- Current branch (wherever you currently are)
- Any branch checked out in a worktree

## Safety

- Uses `git branch -d` for safe deletion of local branches (requires branch to be fully merged)
- Remote deletion uses `git push origin --delete` (non-destructive, can be recovered)
- All operations respect git's merge status checking
- Fetches and prunes before evaluation to ensure accurate remote state

## Building

```bash
# Build git-clean specifically
cargo build -p git-clean --release

# The binary will be at target/release/git-clean
```

## Testing

```bash
# Run git-clean tests
cargo test -p git-clean

# Run with output for debugging
cargo test -p git-clean -- --nocapture
```
