# gc - Automated Git Commit Messages

Uses AI to generate conventional commit messages. Supports multiple LLM providers including Claude CLI, Anthropic API, OpenRouter, and Cerebras. Includes quality checks to ensure commits are clear, concise, and relevant.

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
- `--model <preset>` - Use a specific model preset instead of the default
- `--context <text>` - Provide additional context to guide commit message generation
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

### Providing additional context
```bash
gc --context "This refactor improves performance by caching database queries"
```
Adds extra context to help guide the LLM when generating the commit message.

### Using a specific model
```bash
gc --model cerebras "add new feature"
```
Overrides the default model preset for this commit.

## Configuration

Configuration is stored at `~/.config/cli-programs/llm.toml`. Use `gc config` subcommands to manage LLM providers and presets.

### List available presets
```bash
gc config list
```
Shows all configured model presets and which is the default.

### Show full configuration
```bash
gc config show
```
Displays the complete configuration file.

### Set default preset
```bash
gc config set-default cerebras
```
Changes which preset is used when `--model` is not specified.

### Add a new preset
```bash
gc config add-preset my-preset -p openrouter -M anthropic/claude-3.5-sonnet
```
Creates a new preset with the specified provider and model.

**Available providers:**
- `claude-cli` - Uses local Claude CLI (no API key required)
- `anthropic` - Anthropic API (requires `ANTHROPIC_API_KEY`)
- `openrouter` - OpenRouter API (requires `OPENROUTER_API_KEY`)
- `cerebras` - Cerebras API (requires `CEREBRAS_API_KEY`)

## Architecture

**Entry Point:** `src/main.rs`
**Prompts Module:** `src/prompts.rs`
**LLM Client:** `../llm-client/` (shared crate for multi-provider support)

### Core Flow

1. **Prerequisites Check** - Validates environment (git repo, LLM provider availability)
2. **Change Detection** - Checks for staged/unstaged changes based on `--staged` flag
3. **Context Gathering** - Collects git diff, file status, branch info, commit history
4. **LLM Generation** - Uses configured provider to generate conventional commit message
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

**LLM Integration** (`src/llm.rs`, `../llm-client/`)
- Uses `llm-client` crate for multi-provider support
- Providers: Claude CLI, Anthropic API, OpenRouter, Cerebras
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
