use anyhow::Result;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use std::io::Write;

use crate::output::DecodedImage;

/// Base64 characters per escape sequence in the kitty graphics protocol
const KITTY_CHUNK_SIZE: usize = 4096;

/// An inline-image escape-sequence protocol a terminal emulator understands
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineProtocol {
    /// iTerm2 inline images (OSC 1337), also spoken by WezTerm
    Iterm,
    /// Kitty graphics protocol, also spoken by Ghostty
    Kitty,
}

/// Detect which inline-image protocol the current terminal speaks, if any
pub fn detect_protocol() -> Option<InlineProtocol> {
    protocol_for_environment(&|name| std::env::var(name).ok())
}

/// Detection against an environment lookup, separated for testability
fn protocol_for_environment(get: &dyn Fn(&str) -> Option<String>) -> Option<InlineProtocol> {
    let variable = |name: &str| get(name).unwrap_or_default();

    // tmux and screen don't pass image escape sequences through by default
    let term = variable("TERM");
    if get("TMUX").is_some() || term.starts_with("screen") || term.starts_with("tmux") {
        return None;
    }

    let term_program = variable("TERM_PROGRAM");
    // LC_TERMINAL survives ssh from iTerm2, where TERM_PROGRAM does not
    if term_program == "iTerm.app"
        || term_program == "WezTerm"
        || variable("LC_TERMINAL") == "iTerm2"
    {
        return Some(InlineProtocol::Iterm);
    }
    if term.contains("kitty")
        || term.contains("ghostty")
        || term_program == "ghostty"
        || get("KITTY_WINDOW_ID").is_some()
    {
        return Some(InlineProtocol::Kitty);
    }
    None
}

/// Render an image inline in the terminal using `protocol`
pub fn display_inline(protocol: InlineProtocol, image: &DecodedImage) -> Result<()> {
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    match protocol {
        InlineProtocol::Iterm => write_iterm(&mut writer, image)?,
        InlineProtocol::Kitty => write_kitty(&mut writer, image)?,
    }
    writer.flush()?;
    Ok(())
}

/// iTerm2 OSC 1337 inline image: a single sequence carrying the whole file.
/// The terminal scales images wider than the pane down to fit.
fn write_iterm(writer: &mut impl Write, image: &DecodedImage) -> std::io::Result<()> {
    writeln!(
        writer,
        "\x1b]1337;File=inline=1;size={};preserveAspectRatio=1:{}\x07",
        image.data.len(),
        BASE64.encode(&image.data),
    )
}

/// Kitty graphics protocol: transmit-and-display (a=T) in base64 chunks,
/// m=1 on every chunk but the last. f=100 carries PNG data only, so other
/// formats are skipped; q=2 suppresses terminal responses we don't read.
fn write_kitty(writer: &mut impl Write, image: &DecodedImage) -> std::io::Result<()> {
    if image.extension != "png" {
        return Ok(());
    }

    let encoded = BASE64.encode(&image.data);
    let mut chunks = encoded.as_bytes().chunks(KITTY_CHUNK_SIZE).peekable();
    let mut is_first_chunk = true;
    while let Some(chunk) = chunks.next() {
        let more = if chunks.peek().is_some() { 1 } else { 0 };
        if is_first_chunk {
            write!(writer, "\x1b_Ga=T,f=100,q=2,m={};", more)?;
            is_first_chunk = false;
        } else {
            write!(writer, "\x1b_Gm={};", more)?;
        }
        writer.write_all(chunk)?;
        write!(writer, "\x1b\\")?;
    }
    writeln!(writer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn protocol_for(pairs: &[(&str, &str)]) -> Option<InlineProtocol> {
        let environment: HashMap<String, String> = pairs
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();
        protocol_for_environment(&move |name| environment.get(name).cloned())
    }

    #[test]
    fn test_detection_recognizes_terminals_by_protocol_family() {
        assert_eq!(
            protocol_for(&[("TERM_PROGRAM", "iTerm.app")]),
            Some(InlineProtocol::Iterm)
        );
        assert_eq!(
            protocol_for(&[("LC_TERMINAL", "iTerm2")]),
            Some(InlineProtocol::Iterm)
        );
        assert_eq!(
            protocol_for(&[("TERM", "xterm-kitty")]),
            Some(InlineProtocol::Kitty)
        );
        assert_eq!(
            protocol_for(&[("TERM", "xterm-ghostty")]),
            Some(InlineProtocol::Kitty)
        );
        assert_eq!(protocol_for(&[("TERM", "xterm-256color")]), None);
    }

    #[test]
    fn test_detection_declines_inside_tmux_and_screen() {
        assert_eq!(
            protocol_for(&[("TERM_PROGRAM", "iTerm.app"), ("TMUX", "/tmp/tmux-1")]),
            None
        );
        assert_eq!(
            protocol_for(&[("TERM", "screen-256color"), ("KITTY_WINDOW_ID", "1")]),
            None
        );
    }

    #[test]
    fn test_iterm_sequence_carries_size_and_base64_payload() {
        let image = DecodedImage {
            data: b"hi".to_vec(),
            extension: "png",
        };
        let mut sequence = Vec::new();
        write_iterm(&mut sequence, &image).unwrap();
        assert_eq!(
            String::from_utf8(sequence).unwrap(),
            "\x1b]1337;File=inline=1;size=2;preserveAspectRatio=1:aGk=\x07\n"
        );
    }

    #[test]
    fn test_kitty_sequence_chunks_with_continuation_flags() {
        // Enough data that the base64 spans two chunks
        let image = DecodedImage {
            data: vec![0u8; KITTY_CHUNK_SIZE],
            extension: "png",
        };
        let mut sequence = Vec::new();
        write_kitty(&mut sequence, &image).unwrap();
        let text = String::from_utf8(sequence).unwrap();
        assert!(text.starts_with("\x1b_Ga=T,f=100,q=2,m=1;"));
        assert!(text.contains("\x1b\\\x1b_Gm=0;"));
        assert!(text.ends_with("\x1b\\\n"));
    }

    #[test]
    fn test_kitty_skips_formats_it_cannot_transmit() {
        let image = DecodedImage {
            data: b"hi".to_vec(),
            extension: "jpg",
        };
        let mut sequence = Vec::new();
        write_kitty(&mut sequence, &image).unwrap();
        assert!(sequence.is_empty());
    }
}
