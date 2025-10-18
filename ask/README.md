# ask

A simple CLI wrapper for Claude Code that provides command line assistance and general AI interaction.

## Overview

`ask` is a Rust-based tool that makes it easy to get help from Claude AI directly from your command line. It has two modes:

1. **Shell Command Mode (default)**: Ask for command suggestions and get executable commands without markup, automatically copied to your clipboard
2. **General Mode**: Ask general questions and get detailed responses

## Installation

Install using the workspace installer:

```bash
cargo run -p update-cli-programs --release
```

Or build and install manually:

```bash
cargo build -p ask --release
cp target/release/ask ~/.local/bin/
```

## Prerequisites

The Claude Code CLI must be installed and available at `/Users/aaron/.local/bin/claude` or in your PATH.

## Usage

### Basic Command Help

```bash
# Ask for a command suggestion
ask how to find all pdf files

# The command will be copied to your clipboard automatically
# Response: find . -name "*.pdf"
```

### With Piped Input

```bash
# Get help analyzing piped data
cat error.log | ask what is causing this error

# Combine with other commands
git status | ask create a commit message for these changes
```

### General Questions

```bash
# Ask general questions (won't copy to clipboard)
ask -g explain how rust ownership works

# Or use the long form
ask --general what is the difference between tcp and udp
```

### Advanced Options

```bash
# Specify output format (passed to Claude CLI)
ask --output-format json how to list files

# Interactive mode (prompts for question)
ask
```

## How It Works

1. **Shell Mode (default)**: Includes a system prompt that instructs Claude to return only valid shell commands without markup (no triple backticks). The response is automatically copied to your clipboard for easy pasting.

2. **General Mode** (`-g` or `--general`): Removes the shell-specific prompt, allowing for detailed explanations and general knowledge questions. Responses are not copied to clipboard.

3. **Piped Input**: When you pipe data to `ask`, it's used as context for the question, making it easy to analyze logs, code, or other text.

## Examples

```bash
# Get command to compress a directory
ask compress directory into tar.gz

# Analyze git diff
git diff | ask -g summarize these changes

# Debug an error message
ask -g explain this error: "segmentation fault"

# Find files modified today
ask find files modified today

# Get help with sed
ask replace all foo with bar in file.txt
```

## Command Line Options

- `-g`, `--general`: Enable general question mode (no shell command prompt, no clipboard)
- `--output-format <FORMAT>`: Specify output format to pass to Claude CLI
- `<QUESTION>...`: The question to ask (if not provided, will prompt interactively)

## Notes

- The clipboard functionality uses `pbcopy` and is macOS-specific
- Empty responses or CLI errors will result in a non-zero exit code
- You can provide questions as arguments or pipe them via stdin
- If no question is provided and stdin is a terminal, you'll be prompted interactively

## Development

```bash
# Build
cargo build -p ask

# Run tests
cargo test -p ask

# Run directly
cargo run -p ask -- how to list files
```
