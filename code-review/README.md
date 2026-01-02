# code-review - LLM Code Reviews via Codex

Wraps `codex review` to get AI-powered code reviews, automatically parsing the output to return only the relevant review section.

## Usage

```bash
# Auto-detect mode based on uncommitted changes
code-review

# Force review of uncommitted changes only
code-review --uncommitted

# Review a specific commit
code-review --commit abc123
```

## CLI Flags

- `--uncommitted` - Review only uncommitted changes (staged, unstaged, untracked)
- `--commit <SHA>` - Review a specific commit
- `--help` - Show help information
- `--version` - Show version information

## Behavior

The tool auto-detects the appropriate review mode based on git state:

| Git State | Command |
|-----------|---------|
| Has uncommitted changes | `codex review --uncommitted` |
| No uncommitted changes | `codex review --base main` |
| `--commit` flag provided | `codex review --commit <SHA>` |

## Output Parsing

The tool parses codex output to extract just the review section, removing the metadata, thinking steps, and token usage information. Only the actual code review content is returned.

If parsing fails, the full codex output is logged to `./logs/codex_output_<timestamp>.log` for inspection.

## Build

```bash
cargo build -p code-review --release
```

## Testing

```bash
cargo test -p code-review
```
