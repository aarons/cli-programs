# gc - AI-Powered Git Commit Tool

Automated git commit with AI-generated conventional commit messages using Claude CLI.

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

## CLI Flags

- `--debug` - Verbose output showing LLM interactions and validation steps
- `--staged` - Only commit staged changes (don't auto-stage)
- `--nopush` - Skip pushing to remote after commit
- `--context <text>` - Provide squash merge context (changes mode to use provided context instead of branch commits)
- Trailing args - High-level description to guide commit message generation

## Building

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
