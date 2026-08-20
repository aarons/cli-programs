use anyhow::{Context, Result};
use serde::Serialize;
use std::io::Write as _;
use std::path::Path;

/// File name of the append-only generation log kept beside the images
pub const LOG_FILE_NAME: &str = "image-generation-log.jsonl";

/// One image-generation API call: the prompt and settings used, the cost the
/// provider reported for the call, and every file the call produced.
#[derive(Serialize)]
pub struct GenerationRecord {
    pub time: String,
    pub prompt: String,
    pub model: String,
    pub quality: String,
    pub size: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    pub files: Vec<String>,
}

/// Append a record to the generation log in `directory`, one JSON object per
/// line, creating the log on first use
pub fn append_record(directory: &Path, record: &GenerationRecord) -> Result<()> {
    let path = directory.join(LOG_FILE_NAME);
    let line =
        serde_json::to_string(record).context("Failed to serialize generation log record")?;

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("Failed to open {}", path.display()))?;
    writeln!(file, "{}", line).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(prompt: &str, cost: Option<f64>) -> GenerationRecord {
        GenerationRecord {
            time: "2026-07-29T14:35:02-07:00".to_string(),
            prompt: prompt.to_string(),
            model: "google/gemini-2.5-flash-image".to_string(),
            quality: "low".to_string(),
            size: "1024".to_string(),
            cost,
            files: vec!["2026-07-29-a-fox.png".to_string()],
        }
    }

    #[test]
    fn test_append_record_accumulates_parseable_json_lines() {
        let directory = std::env::temp_dir().join(format!(
            "get-image-log-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();

        append_record(&directory, &record("a fox", Some(0.0034))).unwrap();
        append_record(&directory, &record("a badger", None)).unwrap();

        let contents = std::fs::read_to_string(directory.join(LOG_FILE_NAME)).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2);

        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["prompt"], "a fox");
        assert_eq!(first["cost"], 0.0034);
        assert_eq!(first["files"][0], "2026-07-29-a-fox.png");

        // An unreported cost is omitted rather than logged as 0 or null
        let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert!(second.get("cost").is_none());

        std::fs::remove_dir_all(&directory).unwrap();
    }
}
