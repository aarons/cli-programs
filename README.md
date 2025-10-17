# CLI Programs

Collection of command-line utilities written in Rust for Unix environments (macOS and Linux).

## Tools

- **gc** - Automated git commit with AI-generated conventional commit messages

## Installation

Install individual tools using cargo:

```bash
cargo install --path gc
```

Or install all tools:

```bash
cargo install --path gc
# Add more as they're created
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
