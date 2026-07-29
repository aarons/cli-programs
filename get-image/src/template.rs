use anyhow::Result;

/// Ceiling on prompts a single template may expand to — a courtesy guard
/// against accidental large bills, in the spirit of the count limit.
const VARIANTS_MAX: usize = 16;

/// A parsed piece of a template: literal text, or a `[a|b|c]` choice group
enum Segment {
    Literal(String),
    Choices(Vec<String>),
}

/// Expand `[option|option]` groups in a prompt into every combination.
///
/// "a [red|blue] [cat|dog]" expands to four prompts. Brackets without a `|`
/// inside are kept as literal text, so "[sic]" survives untouched. Groups
/// don't nest. A prompt without groups expands to itself.
pub fn expand_template(prompt: &str) -> Result<Vec<String>> {
    let segments = parse_segments(prompt);

    let mut variant_count: usize = 1;
    for segment in &segments {
        if let Segment::Choices(options) = segment {
            variant_count = variant_count.saturating_mul(options.len());
        }
    }
    if variant_count > VARIANTS_MAX {
        anyhow::bail!(
            "Template expands to {} prompts; the limit is {}",
            variant_count,
            VARIANTS_MAX
        );
    }

    let mut variants = vec![String::new()];
    for segment in &segments {
        match segment {
            Segment::Literal(text) => {
                for variant in &mut variants {
                    variant.push_str(text);
                }
            }
            Segment::Choices(options) => {
                variants = variants
                    .iter()
                    .flat_map(|variant| {
                        options
                            .iter()
                            .map(move |option| format!("{variant}{option}"))
                    })
                    .collect();
            }
        }
    }
    Ok(variants)
}

/// Split a prompt into literal text and choice groups
fn parse_segments(prompt: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut remaining = prompt;

    while let Some(open) = remaining.find('[') {
        let after_open = &remaining[open + 1..];
        match after_open.find(']') {
            Some(close) if after_open[..close].contains('|') => {
                if open > 0 {
                    segments.push(Segment::Literal(remaining[..open].to_string()));
                }
                let options = after_open[..close].split('|').map(str::to_string).collect();
                segments.push(Segment::Choices(options));
                remaining = &after_open[close + 1..];
            }
            _ => {
                // No closing bracket, or no alternatives: the bracket is literal
                segments.push(Segment::Literal(remaining[..=open].to_string()));
                remaining = after_open;
            }
        }
    }
    if !remaining.is_empty() {
        segments.push(Segment::Literal(remaining.to_string()));
    }
    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_without_groups_expands_to_itself() {
        assert_eq!(
            expand_template("a quiet harbor at dawn").unwrap(),
            vec!["a quiet harbor at dawn"]
        );
    }

    #[test]
    fn test_groups_expand_to_every_combination() {
        assert_eq!(
            expand_template("a [red|blue] [cat|dog] at home").unwrap(),
            vec![
                "a red cat at home",
                "a red dog at home",
                "a blue cat at home",
                "a blue dog at home",
            ]
        );
    }

    #[test]
    fn test_brackets_without_alternatives_stay_literal() {
        assert_eq!(
            expand_template("a [large] painting [sic").unwrap(),
            vec!["a [large] painting [sic"]
        );
    }

    #[test]
    fn test_empty_option_drops_the_group_text() {
        assert_eq!(
            expand_template("a [very |]large cat").unwrap(),
            vec!["a very large cat", "a large cat"]
        );
    }

    #[test]
    fn test_expansion_beyond_the_limit_is_rejected() {
        // 5 * 5 = 25 combinations, above VARIANTS_MAX
        let error = expand_template("[a|b|c|d|e] [1|2|3|4|5]").unwrap_err();
        assert!(error.to_string().contains("25"));
    }
}
