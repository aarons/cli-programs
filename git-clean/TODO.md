# TODO: Fix remote branch cleanup

## Issue
Remote branches that are fully merged to origin/main are not being deleted.

## Root Cause
The current implementation incorrectly skips deleting remote branches if a local branch with the same name exists. This cross-consideration between local and remote state is wrong.

**Key principle:** Local and remote branches should be evaluated independently against their respective main branches.

## Why the Current Logic is Wrong

Remote branches merged to `origin/main` should be deleted regardless of local branch state:
- If `origin/feature` is merged to `origin/main`, the remote work is done
- The existence or merge state of local `feature` is irrelevant to remote cleanup
- Only the remote's source of truth (origin/main) should determine remote cleanup

## Implementation Fix

**Remove the `has_local_branch()` check from remote cleanup entirely.**

```rust
// Remote cleanup - evaluate against origin/main, no local consideration
let output = git(&["branch", "-r", "--merged", &format!("origin/{}", main_branch)])?;
for remote_branch in parse_branches(&output) {
    if remote_branch == format!("origin/{}", main_branch) ||
       remote_branch.ends_with("/HEAD") {
        continue;
    }

    let branch = remote_branch.strip_prefix("origin/").unwrap();
    git(&["push", "origin", "--delete", branch])?;
}

// Local cleanup - evaluate against local main, no remote consideration
let output = git(&["branch", "--merged", main_branch])?;
for local_branch in parse_branches(&output) {
    if local_branch == main_branch || is_current_branch(&local_branch) {
        continue;
    }

    git(&["branch", "-d", local_branch])?;
}
```

**Important:** Keep using `origin/main` as the reference for remote branches (not local main). Using local main would be dangerous - we could delete remote branches that are only merged locally but not yet pushed.
