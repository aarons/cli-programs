# add-reminders - Text-to-Reminders CLI

Process text input and automatically add reminders to macOS Reminders app. Supports batch processing with intelligent text cleaning.

## Example

```bash
$ add-reminders -t "- [ ] practice stepping back
	- [ ] stand up and stretch when needed
do another load of laundry"
✓ Added: practice stepping back
✓ Added: stand up and stretch when needed
✓ Added: do another load of laundry

Successfully added 3 reminder(s) to 'inbox'
```

## CLI Flags

- `-t, --todos <TEXT>` - (Required) The text containing todos to add, one per line
- `-l, --list <NAME>` - The Reminders list to add to (default: "inbox")

## Usage

### Basic usage with default inbox list
```bash
add-reminders -t "do another load of laundry"
```

### Specify a different list
```bash
add-reminders -t "buy groceries
pick up dry cleaning" -l "errands"
```

### Process markdown todo format
```bash
add-reminders -t "- [ ] first task
- [x] second task
- [ ] third task" -l "work"
```

## Text Processing Rules

The tool processes input text according to these rules:

1. **One todo per line** - Each newline creates a separate reminder
2. **Remove indentation** - Leading spaces and tabs are stripped from all todos
3. **Remove markdown todo markers** - Patterns like `- [ ]` and `- [x]` are removed
4. **Skip empty lines** - Blank lines are ignored

### Processing Example

**Input:**
```
- [ ] practice stepping back to problem solve when overwhelmed
	- [ ] stand up and stretch when needed
	- [ ] lean into using llms for support
do another load of laundry
change the sheets
```

**Output (5 reminders):**
1. practice stepping back to problem solve when overwhelmed
2. stand up and stretch when needed
3. lean into using llms for support
4. do another load of laundry
5. change the sheets

## Architecture

**Entry Point:** `src/main.rs`

### Core Flow

1. **CLI Parsing** - Parse arguments using clap (list name and todos text)
2. **Text Processing** - Process input line by line:
   - Trim leading/trailing whitespace
   - Remove markdown todo markers with regex
   - Filter out empty lines
3. **Reminder Creation** - For each processed todo:
   - Generate AppleScript command
   - Execute via `osascript` command
   - Handle errors and provide feedback

### Key Components

**Text Processing** (`process_line()`, `process_todos()`)
- Regex-based markdown todo marker removal: `^-\s*\[[^\]]*\]\s*`
- Whitespace normalization
- Empty line filtering

**macOS Integration** (`add_reminder()`)
- Uses AppleScript via `osascript` command
- Escapes double quotes in reminder text
- Provides detailed error messages on failure

**Error Handling**
- Uses `anyhow::Result` for error propagation
- Context-aware error messages
- Validates that target list exists in Reminders

## Requirements

- macOS (uses macOS Reminders app via AppleScript)
- The target Reminders list must exist before running the command

## Build

```bash
# Build add-reminders specifically
cargo build -p add-reminders --release

# The binary will be at target/release/add-reminders
```

## Testing

```bash
# Run add-reminders tests
cargo test -p add-reminders

# Run with output for debugging
cargo test -p add-reminders -- --nocapture
```

## Installation

Use the automated installer to install to ~/.local/bin:

```bash
cargo run -p update-cli-programs --release
```
