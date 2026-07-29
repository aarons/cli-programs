use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use std::path::{Path, PathBuf};

/// Maximum length of a filename stem derived from a prompt
const SLUG_LENGTH_MAX: usize = 48;

/// A decoded image ready to be written to disk
pub struct DecodedImage {
    pub data: Vec<u8>,
    pub extension: &'static str,
}

/// Decode base64 image data (as returned by the OpenRouter Images API) into
/// raw bytes and a file extension. The media type may be absent when the API
/// couldn't determine the format; PNG is assumed then.
pub fn decode_base64_image(base64_data: &str, media_type: Option<&str>) -> Result<DecodedImage> {
    let data = BASE64
        .decode(base64_data.trim())
        .context("Failed to decode base64 image data")?;

    Ok(DecodedImage {
        data,
        extension: extension_for_media_type(media_type.unwrap_or("image/png")),
    })
}

/// Map an image media type to a file extension
fn extension_for_media_type(media_type: &str) -> &'static str {
    match media_type {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        _ => "png",
    }
}

/// Build a filesystem-friendly slug from a prompt
pub fn slug_from_prompt(prompt: &str) -> String {
    let mut slug = String::new();
    let mut previous_was_dash = true; // suppress leading dashes

    for character in prompt.chars() {
        if slug.len() >= SLUG_LENGTH_MAX {
            break;
        }
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            previous_was_dash = false;
        } else if !previous_was_dash {
            slug.push('-');
            previous_was_dash = true;
        }
    }

    let slug = slug.trim_end_matches('-').to_string();
    if slug.is_empty() { "image".to_string() } else { slug }
}

/// Find a path in `directory` for `stem` + `extension` that doesn't collide
/// with an existing file: stem.png, stem-2.png, stem-3.png, ...
pub fn unique_image_path(directory: &Path, stem: &str, extension: &str) -> PathBuf {
    let candidate = directory.join(format!("{}.{}", stem, extension));
    if !candidate.exists() {
        return candidate;
    }

    for suffix in 2.. {
        let candidate = directory.join(format!("{}-{}.{}", stem, suffix, extension));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("suffix loop always returns")
}

/// Write a decoded image into `directory`, returning the saved path
pub fn save_image(directory: &Path, stem: &str, image: &DecodedImage) -> Result<PathBuf> {
    let path = unique_image_path(directory, stem, image.extension);
    std::fs::write(&path, &image.data)
        .with_context(|| format!("Failed to write image: {}", path.display()))?;
    Ok(path)
}

/// Open a file in the system's default viewer (macOS `open`, Linux `xdg-open`)
pub fn open_in_viewer(path: &Path) -> Result<()> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let status = std::process::Command::new(opener)
        .arg(path)
        .status()
        .with_context(|| format!("Failed to run {}", opener))?;
    if !status.success() {
        anyhow::bail!("{} exited with {}", opener, status);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_maps_media_type_to_extension() {
        // "hi" base64-encoded is aGk=
        let image = decode_base64_image("aGk=", Some("image/jpeg")).unwrap();
        assert_eq!(image.data, b"hi");
        assert_eq!(image.extension, "jpg");
    }

    #[test]
    fn test_decode_without_media_type_defaults_to_png() {
        let image = decode_base64_image("aGk=", None).unwrap();
        assert_eq!(image.data, b"hi");
        assert_eq!(image.extension, "png");
    }

    #[test]
    fn test_decode_rejects_invalid_base64() {
        assert!(decode_base64_image("!!!not-base64!!!", Some("image/png")).is_err());
    }

    #[test]
    fn test_slug_from_prompt_produces_clean_filenames() {
        assert_eq!(
            slug_from_prompt("A sunset over the lake, painted!"),
            "a-sunset-over-the-lake-painted"
        );
        assert_eq!(slug_from_prompt("  éclair &&& café  "), "clair-caf");
        assert_eq!(slug_from_prompt("!!!"), "image");
    }

    #[test]
    fn test_slug_is_truncated_for_long_prompts() {
        let long_prompt = "word ".repeat(50);
        let slug = slug_from_prompt(&long_prompt);
        assert!(slug.len() <= SLUG_LENGTH_MAX);
        assert!(!slug.ends_with('-'));
    }

    #[test]
    fn test_unique_image_path_avoids_collisions() {
        let directory = std::env::temp_dir().join(format!(
            "get-image-test-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("t").len()
        ));
        std::fs::create_dir_all(&directory).unwrap();

        let first = unique_image_path(&directory, "cat", "png");
        assert_eq!(first.file_name().unwrap(), "cat.png");
        std::fs::write(&first, b"x").unwrap();

        let second = unique_image_path(&directory, "cat", "png");
        assert_eq!(second.file_name().unwrap(), "cat-2.png");
        std::fs::write(&second, b"x").unwrap();

        let third = unique_image_path(&directory, "cat", "png");
        assert_eq!(third.file_name().unwrap(), "cat-3.png");

        std::fs::remove_dir_all(&directory).unwrap();
    }
}
