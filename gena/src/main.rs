// gena - Convert EPUB files to audio using text-to-speech

mod config;
mod epub;
mod tts;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use config::GenaConfig;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use tts::{TtsBackend, TtsOptions};

#[derive(Parser, Debug)]
#[command(name = "gena")]
#[command(about = "Convert EPUB files to audio using text-to-speech", long_about = None)]
#[command(version)]
struct Args {
    /// Path to the EPUB file
    epub_file: Option<PathBuf>,

    /// Output file path (default: <epub-name>.m4a)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Voice to use for TTS
    #[arg(short, long)]
    voice: Option<String>,

    /// Speaking rate in words per minute
    #[arg(short, long)]
    rate: Option<u32>,

    /// TTS backend to use
    #[arg(short, long)]
    backend: Option<String>,

    /// List available voices
    #[arg(long)]
    list_voices: bool,

    /// Enable debug output
    #[arg(short, long, default_value_t = false)]
    debug: bool,

    /// Configuration subcommand
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set default voice
    SetVoice {
        /// Voice name to use
        voice: String,
    },
    /// Set default backend
    SetBackend {
        /// Backend name (e.g., macos-say)
        backend: String,
    },
    /// Set default speaking rate
    SetRate {
        /// Rate in words per minute
        rate: u32,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Handle config subcommands
    if let Some(Commands::Config { action }) = &args.command {
        return handle_config_command(action);
    }

    // Load configuration
    let config = GenaConfig::load().context("Failed to load configuration")?;

    // Determine backend
    let backend_name = args.backend.as_deref().unwrap_or(&config.backend);
    let backend = tts::create_backend(backend_name)?;

    // Handle --list-voices
    if args.list_voices {
        return list_voices(&*backend);
    }

    // Require EPUB file for conversion
    let epub_path = args
        .epub_file
        .ok_or_else(|| anyhow::anyhow!("EPUB file path is required"))?;

    if !epub_path.exists() {
        anyhow::bail!("EPUB file not found: {}", epub_path.display());
    }

    // Determine output path
    let output_path = args.output.unwrap_or_else(|| {
        let stem = epub_path.file_stem().unwrap_or_default();
        epub_path.with_file_name(format!("{}.m4a", stem.to_string_lossy()))
    });

    // Build TTS options
    let tts_options = TtsOptions {
        voice: args.voice.or(config.voice),
        rate: Some(args.rate.unwrap_or(config.rate)),
    };

    if args.debug {
        eprintln!("EPUB: {}", epub_path.display());
        eprintln!("Output: {}", output_path.display());
        eprintln!("Backend: {}", backend.name());
        eprintln!("Voice: {:?}", tts_options.voice);
        eprintln!("Rate: {:?}", tts_options.rate);
    }

    // Parse EPUB
    eprintln!("Parsing EPUB: {}", epub_path.display());
    let book = epub::parse_epub(&epub_path).context("Failed to parse EPUB")?;

    eprintln!(
        "Book: \"{}\" by {}",
        book.title,
        book.author.as_deref().unwrap_or("Unknown")
    );
    eprintln!(
        "Chapters: {}, Words: ~{}",
        book.chapters.len(),
        book.total_words()
    );

    if book.chapters.is_empty() {
        anyhow::bail!("No chapters found in EPUB");
    }

    // Combine all chapter text
    let mut full_text = String::new();
    for chapter in &book.chapters {
        if let Some(title) = &chapter.title {
            full_text.push_str(title);
            full_text.push_str(".\n\n");
        }
        full_text.push_str(&chapter.content);
        full_text.push_str("\n\n");
    }

    // Estimate duration (rough: 150 words/min average)
    let words = full_text.split_whitespace().count();
    let rate = tts_options.rate.unwrap_or(150);
    let estimated_minutes = words as f64 / rate as f64;

    eprintln!(
        "Generating audio (~{:.0} minutes estimated)...",
        estimated_minutes
    );

    // Create progress bar
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message("Synthesizing speech...");
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    // Generate audio
    backend
        .synthesize(&full_text, &output_path, &tts_options)
        .await
        .context("Failed to synthesize audio")?;

    pb.finish_with_message("Done!");

    // Get output file size
    let metadata = tokio::fs::metadata(&output_path).await?;
    let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);

    eprintln!("Output: {} ({:.1} MB)", output_path.display(), size_mb);

    Ok(())
}

fn handle_config_command(action: &ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Show => {
            let config = GenaConfig::load()?;
            println!("Configuration file: {:?}", GenaConfig::config_path()?);
            println!();
            println!("backend = \"{}\"", config.backend);
            if let Some(voice) = &config.voice {
                println!("voice = \"{}\"", voice);
            } else {
                println!("voice = (system default)");
            }
            println!("rate = {}", config.rate);
        }
        ConfigAction::SetVoice { voice } => {
            let mut config = GenaConfig::load()?;
            config.voice = Some(voice.clone());
            config.save()?;
            println!("Default voice set to: {}", voice);
        }
        ConfigAction::SetBackend { backend } => {
            // Validate backend exists
            tts::create_backend(backend)?;
            let mut config = GenaConfig::load()?;
            config.backend = backend.clone();
            config.save()?;
            println!("Default backend set to: {}", backend);
        }
        ConfigAction::SetRate { rate } => {
            let mut config = GenaConfig::load()?;
            config.rate = *rate;
            config.save()?;
            println!("Default rate set to: {} WPM", rate);
        }
    }
    Ok(())
}

fn list_voices(backend: &dyn TtsBackend) -> Result<()> {
    let voices = backend.list_voices()?;

    println!("Available voices for {} backend:", backend.name());
    println!();

    for voice in voices {
        if let Some(lang) = &voice.language {
            println!("  {} ({})", voice.name, lang);
        } else {
            println!("  {}", voice.name);
        }
    }

    Ok(())
}
