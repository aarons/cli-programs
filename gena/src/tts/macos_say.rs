// macOS say command TTS backend

use super::{TtsBackend, TtsOptions, Voice};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::Path;
use tokio::process::Command;

/// macOS TTS backend using the `say` command
pub struct MacOsSayBackend;

impl MacOsSayBackend {
    pub fn new() -> Self {
        Self
    }

    /// Parse voice list output from `say -v ?`
    fn parse_voice_list(output: &str) -> Vec<Voice> {
        let mut voices = Vec::new();

        for line in output.lines() {
            // Format: "Name    language  # description"
            // Example: "Alex    en_US     # Most people recognize me by my voice."
            let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
            if parts.is_empty() {
                continue;
            }

            let name = parts[0].trim().to_string();
            if name.is_empty() {
                continue;
            }

            // Try to extract language
            let language = if parts.len() > 1 {
                let rest = parts[1].trim();
                let lang_part: Vec<&str> = rest.splitn(2, '#').collect();
                if !lang_part.is_empty() {
                    let lang = lang_part[0].trim();
                    if !lang.is_empty() {
                        Some(lang.to_string())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            voices.push(Voice {
                id: name.clone(),
                name,
                language,
            });
        }

        voices
    }
}

impl Default for MacOsSayBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TtsBackend for MacOsSayBackend {
    async fn synthesize(&self, text: &str, output_path: &Path, options: &TtsOptions) -> Result<()> {
        // Build say command
        let mut cmd = Command::new("say");

        // Add voice if specified
        if let Some(voice) = &options.voice {
            cmd.arg("-v").arg(voice);
        }

        // Add rate if specified
        if let Some(rate) = options.rate {
            cmd.arg("-r").arg(rate.to_string());
        }

        // Determine output format from extension
        let extension = output_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("aiff");

        // For m4a, we need to generate AIFF first then convert
        let needs_conversion = extension.eq_ignore_ascii_case("m4a");

        let temp_path;
        let actual_output = if needs_conversion {
            temp_path = output_path.with_extension("aiff");
            &temp_path
        } else {
            output_path
        };

        // Output to file
        cmd.arg("-o").arg(actual_output);

        // Pass text via stdin to avoid shell escaping issues
        cmd.stdin(std::process::Stdio::piped());

        let mut child = cmd.spawn().context("Failed to spawn say command")?;

        // Write text to stdin
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin
                .write_all(text.as_bytes())
                .await
                .context("Failed to write to say stdin")?;
        }

        let status = child.wait().await.context("Failed to wait for say")?;

        if !status.success() {
            anyhow::bail!("say command failed with status: {}", status);
        }

        // Convert to m4a if needed
        if needs_conversion {
            let convert_status = Command::new("afconvert")
                .arg("-f")
                .arg("m4af") // M4A format
                .arg("-d")
                .arg("aac") // AAC codec
                .arg("-b")
                .arg("128000") // 128kbps bitrate
                .arg(actual_output)
                .arg(output_path)
                .status()
                .await
                .context("Failed to run afconvert")?;

            if !convert_status.success() {
                anyhow::bail!("afconvert failed with status: {}", convert_status);
            }

            // Clean up temp file
            tokio::fs::remove_file(actual_output)
                .await
                .context("Failed to remove temp AIFF file")?;
        }

        Ok(())
    }

    fn list_voices(&self) -> Result<Vec<Voice>> {
        // Use blocking command for simplicity (called once at startup)
        let output = std::process::Command::new("say")
            .arg("-v")
            .arg("?")
            .output()
            .context("Failed to run say -v ?")?;

        if !output.status.success() {
            anyhow::bail!("say -v ? failed");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(Self::parse_voice_list(&stdout))
    }

    fn name(&self) -> &str {
        "macos-say"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_voice_list() {
        let output = r#"Alex                en_US    # Most people recognize me by my voice.
Daniel              en_GB    # Hello, my name is Daniel. I am a British-English voice.
Samantha            en_US    # Hello, my name is Samantha. I am an American-English voice.
"#;
        let voices = MacOsSayBackend::parse_voice_list(output);
        assert_eq!(voices.len(), 3);
        assert_eq!(voices[0].name, "Alex");
        assert_eq!(voices[0].language, Some("en_US".to_string()));
        assert_eq!(voices[1].name, "Daniel");
        assert_eq!(voices[1].language, Some("en_GB".to_string()));
    }
}
