use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;
use tempfile::NamedTempFile;

/// Required sample rate for whisper.cpp
const REQUIRED_SAMPLE_RATE: u32 = 16000;

/// Audio file information from ffprobe
#[derive(Debug)]
pub struct AudioInfo {
    pub sample_rate: u32,
    pub channels: u32,
    pub codec: String,
}

impl AudioInfo {
    /// Check if audio needs conversion to meet whisper requirements
    pub fn needs_conversion(&self) -> bool {
        self.sample_rate != REQUIRED_SAMPLE_RATE || self.channels != 1
    }

    /// Get a human-readable description of issues
    pub fn issues(&self) -> Vec<String> {
        let mut issues = Vec::new();
        if self.sample_rate != REQUIRED_SAMPLE_RATE {
            issues.push(format!(
                "sample rate is {} Hz (requires {} Hz)",
                self.sample_rate, REQUIRED_SAMPLE_RATE
            ));
        }
        if self.channels != 1 {
            issues.push(format!(
                "audio has {} channels (requires mono)",
                self.channels
            ));
        }
        issues
    }
}

/// Get audio file information using ffprobe
pub fn check_audio_format(path: &Path) -> Result<AudioInfo> {
    // Check if ffprobe is available
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_streams",
            "-select_streams",
            "a:0",
        ])
        .arg(path)
        .output()
        .context("Failed to run ffprobe. Is ffmpeg installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("ffprobe failed: {}", stderr);
    }

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("Failed to parse ffprobe output")?;

    let streams = json["streams"]
        .as_array()
        .context("No audio streams found in file")?;

    if streams.is_empty() {
        bail!("No audio streams found in file");
    }

    let stream = &streams[0];

    let sample_rate = stream["sample_rate"]
        .as_str()
        .context("Missing sample_rate")?
        .parse::<u32>()
        .context("Invalid sample_rate")?;

    let channels = stream["channels"]
        .as_u64()
        .context("Missing channels")?
        .try_into()
        .context("Invalid channels")?;

    let codec = stream["codec_name"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    Ok(AudioInfo {
        sample_rate,
        channels,
        codec,
    })
}

/// Convert audio file to whisper-compatible format (16kHz mono PCM WAV)
/// Returns a temporary file that will be deleted when dropped
pub fn convert_audio(input: &Path) -> Result<NamedTempFile> {
    let temp_file = NamedTempFile::with_suffix(".wav").context("Failed to create temp file")?;

    let output = Command::new("ffmpeg")
        .args([
            "-i",
            input.to_str().context("Invalid input path")?,
            "-ar",
            "16000", // 16kHz sample rate
            "-ac",
            "1", // Mono
            "-c:a",
            "pcm_s16le", // 16-bit PCM
            "-y",        // Overwrite output
        ])
        .arg(temp_file.path())
        .output()
        .context("Failed to run ffmpeg. Is ffmpeg installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("ffmpeg conversion failed: {}", stderr);
    }

    Ok(temp_file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_conversion() {
        let info = AudioInfo {
            sample_rate: 16000,
            channels: 1,
            codec: "pcm_s16le".to_string(),
        };
        assert!(!info.needs_conversion());

        let info = AudioInfo {
            sample_rate: 44100,
            channels: 1,
            codec: "pcm_s16le".to_string(),
        };
        assert!(info.needs_conversion());

        let info = AudioInfo {
            sample_rate: 16000,
            channels: 2,
            codec: "pcm_s16le".to_string(),
        };
        assert!(info.needs_conversion());
    }

    #[test]
    fn test_issues() {
        let info = AudioInfo {
            sample_rate: 44100,
            channels: 2,
            codec: "pcm_s16le".to_string(),
        };
        let issues = info.issues();
        assert_eq!(issues.len(), 2);
        assert!(issues[0].contains("44100"));
        assert!(issues[1].contains("2 channels"));
    }
}
