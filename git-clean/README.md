# git-clean - Git Branch Cleanup Tool

Clean up merged local and remote git branches automatically.

## Overview

`git-clean` automates clean up git branches that have been merged into the main branch.

It safely identifies and deletes both local and remote branches while protecting important branches and providing interactive prompts when branches are out of sync.

## Features

- **Automatic Detection**: Identifies branches merged into main (or master)
- **Safe Deletion**: Verifies merge status before deleting
- **Smart Protection**: Excludes current branch, main/master/develop, and worktree branches
- **Interactive Mode**: Prompts for confirmation when local/remote branches are out of sync
- **Dual Cleanup**: Handles both local and remote branches in a single run

## How It Works

1. **Fetch & Prune**: Updates remote references and prunes stale remote branches
2. **Detect Main Branch**: Automatically detects whether you use `main` or `master`
3. **Find Merged Branches**: Identifies local and remote branches fully merged into main
4. **Protected Exclusions**:
   - Current branch (marked with `*`)
   - Protected branches: `main`, `master`, `develop`
   - Branches used by worktrees
5. **Smart Deletion**:
   - Auto-delete when both local and remote are merged
   - Auto-delete local-only branches that are merged
   - Prompt user when branches are out of sync (ahead/behind)
6. **Cleanup Remote-Only**: Removes merged remote branches without local counterparts

## Usage

```bash
# Run from any git repository
git-clean

# The tool will:
# 1. Fetch and prune remote references
# 2. Evaluate all branches for merge status
# 3. Auto-delete safe branches
# 4. Prompt for out-of-sync branches
# 5. Clean up remote-only branches
```

## Interactive Prompts

When a local branch is merged but the remote is not (or vice versa), you'll be prompted with options:

- **push** - Push local changes to remote, then re-evaluate merge status
- **skip** - Keep the branch (skip deletion)
- **local** - Delete only the local branch
- **both** - Force delete both local and remote branches

The prompt shows:
- Local merge status
- Remote merge status
- Ahead/behind commit counts

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

## Protected Branches

The following branches are always protected from deletion:

- `main`
- `master`
- `develop`
- Current branch (wherever you currently are)
- Any branch checked out in a worktree

## Safety

- Uses `git branch -d` for safe deletion (requires branch to be fully merged)
- Only uses force delete (`-D`) when user explicitly chooses "both" option
- Remote deletion uses `git push --delete` (non-destructive, can be recovered)
- All operations respect git's merge status checking
