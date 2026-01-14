//! LLM-powered test suggestions

use crate::detector::ProjectType;
use crate::report::{Priority, SuggestionType, TestSuggestion};
use crate::runners::{MutationResults, SurvivingMutant};
use anyhow::{Context, Result};
use llm_client::{Config, LlmProvider, LlmRequest, get_provider_with_fallback};
use std::path::Path;

const SYSTEM_PROMPT: &str = r#"You are an expert software testing consultant. Your job is to analyze mutation testing results and suggest specific, actionable tests that would catch the surviving mutants.

For each suggestion:
1. Be specific about what to test
2. Provide example test code when possible
3. Focus on the mutation that survived
4. Consider property-based testing for boundary conditions
5. Consider edge cases and error handling

Output your suggestions in the following XML format:

<suggestions>
<suggestion>
<file>path/to/file.rs</file>
<type>new_test|property_test|boundary_test|error_handling|assertion</type>
<priority>high|medium|low</priority>
<description>Clear description of what test to add</description>
<code>
// Example test code here
</code>
</suggestion>
</suggestions>

Only output the XML, no other text."#;

/// Generate test suggestions using LLM
pub async fn generate_suggestions(
    project_type: &ProjectType,
    results: &MutationResults,
    source_context: Option<&str>,
    preset: Option<&str>,
) -> Result<Vec<TestSuggestion>> {
    if results.survivors.is_empty() {
        return Ok(vec![]);
    }

    let config = Config::load()?;
    let preset_name = preset.unwrap_or_else(|| config.get_default_for_program("test-review"));
    let provider = get_provider_with_fallback(&config, preset_name)?;

    let prompt = build_prompt(project_type, results, source_context);

    let request = LlmRequest {
        prompt,
        system_prompt: Some(SYSTEM_PROMPT.to_string()),
        max_tokens: Some(4000),
        temperature: Some(0.3),
        files: vec![],
        json_schema: None,
    };

    let response = provider
        .complete(request)
        .await
        .context("Failed to get LLM suggestions")?;

    parse_suggestions(&response.content)
}

fn build_prompt(
    project_type: &ProjectType,
    results: &MutationResults,
    source_context: Option<&str>,
) -> String {
    let mut prompt = String::new();

    prompt.push_str(&format!(
        "# Mutation Testing Results for {} Project\n\n",
        project_type
    ));

    prompt.push_str(&format!(
        "## Summary\n- Total mutants: {}\n- Killed: {}\n- Survived: {}\n- Score: {:.1}%\n\n",
        results.total_mutants, results.killed, results.survived, results.score
    ));

    prompt.push_str("## Surviving Mutants\n\n");
    prompt.push_str("These mutations were NOT caught by existing tests:\n\n");

    for (i, survivor) in results.survivors.iter().take(20).enumerate() {
        prompt.push_str(&format!(
            "{}. **{}:{}**\n   Mutation: {}\n",
            i + 1,
            survivor.file,
            survivor.line.map(|l| l.to_string()).unwrap_or_default(),
            survivor.description
        ));

        if let Some(ref orig) = survivor.original {
            prompt.push_str(&format!("   Original: `{}`\n", orig));
        }
        if let Some(ref repl) = survivor.replacement {
            prompt.push_str(&format!("   Replaced with: `{}`\n", repl));
        }
        prompt.push('\n');
    }

    if results.survivors.len() > 20 {
        prompt.push_str(&format!(
            "\n... and {} more surviving mutants\n",
            results.survivors.len() - 20
        ));
    }

    if let Some(context) = source_context {
        prompt.push_str("\n## Relevant Source Code\n\n```\n");
        prompt.push_str(context);
        prompt.push_str("\n```\n");
    }

    let framework = match project_type {
        ProjectType::Rust => "proptest for property-based testing, standard #[test] for unit tests",
        ProjectType::Python => "hypothesis for property-based testing, pytest for unit tests",
        ProjectType::Unknown => "appropriate testing frameworks",
    };

    prompt.push_str(&format!(
        "\n## Task\n\nSuggest specific tests that would catch these surviving mutants.\n\
        Use {} as appropriate.\n\
        Focus on the highest-impact tests first.\n\
        Provide example code for each suggestion.\n",
        framework
    ));

    prompt
}

fn parse_suggestions(response: &str) -> Result<Vec<TestSuggestion>> {
    let mut suggestions = Vec::new();

    // Find suggestions section
    let start = response
        .find("<suggestions>")
        .ok_or_else(|| anyhow::anyhow!("No <suggestions> tag found in response"))?;
    let end = response
        .find("</suggestions>")
        .ok_or_else(|| anyhow::anyhow!("No </suggestions> tag found in response"))?;

    let content = &response[start..end + "</suggestions>".len()];

    // Parse individual suggestions
    let mut pos = 0;
    while let Some(sugg_start) = content[pos..].find("<suggestion>") {
        let sugg_start = pos + sugg_start;
        if let Some(sugg_end) = content[sugg_start..].find("</suggestion>") {
            let sugg_end = sugg_start + sugg_end + "</suggestion>".len();
            let sugg_content = &content[sugg_start..sugg_end];

            if let Some(suggestion) = parse_single_suggestion(sugg_content) {
                suggestions.push(suggestion);
            }

            pos = sugg_end;
        } else {
            break;
        }
    }

    // Sort by priority (high first)
    suggestions.sort_by(|a, b| b.priority.cmp(&a.priority));

    Ok(suggestions)
}

fn parse_single_suggestion(content: &str) -> Option<TestSuggestion> {
    let file = extract_tag(content, "file")?;
    let type_str = extract_tag(content, "type").unwrap_or_else(|| "new_test".to_string());
    let priority_str = extract_tag(content, "priority").unwrap_or_else(|| "medium".to_string());
    let description = extract_tag(content, "description")?;
    let code = extract_tag(content, "code");

    let suggestion_type = match type_str.as_str() {
        "property_test" => SuggestionType::PropertyTest,
        "boundary_test" => SuggestionType::BoundaryTest,
        "error_handling" => SuggestionType::ErrorHandling,
        "assertion" => SuggestionType::Assertion,
        _ => SuggestionType::NewTest,
    };

    let priority = match priority_str.as_str() {
        "high" => Priority::High,
        "low" => Priority::Low,
        _ => Priority::Medium,
    };

    Some(TestSuggestion {
        file,
        suggestion_type,
        description,
        example_code: code,
        priority,
    })
}

fn extract_tag(content: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{}>", tag);
    let end_tag = format!("</{}>", tag);

    let start = content.find(&start_tag)? + start_tag.len();
    let end = content[start..].find(&end_tag)?;

    Some(content[start..start + end].trim().to_string())
}

/// Read source file content around specific line numbers
pub fn read_source_context(
    project_path: &Path,
    survivors: &[SurvivingMutant],
    context_lines: usize,
) -> Option<String> {
    use std::collections::HashMap;
    use std::fs;

    let mut file_lines: HashMap<String, Vec<usize>> = HashMap::new();

    // Group line numbers by file
    for survivor in survivors.iter().take(10) {
        if let Some(line) = survivor.line {
            file_lines
                .entry(survivor.file.clone())
                .or_default()
                .push(line);
        }
    }

    let mut context = String::new();

    for (file, lines) in file_lines.iter().take(5) {
        let file_path = project_path.join(file);
        if let Ok(content) = fs::read_to_string(&file_path) {
            let file_lines: Vec<&str> = content.lines().collect();

            context.push_str(&format!("// {}\n", file));

            for &line_num in lines.iter().take(3) {
                let start = line_num.saturating_sub(context_lines + 1);
                let end = (line_num + context_lines).min(file_lines.len());

                for (i, line) in file_lines[start..end].iter().enumerate() {
                    let actual_line = start + i + 1;
                    let marker = if actual_line == line_num { ">>>" } else { "   " };
                    context.push_str(&format!("{} {:4}: {}\n", marker, actual_line, line));
                }
                context.push_str("\n");
            }
        }
    }

    if context.is_empty() {
        None
    } else {
        Some(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tag() {
        let content = "<file>src/main.rs</file>";
        assert_eq!(extract_tag(content, "file"), Some("src/main.rs".to_string()));

        let content = "<description>Test boundary conditions</description>";
        assert_eq!(
            extract_tag(content, "description"),
            Some("Test boundary conditions".to_string())
        );
    }

    #[test]
    fn test_extract_tag_missing() {
        let content = "<file>src/main.rs</file>";
        assert_eq!(extract_tag(content, "other"), None);
    }

    #[test]
    fn test_parse_single_suggestion() {
        let content = r#"
<suggestion>
<file>src/lib.rs</file>
<type>boundary_test</type>
<priority>high</priority>
<description>Test edge case for zero input</description>
<code>
#[test]
fn test_zero_input() {
    assert_eq!(process(0), expected_for_zero);
}
</code>
</suggestion>
"#;

        let suggestion = parse_single_suggestion(content).unwrap();
        assert_eq!(suggestion.file, "src/lib.rs");
        assert!(matches!(suggestion.suggestion_type, SuggestionType::BoundaryTest));
        assert_eq!(suggestion.priority, Priority::High);
        assert!(suggestion.example_code.is_some());
    }

    #[test]
    fn test_parse_suggestions_full() {
        let response = r#"
<suggestions>
<suggestion>
<file>src/lib.rs</file>
<type>new_test</type>
<priority>high</priority>
<description>Add test for comparison</description>
</suggestion>
<suggestion>
<file>src/utils.rs</file>
<type>property_test</type>
<priority>medium</priority>
<description>Add property test</description>
</suggestion>
</suggestions>
"#;

        let suggestions = parse_suggestions(response).unwrap();
        assert_eq!(suggestions.len(), 2);
        // Should be sorted by priority
        assert_eq!(suggestions[0].priority, Priority::High);
        assert_eq!(suggestions[1].priority, Priority::Medium);
    }
}
