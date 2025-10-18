# gc - Automated Git Commit Messages

Uses AI to generate conventional commit messages. This has several quality checks to ensure commits are clear, concise, and relevant.

## Example

When you run `gc`, it analyzes your changes and generates a conventional commit message:

```bash
$ gc "implement branch detection and status checking"
```

**Generated commit message:**
```
feat(git-clean): implement branch detection and status checking

Implements core git branch analysis functions including worktree detection,
main branch identification, merged branch discovery, and ahead/behind tracking.
```

The tool automatically stages changes, generates the commit message, commits, and pushes to remote (unless `--nopush` is specified).

## CLI Flags

- `--debug` - Verbose output showing LLM interactions and validation steps
- `--staged` - Only commit staged changes (don't auto-stage)
- `--nopush` - Skip pushing to remote after commit
- `--context <text>` - Provide squash merge context (changes mode to use provided context instead of branch commits)
- Trailing args - High-level description to guide commit message generation

## Usage

### Basic commit with description
```bash
gc "add user authentication feature"
```
Stages all changes, generates commit message, commits, and pushes.

### Commit only staged changes
```bash
git add src/auth.rs
gc --staged "add login functionality"
```
Only commits what's already staged, doesn't auto-stage other changes.

### Commit without pushing
```bash
gc --nopush "fix validation bug"
```
Generates and commits but skips the push to remote.

### Debug mode
```bash
gc --debug "refactor database layer"
```
Shows detailed output including LLM prompts, responses, and validation steps.

### Squash merge with context
```bash
gc --context "Merged PR #123: User authentication system"
```
Uses provided context instead of analyzing branch commits. Useful for squash merges.

## Architecture

**Entry Point:** `src/main.rs`
**Prompts Module:** `src/prompts.rs`

### Core Flow

1. **Prerequisites Check** - Validates environment (git repo, Claude CLI availability)
2. **Change Detection** - Checks for staged/unstaged changes based on `--staged` flag
3. **Context Gathering** - Collects git diff, file status, branch info, commit history
4. **LLM Generation** - Calls Claude CLI to generate conventional commit message
5. **Validation Loop** - Validates message format and content, retries if needed:
   - Format validation using `git-conventional` crate
   - Policy violation checks (URLs, emails, emojis)
   - Automatic cleaning attempts (max 3) if violations found
6. **Commit & Push** - Commits with generated message, optionally pushes to remote

### Key Components

**Git Operations** (`main.rs:69-199`)
- `git()` wrapper function handles all git command execution with error handling
- Functions for diff, status, branch detection, commit history extraction
- Special handling for main/master branch detection

**LLM Integration** (`main.rs:230-364`)
- Uses Claude CLI via subprocess (`which claude` to check availability)
- Structured XML output format with `<observations>` and `<commit_message>` tags
- Retry logic (MAX_RETRIES = 3) for generation failures
- Separate fix/clean prompts for format issues vs policy violations

**Validation System** (`main.rs:366-420`)
- **Format Validation**: Uses `git-conventional` crate to parse conventional commit structure
- **Policy Violations**: Detects and blocks:
  - Email addresses (using `email_address` crate)
  - URLs and domains (using `url` and `addr` crates)
  - Emojis (using `emojis` crate with `unicode-segmentation`)
  - Special logic to exclude actual repository filenames from URL detection
- Validation failures trigger automatic cleaning attempts with LLM

**Prompt Engineering** (`prompts.rs`)
- System prompt defines role as experienced engineer
- Main prompt includes detailed conventional commit specification
- Fix prompts for format and content issues
- Prompts emphasize brevity and clarity

## Build

```bash
# Build gc specifically
cargo build -p gc --release

# The binary will be at target/release/gc
```

## Testing

```bash
# Run gc tests
cargo test -p gc

# Run with output for debugging
cargo test -p gc -- --nocapture
```
