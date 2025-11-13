# add-reminders - Text-to-Reminders CLI

Process text input and automatically add reminders to macOS Reminders app. Supports batch processing with intelligent text cleaning.

## Example

```bash
# Using the --todos flag
$ add-reminders -t "- [ ] practice stepping back
	- [ ] stand up and stretch when needed
do another load of laundry"
✓ Added: practice stepping back
✓ Added: stand up and stretch when needed
✓ Added: do another load of laundry

Successfully added 3 reminder(s) to 'inbox'

# Using stdin (pipe or redirect)
$ echo "- [ ] call dentist" | add-reminders
✓ Added: call dentist

Successfully added 1 reminder(s) to 'inbox'
```

## CLI Flags

- `-t, --todos <TEXT>` - The text containing todos to add, one per line. If not provided, reads from stdin.
- `-l, --list <NAME>` - The Reminders list to add to (default: "inbox")
- `-v, --verbose` - Show detailed processing information (input text, processed output, etc.)

## Logging

All runs are automatically logged to `logs/add-reminders.log` in the cli-programs project directory. The log file automatically truncates when it exceeds 1MB.

**Log Location:** `~/code/cli-programs/logs/add-reminders.log` (or wherever you cloned the repo). The exact path is displayed when using `-v` (verbose mode), or when no todos are found.

**What's Logged:**
- Input parameters and text received
- Each line processing step (what matched, what was skipped, and why)
- AppleScript commands executed
- Success/failure of each reminder creation
- Timestamps for all operations

**View the log:**
```bash
# The log path is shown when running with -v flag
add-reminders -t "test" -v

# Or directly view it from the project directory:
tail -f ~/code/cli-programs/logs/add-reminders.log

# Follow the log in real-time while running commands:
tail -f ~/code/cli-programs/logs/add-reminders.log &
add-reminders -t "test todo"
```

The log is especially useful for debugging when todos aren't being parsed as expected. The `/logs` directory is excluded from git via `.gitignore`.

## Usage

### Basic usage with default inbox list
```bash
# Using flag
add-reminders -t "do another load of laundry"

# Using stdin
echo "do another load of laundry" | add-reminders
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

### Using with macOS Automator (Quick Action)

To create a "Send to Reminders" service:

1. Open **Automator** and create a new **Quick Action**
2. Set "Workflow receives current" to **text** in **any application**
3. Add a **Run Shell Script** action with these settings:
   - Shell: `/bin/bash`
   - Pass input: **as stdin** (this is key!)
   - Script: `/Users/YOUR_USERNAME/.local/bin/add-reminders`

Now you can select text anywhere, right-click, and choose "Send to Reminders" from the Services menu.

**Alternative Automator setup** (if you want to specify a list):
```bash
/Users/YOUR_USERNAME/.local/bin/add-reminders --list "work"
```

### Debug with verbose output
```bash
add-reminders -t "- [ ] test todo
    - [ ] nested todo" -v
```

This will show:
- Raw input text received
- Each line before processing
- Each todo after processing (with markers removed)
- Confirmation of each reminder added

## Text Processing Rules

The tool processes input text according to these rules:

1. **One todo per line** - Each newline creates a separate reminder
2. **Remove invisible Unicode characters** - Strips invisible characters like zero-width spaces (U+200B) and object replacement characters (U+FFFC) that can appear when copying text from some applications
3. **Remove indentation** - Leading spaces and tabs are stripped from all todos
4. **Remove markdown todo markers** - Patterns like `- [ ]` and `- [x]` are removed
5. **Skip empty lines** - Blank lines are ignored

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

**Text Processing** (`strip_leading_junk()`, `process_line()`, `process_todos()`)
- Unicode cleanup: Strips invisible characters (zero-width spaces, object replacement characters, etc.) by finding the first alphanumeric character
- Regex-based markdown todo marker removal: Handles various formats including `- [ ]`, `- [x]`, `* [ ]`, numbered lists, and partial syntax
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
