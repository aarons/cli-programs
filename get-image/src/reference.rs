//! Reference images for image-to-image generation.
//!
//! A reference is given as an HTTP(S) URL or a local file path. URLs pass
//! through to the API untouched; local files are read and embedded as base64
//! data URLs, which is the other form the Images API accepts.

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use std::path::Path;

/// One reference image, ready to send with a generation request
#[derive(Clone, Debug, PartialEq)]
pub struct ImageReference {
    /// The path or URL as the user gave it, for display
    pub label: String,
    /// HTTP(S) URL or base64 data URL, as sent to the API
    pub url: String,
}

impl ImageReference {
    /// Load a reference from a URL or local file path
    pub fn load(source: &str) -> Result<Self> {
        let source = source.trim();
        if source.is_empty() {
            anyhow::bail!("Reference image path or URL is empty");
        }

        if is_http_url(source) {
            return Ok(Self {
                label: source.to_string(),
                url: source.to_string(),
            });
        }

        let path = Path::new(source);
        let data = std::fs::read(path)
            .with_context(|| format!("Failed to read reference image: {}", source))?;
        let media_type = detect_media_type(&data, path).with_context(|| {
            format!(
                "Unrecognized image format for reference image: {} (expected png, jpeg, webp, or gif)",
                source
            )
        })?;

        Ok(Self {
            label: source.to_string(),
            url: format!("data:{};base64,{}", media_type, BASE64.encode(&data)),
        })
    }
}

fn is_http_url(source: &str) -> bool {
    let lowercase = source.to_ascii_lowercase();
    lowercase.starts_with("http://") || lowercase.starts_with("https://")
}

/// Identify the image format from its magic bytes, falling back to the file
/// extension for anything unrecognized
fn detect_media_type(data: &[u8], path: &Path) -> Option<&'static str> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if data.starts_with(b"\xFF\xD8\xFF") {
        return Some("image/jpeg");
    }
    if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        return Some("image/webp");
    }

    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str, contents: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("get-image-reference-{}-{}", std::process::id(), name));
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn test_http_url_passes_through_unchanged() {
        let reference = ImageReference::load("https://example.com/photo.jpg").unwrap();
        assert_eq!(reference.url, "https://example.com/photo.jpg");
        assert_eq!(reference.label, "https://example.com/photo.jpg");
    }

    #[test]
    fn test_local_file_becomes_data_url_with_sniffed_media_type() {
        // PNG signature with a misleading extension: magic bytes win
        let path = temp_file("sniffed.jpg", b"\x89PNG\r\n\x1a\nrest");
        let reference = ImageReference::load(path.to_str().unwrap()).unwrap();
        std::fs::remove_file(&path).unwrap();

        assert!(reference.url.starts_with("data:image/png;base64,"));
        assert_eq!(reference.label, path.to_str().unwrap());
        let encoded = reference.url.strip_prefix("data:image/png;base64,").unwrap();
        assert_eq!(BASE64.decode(encoded).unwrap(), b"\x89PNG\r\n\x1a\nrest");
    }

    #[test]
    fn test_unknown_signature_falls_back_to_extension() {
        let path = temp_file("fallback.webp", b"not a real image");
        let reference = ImageReference::load(path.to_str().unwrap()).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(reference.url.starts_with("data:image/webp;base64,"));
    }

    #[test]
    fn test_unrecognized_format_is_rejected() {
        let path = temp_file("notes.txt", b"just text");
        let error = ImageReference::load(path.to_str().unwrap()).unwrap_err();
        std::fs::remove_file(&path).unwrap();
        assert!(error.to_string().contains("Unrecognized image format"));
    }

    #[test]
    fn test_missing_file_reports_the_path() {
        let error = ImageReference::load("/definitely/not/here.png").unwrap_err();
        assert!(error.to_string().contains("/definitely/not/here.png"));
    }
}
