mod audio;
mod config;

use anyhow::{bail, Context, Result};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use config::Config;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser, Debug)]
#[command(name = "transcribe")]
#[command(about = "Transcribe audio files to text using whisper.cpp")]
#[command(version)]
struct Args {
    /// Audio file to transcribe
    file: Option<PathBuf>,

    /// Model to use for transcription
    #[arg(short, long, value_enum)]
    model: Option<Model>,

    /// Show debug output
    #[arg(long)]
    debug: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Clone, ValueEnum)]
enum Model {
    /// Medium English model (faster, good accuracy)
    Medium,
    /// Large v3 turbo model (slower, better accuracy)
    LargeTurbo,
}

impl Model {
    fn as_str(&self) -> &'static str {
        match self {
            Model::Medium => "medium",
            Model::LargeTurbo => "large-turbo",
        }
    }
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set a configuration value
    Set {
        /// Configuration key (whisper_cli_path, models_dir, default_model)
        key: String,
        /// Value to set
        value: String,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Handle config subcommands
    if let Some(Commands::Config { action }) = args.command {
        return handle_config_command(action);
    }

    // Show help if no file argument provided
    if args.file.is_none() {
        Args::command().print_long_help()?;
        return Ok(());
    }

    // Load config
    let config = Config::load().context("Failed to load configuration")?;

    // Get the input file (safe to unwrap since we checked above)
    let input_file = args.file.unwrap();

    if !input_file.exists() {
        bail!("Input file not found: {}", input_file.display());
    }

    // Determine which model to use
    let model_name = args
        .model
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| config.default_model.clone());

    let model_path = config.model_path(&model_name);

    // Validate paths
    if !PathBuf::from(&config.whisper_cli_path).exists() {
        bail!(
            "whisper-cli not found at: {}\nRun 'transcribe config set whisper_cli_path <path>' to configure",
            config.whisper_cli_path
        );
    }

    if !model_path.exists() {
        bail!(
            "Model file not found: {}\nRun 'transcribe config set models_dir <path>' to configure",
            model_path.display()
        );
    }

    // Check audio format
    let audio_info = audio::check_audio_format(&input_file)
        .context("Failed to analyze audio file")?;

    if args.debug {
        eprintln!(
            "Audio: {} Hz, {} channel(s), codec: {}",
            audio_info.sample_rate, audio_info.channels, audio_info.codec
        );
    }

    // Convert if needed
    let (transcription_file, _temp_file) = if audio_info.needs_conversion() {
        let issues = audio_info.issues().join(", ");
        eprintln!("Converting audio ({})...", issues);

        let temp = audio::convert_audio(&input_file).context("Failed to convert audio")?;
        let path = temp.path().to_path_buf();
        (path, Some(temp))
    } else {
        (input_file.clone(), None)
    };

    if args.debug {
        eprintln!("Using model: {}", model_path.display());
        eprintln!("Transcribing: {}", transcription_file.display());
    }

    // Run whisper-cli
    let output = Command::new(&config.whisper_cli_path)
        .args([
            "-f",
            transcription_file.to_str().context("Invalid file path")?,
            "-m",
            model_path.to_str().context("Invalid model path")?,
            "--no-timestamps",
            "-nt", // No timestamps in output
        ])
        .output()
        .context("Failed to run whisper-cli")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("whisper-cli failed: {}", stderr);
    }

    // Parse and print the transcription
    let stdout = String::from_utf8(output.stdout).context("Invalid UTF-8 in whisper output")?;

    // whisper-cli outputs some metadata lines before the transcription
    // The actual transcription starts after the model loading messages
    let transcription = extract_transcription(&stdout);
    print!("{}", transcription);

    Ok(())
}

/// Extract the transcription text from whisper-cli output
/// whisper-cli prints various status messages before the actual transcription
fn extract_transcription(output: &str) -> String {
    let mut lines: Vec<&str> = Vec::new();
    let mut in_transcription = false;

    for line in output.lines() {
        // Skip whisper status/info lines
        if line.starts_with("whisper_")
            || line.starts_with("main:")
            || line.starts_with("system_info:")
            || line.contains("model size")
            || line.contains("processing")
            || line.contains("audio ctx")
            || line.is_empty() && !in_transcription
        {
            continue;
        }

        in_transcription = true;
        // Trim leading/trailing whitespace from each line
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            lines.push(trimmed);
        }
    }

    let result = lines.join("\n");
    if result.is_empty() {
        result
    } else {
        format!("{}\n", result)
    }
}

fn handle_config_command(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Show => {
            let config = Config::load()?;
            let path = Config::config_path()?;

            println!("Config file: {}", path.display());
            println!();
            println!("whisper_cli_path = \"{}\"", config.whisper_cli_path);
            println!("models_dir = \"{}\"", config.models_dir);
            println!("default_model = \"{}\"", config.default_model);

            // Show status of paths
            println!();
            let cli_exists = PathBuf::from(&config.whisper_cli_path).exists();
            println!(
                "whisper-cli: {}",
                if cli_exists { "found" } else { "NOT FOUND" }
            );

            let models_dir = PathBuf::from(&config.models_dir);
            if models_dir.exists() {
                println!("models_dir: exists");
                // Check for models
                let medium = models_dir.join("ggml-medium.en.bin");
                let large = models_dir.join("ggml-large-v3-turbo.bin");
                println!(
                    "  medium: {}",
                    if medium.exists() { "found" } else { "not found" }
                );
                println!(
                    "  large-turbo: {}",
                    if large.exists() { "found" } else { "not found" }
                );
            } else {
                println!("models_dir: NOT FOUND");
            }

            Ok(())
        }
        ConfigAction::Set { key, value } => {
            let mut config = Config::load()?;

            match key.as_str() {
                "whisper_cli_path" => config.whisper_cli_path = value,
                "models_dir" => config.models_dir = value,
                "default_model" => {
                    if value != "medium" && value != "large-turbo" {
                        bail!("Invalid model. Use 'medium' or 'large-turbo'");
                    }
                    config.default_model = value;
                }
                _ => bail!(
                    "Unknown config key: {}. Valid keys: whisper_cli_path, models_dir, default_model",
                    key
                ),
            }

            config.save()?;
            println!("Configuration updated");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_transcription() {
        let output = r#"whisper_init_from_file_with_params_no_state: loading model from '/path/to/model'
whisper_model_load: loading model
main: processing '/path/to/audio.wav'

 Hello, this is a test.
 This is the second line.

"#;
        let result = extract_transcription(output);
        assert_eq!(result, "Hello, this is a test.\nThis is the second line.\n");
    }

    #[test]
    fn test_extract_transcription_empty() {
        let output = "whisper_init: starting\nmain: done\n";
        let result = extract_transcription(output);
        assert_eq!(result, "");
    }
}
