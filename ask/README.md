# ask

An AI assistant that provides command line help and answers general questions. Supports multiple LLM providers via the shared `llm-client` library.

`ask` has two modes:
- command mode (default) which returns shell commands that are copied to the clipboard for easy pasting
- general mode (`-g`) for answering general questions or analyzing text

## Usage

### Basic Shell Commands

```bash
ask list all unique file extensions in folders recursively
# Response: find . -type f | sed 's/.*\.//' | sort -u

ask find files modified today
# Response: find . -type f -newermt "$(date +%Y-%m-%d)" 2>/dev/null

# Interactive mode (prompts for question)
ask
```

### General Questions

```bash
# Ask general questions (detailed responses, not copied to clipboard)
ask -g explain how rust ownership works

# Or use the long form
ask --general what is the difference between tcp and udp
```

### Using Piped Input for Context

Note that `-g` is also used for these commands so that we get back a summary or explanation instead of a bash command.

```bash
# Analyze error logs
cat error.log | ask -g what is causing this error

# Summarize code changes
git diff | ask -g summarize these changes
```

### Model Selection

Use the `--model` flag to specify a preset:

```bash
# Use a specific preset
ask --model claude-api how to compress a folder

# Use with general mode
ask -g --model openrouter-sonnet explain async/await in rust
```

## Configuration

Configuration is shared with `gc` and stored at `~/.config/cli-programs/llm.toml`.

### Managing Presets

```bash
# List available presets
ask config list

# Show current configuration
ask config show

# Set default preset
ask config set-default claude-api

# Add a new preset
ask config add-preset my-preset --provider anthropic --model claude-sonnet-4-20250514
```

### Supported Providers

- `claude-cli` - Claude Code CLI (default, no API key needed)
- `anthropic` - Anthropic API directly (requires `ANTHROPIC_API_KEY`)
- `openrouter` - OpenRouter API (requires `OPENROUTER_API_KEY`)
- `cerebras` - Cerebras API (requires `CEREBRAS_API_KEY`)

### Example Configuration

```toml
default_preset = "claude-cli"

[presets.claude-cli]
provider = "claude-cli"
model = "sonnet"

[presets.claude-api]
provider = "anthropic"
model = "claude-sonnet-4-20250514"

[presets.openrouter-sonnet]
provider = "openrouter"
model = "anthropic/claude-sonnet-4"
```

## How It Works

### Shell Command Mode (Default)

When you run `ask` without flags, it's optimized for getting shell commands:

- Includes a system prompt that instructs the LLM to return only valid shell commands without any markdown formatting (no triple backticks)
- The response is automatically copied to your clipboard using `pbcopy` (macOS)
- Perfect for quick command lookups that you can immediately paste and execute

Example: `ask how to find all pdf files` returns `find . -name "*.pdf"` (copied to clipboard)

### General Question Mode (`-g` or `--general`)

When you use the `-g` or `--general` flag:

- Removes the shell-specific prompt constraints
- Allows the LLM to provide detailed explanations, answer general knowledge questions, and format responses naturally
- Responses are NOT copied to clipboard
- Ideal for understanding concepts, analyzing piped input, or getting explanations

Example: `ask -g explain how rust ownership works` returns a detailed explanation

### Piped Input

When you pipe data to `ask`, it's automatically included as context for your question:

- Works in both shell and general modes
- Particularly useful with `-g` for analyzing logs, code diffs, or other text
- Example: `git diff | ask -g summarize these changes`

## Command Line Options

- `-g`, `--general`: Enable general question mode (see "How It Works" above)
- `-m`, `--model <PRESET>`: Use a specific model preset
- `-d`, `--debug`: Enable debug output (shows provider, token usage)
- `<QUESTION>...`: Your question (if omitted, will prompt interactively)

### Config Subcommands

- `ask config list`: List available presets
- `ask config show`: Show current configuration
- `ask config set-default <PRESET>`: Set the default preset
- `ask config add-preset <NAME> --provider <P> --model <M>`: Add a new preset

## Notes

- The clipboard functionality uses `pbcopy` and is macOS-specific
- Empty responses or errors will result in a non-zero exit code
- You can provide questions as arguments or pipe them via stdin
- If no question is provided and stdin is a terminal, you'll be prompted interactively

## Installation

### Prerequisites

For the `claude-cli` provider, Claude Code CLI must be installed and available in your PATH.

For API providers, set the appropriate environment variable:
- `ANTHROPIC_API_KEY` for Anthropic
- `OPENROUTER_API_KEY` for OpenRouter
- `CEREBRAS_API_KEY` for Cerebras

### Install

Install using the workspace installer:

```bash
cargo run -p update-cli-programs --release
```

Or build and install manually:

```bash
cargo build -p ask --release
cp target/release/ask ~/.local/bin/
```

## Development

```bash
# Build
cargo build -p ask

# Run tests
cargo test -p ask

# Run directly
cargo run -p ask -- how to list files

# Run with debug output
cargo run -p ask -- --debug how to list files
```
