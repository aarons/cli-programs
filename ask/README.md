# ask

An AI that provides command line assistance and answers general questions. Currently utilize Claude Code (so no credits are needed if you have a subscription).

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

## How It Works

### Shell Command Mode (Default)

When you run `ask` without flags, it's optimized for getting shell commands:

- Includes a system prompt that instructs Claude to return only valid shell commands without any markdown formatting (no triple backticks)
- The response is automatically copied to your clipboard using `pbcopy` (macOS)
- Perfect for quick command lookups that you can immediately paste and execute

Example: `ask how to find all pdf files` returns `find . -name "*.pdf"` (copied to clipboard)

### General Question Mode (`-g` or `--general`)

When you use the `-g` or `--general` flag:

- Removes the shell-specific prompt constraints
- Allows Claude to provide detailed explanations, answer general knowledge questions, and format responses naturally
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
- `--output-format <FORMAT>`: Specify output format to pass to Claude CLI
- `<QUESTION>...`: Your question (if omitted, will prompt interactively)

## Notes

- The clipboard functionality uses `pbcopy` and is macOS-specific
- Empty responses or CLI errors will result in a non-zero exit code
- You can provide questions as arguments or pipe them via stdin
- If no question is provided and stdin is a terminal, you'll be prompted interactively

## Installation

### Prerequisites

The Claude Code CLI must be installed and available in your PATH.

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
```
