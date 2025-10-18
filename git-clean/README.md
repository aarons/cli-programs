# git-clean

Automatically delete merged git branches, both locally and on remote.

## What It Does

Deletes stale branches that have been fully merged to the main branch (main/master).

This handles both local and remote branches.

## Usage

```bash
git-clean
```

The tool will:
1. Fetch and prune remote references
2. Delete local branches merged into main
3. Delete remote branches merged into origin/main

## Protected Branches

These branches are never deleted:
- `main`, `master`, `develop`
- Your current branch
- Branches used by worktrees

## Safety

Uses `git branch -d` for local deletion, which fails if the branch isn't fully merged.

Remote branches are filtered with `git branch -r --merged origin/main` before deletion.

## How Deletion Works

**Local branches:**
- Double safety—pre-filtered with `--merged`, then `git branch -d` performs its own merge check
- Process: `git branch --merged main` → `git branch -d <branch>`

**Remote branches:**
- Single safety—pre-filtered with `--merged`, but `git push origin --delete` has no built-in merge check
- Process: `git branch -r --merged origin/main` → `git push origin --delete <branch>`

Only branches that git confirms are fully merged will be deleted.

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
