use anyhow::Result;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::path::PathBuf;

use crate::config;
use crate::generation_log;
use crate::openrouter::{GenerationSettings, ImageClient};
use crate::output;
use crate::template;
use crate::terminal_display::{self, InlineProtocol};

/// One image-generation session: the current prompt and settings, plus every
/// image saved so far. Drives both the initial generation and the interactive
/// tweak-and-regenerate loop.
pub struct Session {
    pub client: ImageClient,
    pub working_directory: PathBuf,
    pub prompt: String,
    pub settings: GenerationSettings,
    pub output_stem: Option<String>,
    pub open_after_save: bool,
    pub display_protocol: Option<InlineProtocol>,
    pub saved_paths: Vec<PathBuf>,
}

const HELP_TEXT: &str = "\
Enter          regenerate with the same prompt
<new text>     replace the prompt and generate (press Up to edit the last prompt)
               [a|b] in a prompt expands to one generation per option
/model <id>    switch model (see `get-image models` for options)
/quality <q>   set quality: low, medium, high, auto
/size <s>      set size: 512 or 1024x768
/count <n>     set number of images per generation (1-10)
/open [n]      open image n from /list (default: the newest image)
/list          list images generated this session
/settings      show current model, quality, size, and count
/save          save current settings as defaults in the config file
/help          show this help
/quit          exit (Ctrl-C and Ctrl-D also work)";

impl Session {
    /// Generate images for the current prompt — expanding [a|b] template
    /// groups into one generation per combination — and save them to the
    /// working directory, printing each saved path.
    pub async fn generate(&mut self) -> Result<()> {
        let prompts = template::expand_template(&self.prompt)?;

        let mut total_cost = 0.0;
        for (index, prompt) in prompts.iter().enumerate() {
            if prompts.len() > 1 {
                println!("[{}/{}] {}", index + 1, prompts.len(), prompt);
            }
            total_cost += self.generate_for_prompt(prompt).await?;
        }

        if prompts.len() > 1 && total_cost > 0.0 {
            println!("Total cost: ${:.4}", total_cost);
        }
        Ok(())
    }

    /// Generate images for one expanded prompt, returning the reported cost
    async fn generate_for_prompt(&mut self, prompt: &str) -> Result<f64> {
        let plural = if self.settings.count == 1 { "" } else { "s" };
        println!(
            "Generating {} image{} with {} (quality {}, size {})...",
            self.settings.count, plural, self.settings.model, self.settings.quality, self.settings.size
        );

        let result = self.client.generate(prompt, &self.settings).await?;

        if result.images.is_empty() {
            anyhow::bail!("The model returned no images.");
        }

        let now = chrono::Local::now();
        let stem = self
            .output_stem
            .clone()
            .unwrap_or_else(|| output::stem_for_prompt(prompt, now.date_naive()));

        let mut saved_file_names = Vec::new();
        for generated in &result.images {
            let image =
                output::decode_base64_image(&generated.b64_json, generated.media_type.as_deref())?;
            let path = output::save_image(&self.working_directory, &stem, &image)?;
            println!("Saved: {}", path.display());
            saved_file_names.push(path.file_name().unwrap_or_default().to_string_lossy().into_owned());

            if let Some(protocol) = self.display_protocol
                && let Err(error) = terminal_display::display_inline(protocol, &image)
            {
                eprintln!("Could not display image: {}", error);
            }

            if self.open_after_save
                && let Err(error) = output::open_in_viewer(&path)
            {
                eprintln!("Could not open image: {}", error);
            }

            self.saved_paths.push(path);
        }

        let record = generation_log::GenerationRecord {
            time: now.to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
            prompt: prompt.to_string(),
            model: self.settings.model.clone(),
            quality: self.settings.quality.clone(),
            size: self.settings.size.clone(),
            cost: result.cost,
            files: saved_file_names,
        };
        // A log failure shouldn't discard an otherwise successful generation
        if let Err(error) = generation_log::append_record(&self.working_directory, &record) {
            eprintln!(
                "Could not write {}: {:#}",
                generation_log::LOG_FILE_NAME,
                error
            );
        }

        let cost = result.cost.unwrap_or(0.0);
        if cost > 0.0 {
            println!("Cost: ${:.4}", cost);
        }
        Ok(cost)
    }

    /// Run the interactive loop: Enter regenerates, new text replaces the
    /// prompt, /commands adjust settings and browse generated images.
    pub async fn run_interactive(&mut self) -> Result<()> {
        println!();
        println!("Interactive session: Enter regenerates, /help lists commands.");

        let mut editor = DefaultEditor::new()?;
        editor.add_history_entry(&self.prompt)?;

        loop {
            let line = match editor.readline("get-image> ") {
                Ok(line) => line,
                Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
                Err(error) => return Err(error.into()),
            };
            let input = line.trim().to_string();

            if let Some(command) = input.strip_prefix('/') {
                if self.handle_command(command) {
                    break;
                }
                continue;
            }

            if !input.is_empty() && input != self.prompt {
                self.prompt = input.clone();
                editor.add_history_entry(&input)?;
                // A new prompt usually deserves a fresh filename
                self.output_stem = None;
            }

            if let Err(error) = self.generate().await {
                eprintln!("Error: {:#}", error);
            }
        }

        Ok(())
    }

    /// Handle a /command. Returns true when the session should end.
    fn handle_command(&mut self, command: &str) -> bool {
        let (name, argument) = match command.split_once(char::is_whitespace) {
            Some((name, argument)) => (name, argument.trim()),
            None => (command, ""),
        };

        let outcome: Result<()> = match name {
            "quit" | "exit" | "q" => return true,
            "help" | "h" | "?" => {
                println!("{}", HELP_TEXT);
                Ok(())
            }
            "model" | "m" => self.set_model(argument),
            "quality" => self.set_from_parser(argument, "quality"),
            "size" | "s" => self.set_from_parser(argument, "size"),
            "count" | "n" => self.set_from_parser(argument, "count"),
            "open" | "o" => self.open_image(argument),
            "list" | "l" | "ls" => {
                self.print_image_list();
                Ok(())
            }
            "settings" => {
                self.print_settings();
                Ok(())
            }
            "save" => self.save_settings_as_defaults(),
            _ => {
                println!("Unknown command: /{}  (try /help)", name);
                Ok(())
            }
        };

        if let Err(error) = outcome {
            eprintln!("Error: {:#}", error);
        }
        false
    }

    fn set_model(&mut self, argument: &str) -> Result<()> {
        if argument.is_empty() {
            println!("Current model: {}", self.settings.model);
            println!("Usage: /model <id>  (run `get-image models` in another shell to browse)");
            return Ok(());
        }
        self.settings.model = argument.to_string();
        println!("Model set to {}", self.settings.model);
        Ok(())
    }

    fn set_from_parser(&mut self, argument: &str, key: &str) -> Result<()> {
        match key {
            "quality" => {
                if argument.is_empty() {
                    println!("Current quality: {}", self.settings.quality);
                    return Ok(());
                }
                self.settings.quality = config::parse_quality(argument)?;
                println!("Quality set to {}", self.settings.quality);
            }
            "size" => {
                if argument.is_empty() {
                    println!("Current size: {}", self.settings.size);
                    return Ok(());
                }
                self.settings.size = config::parse_size(argument)?;
                println!("Size set to {}", self.settings.size);
            }
            "count" => {
                if argument.is_empty() {
                    println!("Current count: {}", self.settings.count);
                    return Ok(());
                }
                self.settings.count = config::parse_count(argument)?;
                println!("Count set to {}", self.settings.count);
            }
            _ => unreachable!("set_from_parser called with unknown key"),
        }
        Ok(())
    }

    fn open_image(&self, argument: &str) -> Result<()> {
        let path = if argument.is_empty() {
            self.saved_paths.last()
        } else {
            let index: usize = argument
                .parse()
                .map_err(|_| anyhow::anyhow!("Usage: /open [number from /list]"))?;
            self.saved_paths.get(index.wrapping_sub(1))
        };

        match path {
            Some(path) => output::open_in_viewer(path),
            None if self.saved_paths.is_empty() => {
                println!("No images generated yet.");
                Ok(())
            }
            None => {
                println!(
                    "No image {} — session has {} (see /list).",
                    argument,
                    self.saved_paths.len()
                );
                Ok(())
            }
        }
    }

    fn print_image_list(&self) {
        if self.saved_paths.is_empty() {
            println!("No images generated yet.");
            return;
        }
        for (index, path) in self.saved_paths.iter().enumerate() {
            println!("{:>3}  {}", index + 1, path.display());
        }
    }

    fn print_settings(&self) {
        println!("model:   {}", self.settings.model);
        println!("quality: {}", self.settings.quality);
        println!("size:    {}", self.settings.size);
        println!("count:   {}", self.settings.count);
    }

    fn save_settings_as_defaults(&self) -> Result<()> {
        let mut config = config::Config::load()?;
        config.model = self.settings.model.clone();
        config.quality = self.settings.quality.clone();
        config.size = self.settings.size.clone();
        config.count = self.settings.count;
        config.save()?;
        println!(
            "Saved current settings as defaults in {}",
            config::Config::config_path()?.display()
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openrouter::ImageClient;

    fn session() -> Session {
        Session {
            client: ImageClient::new("test-key".to_string(), false),
            working_directory: std::env::temp_dir(),
            prompt: "a fox".to_string(),
            settings: GenerationSettings {
                model: "google/gemini-2.5-flash-image".to_string(),
                quality: "low".to_string(),
                size: "512".to_string(),
                count: 1,
            },
            output_stem: None,
            open_after_save: false,
            display_protocol: None,
            saved_paths: Vec::new(),
        }
    }

    #[test]
    fn test_commands_update_settings_in_place() {
        let mut session = session();

        assert!(!session.handle_command("model openai/gpt-image-1"));
        assert_eq!(session.settings.model, "openai/gpt-image-1");

        assert!(!session.handle_command("quality high"));
        assert_eq!(session.settings.quality, "high");

        assert!(!session.handle_command("size 2k"));
        assert_eq!(session.settings.size, "2K");

        assert!(!session.handle_command("n 4"));
        assert_eq!(session.settings.count, 4);
    }

    #[test]
    fn test_invalid_command_values_leave_settings_unchanged() {
        let mut session = session();

        assert!(!session.handle_command("quality ultra"));
        assert_eq!(session.settings.quality, "low");

        assert!(!session.handle_command("count 99"));
        assert_eq!(session.settings.count, 1);
    }

    #[test]
    fn test_only_quit_commands_end_the_session() {
        let mut session = session();
        assert!(!session.handle_command("help"));
        assert!(!session.handle_command("list"));
        assert!(!session.handle_command("definitely-not-a-command"));
        assert!(session.handle_command("quit"));
        assert!(session.handle_command("exit"));
    }
}
