# help-slots

A macOS helper tool for SlotsAndDaggers timing puzzles. Monitors the game window for active timing puzzles and automatically triggers the spacebar at the optimal moment.

## Features

- Automatic puzzle detection via screen capture and template matching
- Edge-detection preprocessing for robust matching despite animated backgrounds
- Toggle-based activation (press 'F' to enable/disable)
- Runs as an independent application without modifying game display

## Requirements

- macOS (uses Core Graphics APIs)
- Screen Recording permission (for window capture)
- Accessibility permission (for hotkey listening and key injection)

## Usage

```bash
# Run the helper
help-slots run

# Test screen capture
help-slots test-capture

# Test preprocessing pipeline
help-slots test-preprocess
```

## How It Works

1. **Disabled state**: Helper is idle, minimal resource usage
2. **Enabled state**: Captures game window at ~1Hz, looking for puzzle UI
3. **Active state**: When a puzzle is detected, captures at ~60Hz and triggers spacebar when timing conditions are met

## Configuration

The game window is detected by looking for a window titled "SlotsAndDaggers".

## macOS Permissions

On first run, you'll be prompted to grant:
- **Screen Recording** - Required to capture the game window
- **Accessibility** - Required to listen for hotkeys and inject spacebar

You can also add these manually in System Settings > Privacy & Security.
