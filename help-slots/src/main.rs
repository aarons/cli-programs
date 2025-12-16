use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rdev::Key;
use std::path::PathBuf;

mod app;
mod capture;
mod input;
mod preprocessing;
mod puzzles;
mod window;

use app::App;
use capture::Capturer;
use input::InputHandler;
use preprocessing::Preprocessor;
use puzzles::reticle::ReticleHandler;
use puzzles::PuzzleClassifier;
use window::GameWindow;

const GAME_WINDOW_NAME: &str = "SlotsAndDaggers";

#[derive(Parser, Debug)]
#[command(name = "help-slots", about = "Timing puzzle assistant for SlotsAndDaggers", version)]
struct Args {
    #[command(subcommand)]
    command: Commands,

    /// Enable debug logging
    #[arg(short, long, global = true)]
    debug: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the helper
    Run {
        /// Path to puzzle template image
        #[arg(long)]
        puzzle_template: Option<PathBuf>,

        /// Path to reticle template image
        #[arg(long)]
        reticle_template: Option<PathBuf>,
    },

    /// Test screen capture
    TestCapture {
        /// Output path for captured image
        #[arg(short, long, default_value = "capture.png")]
        output: PathBuf,
    },

    /// Test preprocessing pipeline
    TestPreprocess {
        /// Input image path (or capture from game if not specified)
        #[arg(short, long)]
        input: Option<PathBuf>,

        /// Output prefix for processed images
        #[arg(short, long, default_value = "preprocess")]
        output: String,
    },

    /// Test window detection
    TestWindow,

    /// Test spacebar injection
    TestSpacebar,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.debug { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    match args.command {
        Commands::Run {
            puzzle_template,
            reticle_template,
        } => run_helper(puzzle_template, reticle_template).await,
        Commands::TestCapture { output } => test_capture(&output),
        Commands::TestPreprocess { input, output } => test_preprocess(input.as_deref(), &output),
        Commands::TestWindow => test_window(),
        Commands::TestSpacebar => test_spacebar(),
    }
}

async fn run_helper(
    puzzle_template: Option<PathBuf>,
    reticle_template: Option<PathBuf>,
) -> Result<()> {
    log::info!("Starting help-slots for {}", GAME_WINDOW_NAME);

    // Check if game window exists
    if GameWindow::find_by_name(GAME_WINDOW_NAME)?.is_none() {
        log::warn!(
            "Game window '{}' not found. Helper will wait for it to appear.",
            GAME_WINDOW_NAME
        );
    }

    // Set up input handler
    let input = InputHandler::new(Key::KeyF);
    let enabled = input.enabled_flag();

    // Start hotkey listener in background
    let _hotkey_thread = input.start_hotkey_listener();
    log::info!("Hotkey listener started (press 'F' to toggle)");

    // Set up preprocessor
    let preprocessor = Preprocessor::with_defaults();

    // Set up classifier with reticle handler
    let mut classifier = PuzzleClassifier::new();
    let mut reticle_handler = ReticleHandler::with_defaults();

    // Load templates if provided
    if puzzle_template.is_some() || reticle_template.is_some() {
        reticle_handler.load_templates(
            puzzle_template.as_ref().and_then(|p| p.to_str()),
            reticle_template.as_ref().and_then(|p| p.to_str()),
        )?;
    } else {
        log::warn!("No templates provided. Use --puzzle-template and --reticle-template to enable detection.");
    }

    classifier.add_handler(Box::new(reticle_handler));

    // Create and run app
    let mut app = App::new(GAME_WINDOW_NAME, preprocessor, classifier, enabled);
    app.run().await
}

fn test_capture(output: &PathBuf) -> Result<()> {
    log::info!("Testing screen capture for '{}'", GAME_WINDOW_NAME);

    let capturer = Capturer::new(GAME_WINDOW_NAME);
    let image = capturer
        .capture_full()
        .context("Failed to capture game window")?;

    image.save(output).context("Failed to save capture")?;

    log::info!(
        "Captured {}x{} image to {:?}",
        image.width(),
        image.height(),
        output
    );
    Ok(())
}

fn test_preprocess(input: Option<&std::path::Path>, output_prefix: &str) -> Result<()> {
    log::info!("Testing preprocessing pipeline");

    // Get input image
    let image = if let Some(path) = input {
        log::info!("Loading image from {:?}", path);
        let img = image::open(path).context("Failed to open image")?;
        img.to_rgba8()
    } else {
        log::info!("Capturing from game window");
        let capturer = Capturer::new(GAME_WINDOW_NAME);
        capturer
            .capture_full()
            .context("Failed to capture game window")?
    };

    log::info!("Input image: {}x{}", image.width(), image.height());

    // Run preprocessing with intermediates
    let preprocessor = Preprocessor::with_defaults();
    let result = preprocessor.process_with_intermediates(&image);

    // Save all stages
    result.save_debug(output_prefix)?;

    log::info!("Saved preprocessing stages:");
    log::info!("  {}_1_gray.png - Grayscale conversion", output_prefix);
    log::info!("  {}_2_blurred.png - Gaussian blur", output_prefix);
    log::info!("  {}_3_edges.png - Canny edge detection", output_prefix);

    Ok(())
}

fn test_window() -> Result<()> {
    log::info!("Testing window detection for '{}'", GAME_WINDOW_NAME);

    match GameWindow::find_by_name(GAME_WINDOW_NAME)? {
        Some(window) => {
            log::info!("Found window:");
            log::info!("  App: {}", window.app_name);
            log::info!("  Title: {}", window.title);
            log::info!("  ID: {}", window.window_id);
            let bounds = window.bounds();
            log::info!(
                "  Bounds: ({}, {}) {}x{}",
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height
            );
            log::info!("  Focused: {}", window.is_focused()?);
        }
        None => {
            log::warn!("Window '{}' not found", GAME_WINDOW_NAME);
            log::info!("Make sure the game is running");
        }
    }

    Ok(())
}

fn test_spacebar() -> Result<()> {
    log::info!("Testing spacebar injection");
    log::info!("Sending spacebar in 2 seconds...");

    std::thread::sleep(std::time::Duration::from_secs(2));

    InputHandler::send_spacebar()?;

    log::info!("Spacebar sent successfully");
    Ok(())
}
