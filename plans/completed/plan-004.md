# Plan: Handle Large Commits in GC

## Problem

When users submit very large commits (100k+ tokens), LLMs struggle to process all the text. Currently, the full diff is always sent, which can fail or produce poor results for large changes.

## Solution Overview

Detect when a commit diff exceeds a configurable token limit. When exceeded, switch to a "summary mode" that:
1. Prompts the user for context about the change (unless already provided via -c flag)
2. Sends only file list + user context (not full diff) to the LLM

## Implementation Details

### 1. Token Estimation

**Location**: New function in `gc/src/main.rs`

Add a simple token estimation function. A reasonable heuristic is ~4 characters per token for English text/code:

```rust
fn estimate_tokens(text: &str) -> usize {
    // Simple heuristic: ~4 chars per token for code/text
    text.len() / 4
}
```

### 2. Configuration Changes

**Location**: New config file `~/.config/cli-programs/gc.toml`

Create a gc-specific config file with token limit setting:

```toml
# ~/.config/cli-programs/gc.toml
max_diff_tokens = 30000
```

Add config loading in gc:
- New module `gc/src/config.rs` for gc-specific config
- Load from `~/.config/cli-programs/gc.toml`
- Default to 30000 if file doesn't exist

### 3. Large Diff Detection

**Location**: `gc/src/main.rs` in the main workflow (around lines 550-600)

After gathering the staged diff, check if it exceeds the limit:

```rust
let diff = get_staged_diff()?;
let estimated_tokens = estimate_tokens(&diff);

if estimated_tokens > config.max_diff_tokens {
    // Switch to summary mode
}
```

### 4. User Context Prompt

**Location**: New function in `gc/src/main.rs`

When diff is too large AND user hasn't provided context via `-c` flag or trailing args, prompt for context:

```rust
fn prompt_for_large_commit_context(file_count: usize, estimated_tokens: usize) -> Result<String> {
    eprintln!("Large commit detected ({} files, ~{} tokens)", file_count, estimated_tokens);
    eprintln!("Please provide a brief description of these changes:");

    // Read from stdin
    let mut context = String::new();
    std::io::stdin().read_line(&mut context)?;
    Ok(context.trim().to_string())
}
```

**Important**: Skip this prompt if user already provided context via `-c/--context` flag or trailing arguments.

### 5. Modified Prompt for Summary Mode

**Location**: New function in `gc/src/prompts.rs`

Create an alternative prompt that works with file list + context instead of full diff:

```rust
pub fn generate_commit_prompt_summary_mode(
    user_description: &str,
    user_context: &str,  // From interactive prompt
    branch: &str,
    branch_commits: &str,
    file_status: &str,   // The A/M/D file list
) -> String {
    // Similar to existing prompt but explains we're in summary mode
    // and to rely on file list + user context
}
```

### 6. Workflow Changes

**Location**: `gc/src/main.rs` main function

Modify the workflow to branch based on diff size:

```
[Gather diff]
    ↓
[Estimate tokens]
    ↓
[If tokens > limit]
  ├─ Show warning message
  ├─ If no user context provided (-c or trailing args):
  │     └─ Prompt user for context
  ├─ Use summary mode prompt with file list only
  └─ Generate commit message
[Else]
  └─ Use existing full-diff workflow
```

## Files to Modify

1. **gc/src/main.rs**
   - Add `estimate_tokens()` function
   - Add `prompt_for_large_commit_context()` function
   - Modify main workflow to check diff size
   - Handle summary mode branch

2. **gc/src/prompts.rs**
   - Add `generate_commit_prompt_summary_mode()` function

3. **gc/src/config.rs** (new file)
   - Add `GcConfig` struct with `max_diff_tokens: usize`
   - Add `load()` function to read from `~/.config/cli-programs/gc.toml`
   - Default to 30000 if config file doesn't exist

4. **gc/Cargo.toml**
   - Add `toml = { workspace = true }` and `serde = { workspace = true }` (already available in workspace)

## Complexity Assessment

**Low-Medium Complexity**

- Core logic is straightforward (estimate, compare, branch)
- Main work is in the new summary prompt and user interaction
- Config file is simple (single field)
- No architectural changes needed

## Implementation Order

1. Add gc config module with `max_diff_tokens` setting
2. Add `estimate_tokens()` function
3. Add `prompt_for_large_commit_context()` function
4. Add `generate_commit_prompt_summary_mode()` in prompts.rs
5. Integrate into main workflow with branching logic
6. Test with artificially low token limit
