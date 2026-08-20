mod config;
mod generation_log;
mod openrouter;
mod output;
mod session;
mod template;
mod terminal_display;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use std::io::IsTerminal;

use config::Config;
use openrouter::{GenerationSettings, ImageClient};

#[derive(Parser, Debug)]
#[command(
    name = "get-image",
    about = "Generate images from a text prompt via OpenRouter image models",
    long_about = "Generates images from a text prompt using OpenRouter image models and saves \
them to the current working directory. After the first image, an interactive session lets you \
tweak the prompt and settings and regenerate quickly.\n\nOn terminals with inline-image support \
(iTerm2, WezTerm, kitty, Ghostty) each image is also rendered in the terminal.\n\nPrompts may \
contain [a|b] template groups, expanded into one generation per combination: \"a [red|blue] \
[cat|dog]\" generates four images."
)]
#[command(version)]
struct Args {
    /// OpenRouter model id (see `get-image models` for options)
    #[arg(short, long)]
    model: Option<String>,

    /// Image quality: low, medium, high, or auto
    #[arg(short, long)]
    quality: Option<String>,

    /// Image size: a single dimension (512) or WIDTHxHEIGHT (1024x768)
    #[arg(short, long)]
    size: Option<String>,

    /// Number of images to generate (1-10)
    #[arg(short = 'n', long)]
    count: Option<u32>,

    /// Output filename stem (default: the date plus a short prompt slug)
    #[arg(short, long)]
    output: Option<String>,

    /// Open images in the system viewer after saving
    #[arg(long)]
    open: bool,

    /// Don't render images inline in the terminal
    #[arg(long)]
    no_display: bool,

    /// Generate once and exit without the interactive session
    #[arg(long)]
    once: bool,

    /// Enable debug output (prints request/response details)
    #[arg(short, long)]
    debug: bool,

    #[command(subcommand)]
    command: Option<Commands>,

    /// The image prompt
    #[arg(trailing_var_arg = true)]
    prompt: Vec<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// List available image models with pricing, cheapest first
    Models,
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// Show current configuration and the config file path
    Show,
    /// Set a default: model, quality, size, count, or open_after_save
    Set {
        /// Config key (model, quality, size, count, open_after_save)
        key: String,
        /// New value
        value: String,
    },
    /// Print the config file path
    Path,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    match &args.command {
        Some(Commands::Config { action }) => return handle_config_command(action),
        Some(Commands::Models) => {
            // Model browsing hits a public endpoint; no API key required
            let client = ImageClient::new(resolve_api_key().unwrap_or_default(), args.debug);
            return print_model_list(&client).await;
        }
        None => {}
    }

    let prompt = args.prompt.join(" ");
    if prompt.trim().is_empty() {
        Args::command().print_long_help()?;
        return Ok(());
    }

    let config = Config::load()?;
    let settings = build_settings(&args, &config)?;
    let open_after_save = args.open || config.open_after_save;

    let client = ImageClient::new(resolve_api_key()?, args.debug);
    let working_directory = std::env::current_dir().context("Cannot determine current directory")?;

    let display_protocol = if args.no_display || !std::io::stdout().is_terminal() {
        None
    } else {
        terminal_display::detect_protocol()
    };

    let mut session = session::Session {
        client,
        working_directory,
        prompt,
        settings,
        output_stem: args.output,
        open_after_save,
        display_protocol,
        saved_paths: Vec::new(),
    };

    session.generate().await?;

    let interactive = !args.once && std::io::stdin().is_terminal();
    if interactive {
        session.run_interactive().await?;
    }

    Ok(())
}

/// Resolve the OpenRouter API key: shared llm-client config first, then the
/// OPENROUTER_API_KEY environment variable.
fn resolve_api_key() -> Result<String> {
    if let Ok(llm_config) = llm_client::Config::load()
        && let Some(provider_config) = llm_config.get_provider_config("openrouter")
        && let Some(api_key) = &provider_config.api_key
    {
        return Ok(api_key.clone());
    }

    std::env::var("OPENROUTER_API_KEY").context(
        "No OpenRouter API key found. Set the OPENROUTER_API_KEY environment variable, or add \
an api_key under [providers.openrouter] in ~/.config/cli-programs/llm.toml",
    )
}

/// Merge command-line flags over config defaults into generation settings
fn build_settings(args: &Args, config: &Config) -> Result<GenerationSettings> {
    let quality = match &args.quality {
        Some(quality) => config::parse_quality(quality)?,
        None => config.quality.clone(),
    };
    let size = match &args.size {
        Some(size) => config::parse_size(size)?,
        None => config.size.clone(),
    };
    let count = match args.count {
        Some(count) => config::parse_count(&count.to_string())?,
        None => config.count,
    };

    Ok(GenerationSettings {
        model: args.model.clone().unwrap_or_else(|| config.model.clone()),
        quality,
        size,
        count,
    })
}

fn handle_config_command(action: &ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Show => {
            let config = Config::load()?;
            let path = Config::config_path()?;
            println!("Config file: {}", path.display());
            println!();
            println!("model           = {}", config.model);
            println!("quality         = {}", config.quality);
            println!("size            = {}", config.size);
            println!("count           = {}", config.count);
            println!("open_after_save = {}", config.open_after_save);
        }
        ConfigAction::Set { key, value } => {
            let mut config = Config::load()?;
            config.set(key, value)?;
            config.save()?;
            println!("Set {} = {}", key, value);
        }
        ConfigAction::Path => {
            println!("{}", Config::config_path()?.display());
        }
    }
    Ok(())
}

/// Print available image models with pricing, cheapest first
async fn print_model_list(client: &ImageClient) -> Result<()> {
    let models = client.list_image_models().await?;

    if models.is_empty() {
        println!("No image-capable models found.");
        return Ok(());
    }

    let id_width = models
        .iter()
        .map(|model| model.id.len())
        .max()
        .unwrap_or(0)
        .max("MODEL".len());

    println!("{:<id_width$}  {:>12}  NAME", "MODEL", "EST $/IMAGE");
    for model in &models {
        println!(
            "{:<id_width$}  {:>12}  {}",
            model.id,
            model.price_per_image_display(),
            model.name,
        );
    }
    println!();
    println!(
        "{} models. Use with: get-image --model <MODEL> \"prompt\"",
        models.len()
    );
    println!(
        "Prices are full-quality estimates; the actual cost is printed after each generation."
    );
    Ok(())
}
