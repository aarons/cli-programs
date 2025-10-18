# CLI Programs

Collection of command-line utilities written in Rust for Unix environments (macOS and Linux).

## Tools

- **gc** - Automated git commit messages using Claude CLI to generate conventional commit messages
- **git-clean** - Safely remove local and remote git branches that have been merged to main
- **update-cli-programs** - Automated installer/updater for all CLI programs

## Installation

From the repository root:

```bash
# Install/update to default location (~/.local/bin)
cargo run -p update-cli-programs --release

# Install/update to custom location
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
