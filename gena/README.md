# gena

Convert EPUB files to audio using text-to-speech.

## Installation

```bash
cargo install --path .
```

Or use the workspace installer:

```bash
cargo run -p update-cli-programs --release
```

## Usage

```bash
# Convert an EPUB to audio (outputs <book-name>.m4a)
gena book.epub

# Specify output file
gena book.epub -o audiobook.m4a

# Use a specific voice
gena book.epub -v "Samantha"

# Adjust speaking rate (words per minute)
gena book.epub -r 200

# List available voices
gena --list-voices
```

## Configuration

Configuration is stored at `~/.config/cli-programs/gena.toml`.

```bash
# Show current configuration
gena config show

# Set default voice
gena config set-voice "Alex"

# Set default backend
gena config set-backend macos-say
```

### Configuration Options

| Option | Description | Default |
|--------|-------------|---------|
| `backend` | TTS backend to use | `macos-say` |
| `voice` | Default voice | System default |
| `rate` | Speaking rate (WPM) | 175 |

## TTS Backends

### macOS Say (default)

Uses the built-in macOS `say` command. Available on all macOS systems.

```bash
gena book.epub --backend macos-say
```

### Future Backends

- ElevenLabs API
- OpenAI TTS
- Local models (Piper, etc.)

### Adding New Backends

The `TtsBackend` trait makes it easy to add new backends:

1. Create a new file like `src/tts/elevenlabs.rs`
2. Implement the `TtsBackend` trait
3. Add it to the factory in `src/tts/mod.rs`

## How It Works

1. Parses the EPUB file and extracts chapter content
2. Converts HTML to plain text
3. Generates audio for each chapter
4. Combines chapters into a single audio file
5. Outputs as M4A format

## Requirements

- macOS (for `say` command and `afconvert`)
- Rust toolchain (for building)
