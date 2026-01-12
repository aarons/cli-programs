# CLI Programs

Collection of command-line utilities written in Rust for Unix environments (macOS and Linux).

## Programs

- **[ask](ask/README.md)** - Helper for shell commands and general questions
- **[gc](gc/README.md)** - Automated git commit messages using Claude Code CLI
- **[git-clean](git-clean/README.md)** - Interactive tool to safely remove merged local and remote git branches
- **[transcribe](transcribe/README.md)** - Audio to text transcription using whisper.cpp
- **[update-cli-programs](update-cli-programs/README.md)** - Automated installer/updater for these CLI programs

### ask

`ask` is an AI helper for the command line that provides shell commands or answers to general questions. It uses Claude Code CLI by default, so no API credits are required if you have a subscription.

Example usage:

```bash
# Get a shell command
ask how to count all files in subdirectories
> find . -type f | wc -l

# Ask a general question
ask -g explain how rust ownership works
> detailed answer that could have been piped to a markdown file...

# Also works with piped input
git diff | ask -g summarize these changes
```

`ask` has two modes:
- default mode which responds with a shell command (and only a shell command). It also copies this to the clipboard for easy pasting
- general mode (`-g`) which is a general question, it will respond with an answer and not a shell command, and will not touch the clipboard.

---

### gc

Automatically generates conventional commit messages. Analyzes your git changes and creates properly formatted commit messages following the conventional commits specification.

Example usage:

```bash
# Generate and commit with AI message
gc

# Commit only staged changes
gc --staged

# Commit without pushing to remote
gc --nopush

# Optionally you can provide additional context for better messages
gc refactored authentication system
```

---

### git-clean

Simple tool to clean up git branches that have been merged into main. Safely identifies and deletes both local and remote branches while protecting important branches.

Example usage:

```bash
# Clean up merged branches
git-clean
```

---

### transcribe

Transcribe audio files to text using whisper.cpp. Automatically handles audio format conversion (sample rate, channels) for compatibility with whisper requirements.

Example usage:

```bash
# Basic transcription
transcribe recording.wav

# Use the larger, more accurate model
transcribe -m large-turbo recording.wav

# Debug mode shows audio format info
transcribe --debug recording.wav
```

Requires whisper.cpp and ffmpeg to be installed. Configure paths with `transcribe config set`.

---

## Installation

From the repository root:

```bash
# First run: install all programs to ~/.local/bin
cargo run -p update-cli-programs --release

# Future runs, can just do:
update-cli-programs

# Install programs to a custom bin folder
cargo run -p update-cli-programs --release -- --target /usr/local/bin
```


## Development Process

Let's say we're going to add a feature to the git-clean tool.

The general workflow is to:
- checkout a feature branch `git checkout -b new-feature`
- make changes to `git-clean/src/main.rs`
- run the tests (you added tests right?) `cargo test -p git-clean`
- try out the program to make sure it works correctly `cargo run -p git-clean`
- update the changelog and Cargo.toml version (can use the `/update-changelog` claude command)
- commit the changes using `gc` (might as well use the program in this repo right!)
- merge the changes to main if you are feeling good about it all `git checkout main; git merge new-feature`
- push the changes to remote `git push`
- install the updated program `update-cli-programs`

## License

This software is licensed under the **PolyForm Noncommercial License 1.0.0**.

- ✅ Free for personal, educational, and nonprofit use
- ✅ Attribution required
- ❌ Use within a business or commercial context requires a commercial license

For commercial licensing inquiries, please [create an issue](https://github.com/aarons/cli-programs/issues) with contact info.

See the [LICENSE](LICENSE) file for full terms.

## Requirements

- Rust 1.70 or later
- Unix-like environment (macOS, Linux)
