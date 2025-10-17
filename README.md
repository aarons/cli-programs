# CLI Programs

Collection of command-line utilities written in Rust for Unix environments (macOS and Linux).

## Tools

- **gc** - Automated git commit with AI-generated conventional commit messages

## Installation

### Automated Installation (Recommended)

Use the Rust installer to build and install all tools to `~/code/bin`:

```bash
cargo run -p install --release
```

Or install to a custom location:

```bash
cargo run -p install --release -- --target /usr/local/bin
```

Make sure the target directory is in your PATH:

```bash
export PATH="$HOME/code/bin:$PATH"
```

### Manual Installation

Install individual tools using cargo:

```bash
cargo install --path gc
```

## Development

Build all tools:
```bash
cargo build --release
```

Build a specific tool:
```bash
cargo build -p gc --release
```

Run tests:
```bash
cargo test
```

Run a specific tool during development:
```bash
cargo run -p gc -- <args>
```

## Requirements

- Rust 1.70 or later
- Unix-like environment (macOS, Linux)
