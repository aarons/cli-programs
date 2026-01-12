# transcribe - Audio to Text Transcription

Transcribe audio files to text using whisper.cpp. Automatically handles audio format conversion for compatibility with whisper requirements.

## Example

```bash
$ transcribe recording.wav
And so my fellow Americans, ask not what your country can do for you,
ask what you can do for your country.
```

## Prerequisites

- **whisper.cpp** - Build and install from https://github.com/ggerganov/whisper.cpp
- **ffmpeg** - Required for audio format detection and conversion
- **Models** - Download at least one whisper model (ggml-medium.en.bin recommended)

## Installation

```bash
cargo run -p update-cli-programs --release
```

Or build directly:

```bash
cargo build -p transcribe --release
cp target/release/transcribe ~/.local/bin/
```

## Usage

### Basic transcription

```bash
transcribe audio.wav
```

Outputs the transcription text to stdout.

### Using a specific model

```bash
transcribe -m large-turbo audio.wav
```

Use the larger, more accurate model (requires ggml-large-v3-turbo.bin).

### Debug output

```bash
transcribe --debug audio.wav
```

Shows audio format info and conversion steps.

## CLI Flags

- `-m, --model <MODEL>` - Model to use: `medium` (default) or `large-turbo`
- `--debug` - Show debug output including audio format info
- `-h, --help` - Print help
- `-V, --version` - Print version

## Configuration

Configuration is stored at `~/.config/cli-programs/transcribe.toml`.

### Show configuration

```bash
transcribe config show
```

Displays current settings and validates that paths exist.

### Set configuration values

```bash
transcribe config set whisper_cli_path /path/to/whisper-cli
transcribe config set models_dir /path/to/models
transcribe config set default_model large-turbo
```

### Configuration options

| Key | Description | Default |
|-----|-------------|---------|
| `whisper_cli_path` | Path to whisper-cli binary | `~/code/whisper.cpp/build/bin/whisper-cli` |
| `models_dir` | Directory containing model files | `~/code/whisper.cpp/models` |
| `default_model` | Default model: `medium` or `large-turbo` | `medium` |

## Models

| Model | File | Size | Notes |
|-------|------|------|-------|
| medium | ggml-medium.en.bin | 1.4 GB | Fast, good accuracy, English only |
| large-turbo | ggml-large-v3-turbo.bin | 1.5 GB | Better accuracy, multilingual |

## Audio Format Handling

Whisper requires audio in a specific format (16kHz mono). This tool automatically:

1. Analyzes input audio with `ffprobe`
2. Detects if conversion is needed (wrong sample rate or stereo)
3. Converts to compatible format using `ffmpeg` if necessary
4. Cleans up temporary files after transcription

Supported input formats: WAV, MP3, OGG, FLAC, and any format ffmpeg can read.

## Architecture

**Entry Point:** `src/main.rs`
**Config Module:** `src/config.rs`
**Audio Module:** `src/audio.rs`

### Core Flow

1. Load configuration from `~/.config/cli-programs/transcribe.toml`
2. Validate input file exists
3. Check audio format with `ffprobe`
4. Convert audio if needed (sample rate != 16kHz or channels != 1)
5. Run whisper-cli with selected model
6. Parse and output transcription text
7. Clean up temporary files

## Build

```bash
cargo build -p transcribe --release
```

## Testing

```bash
cargo test -p transcribe
```
