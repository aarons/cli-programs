//! Report generation for test-review

use crate::runners::MutationResults;
use serde::{Deserialize, Serialize};

/// Complete test review report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestReviewReport {
    /// Project type detected
    pub project_type: String,
    /// Path to the project
    pub project_path: String,
    /// Mutation testing results
    pub mutation_results: Option<MutationResults>,
    /// LLM-generated suggestions for improving tests
    pub suggestions: Option<Vec<TestSuggestion>>,
    /// Overall assessment
    pub assessment: Assessment,
}

/// Assessment of test quality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assessment {
    /// Overall grade (A, B, C, D, F)
    pub grade: char,
    /// Summary of findings
    pub summary: String,
    /// Key areas for improvement
    pub improvements: Vec<String>,
}

/// A suggestion for improving tests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSuggestion {
    /// File to add/modify tests for
    pub file: String,
    /// Type of suggestion (new_test, property_test, assertion)
    pub suggestion_type: SuggestionType,
    /// Description of what to test
    pub description: String,
    /// Example test code if available
    pub example_code: Option<String>,
    /// Priority (high, medium, low)
    pub priority: Priority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionType {
    NewTest,
    PropertyTest,
    BoundaryTest,
    ErrorHandling,
    Assertion,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    Medium,
    High,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Priority::High => write!(f, "high"),
            Priority::Medium => write!(f, "medium"),
            Priority::Low => write!(f, "low"),
        }
    }
}

impl std::fmt::Display for SuggestionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SuggestionType::NewTest => write!(f, "New Test"),
            SuggestionType::PropertyTest => write!(f, "Property-Based Test"),
            SuggestionType::BoundaryTest => write!(f, "Boundary Test"),
            SuggestionType::ErrorHandling => write!(f, "Error Handling Test"),
            SuggestionType::Assertion => write!(f, "Additional Assertion"),
        }
    }
}

/// Calculate grade from mutation score
pub fn calculate_grade(mutation_score: f64) -> char {
    match mutation_score as u32 {
        90..=100 => 'A',
        80..=89 => 'B',
        70..=79 => 'C',
        60..=69 => 'D',
        _ => 'F',
    }
}

/// Generate assessment from mutation results
pub fn generate_assessment(results: &MutationResults) -> Assessment {
    let grade = calculate_grade(results.score);

    let summary = match grade {
        'A' => format!(
            "Excellent test coverage! {:.1}% of mutations were caught by tests.",
            results.score
        ),
        'B' => format!(
            "Good test coverage with {:.1}% mutation score. Some edge cases may be missing.",
            results.score
        ),
        'C' => format!(
            "Moderate test coverage at {:.1}%. Tests catch most obvious bugs but miss many edge cases.",
            results.score
        ),
        'D' => format!(
            "Below average test coverage at {:.1}%. Many bugs could slip through undetected.",
            results.score
        ),
        _ => format!(
            "Poor test coverage at {:.1}%. Tests are not effectively validating the code.",
            results.score
        ),
    };

    let mut improvements = Vec::new();

    if results.survived > 0 {
        improvements.push(format!(
            "{} mutations survived - add tests for these code paths",
            results.survived
        ));
    }

    if results.timeout > 0 {
        improvements.push(format!(
            "{} mutations timed out - consider optimizing test performance",
            results.timeout
        ));
    }

    // Analyze patterns in surviving mutants
    let mut operator_mutations = 0;
    let mut boundary_mutations = 0;

    for survivor in &results.survivors {
        let desc = survivor.description.to_lowercase();
        if desc.contains("replace") && (desc.contains(">") || desc.contains("<") || desc.contains("==")) {
            operator_mutations += 1;
        }
        if desc.contains("boundary") || desc.contains("0") || desc.contains("1") {
            boundary_mutations += 1;
        }
    }

    if operator_mutations > 0 {
        improvements.push(format!(
            "{} comparison operator mutations survived - add boundary condition tests",
            operator_mutations
        ));
    }

    if boundary_mutations > 0 {
        improvements.push("Consider property-based testing to catch boundary conditions".to_string());
    }

    Assessment {
        grade,
        summary,
        improvements,
    }
}

/// Format report for terminal output
pub fn format_report_terminal(report: &TestReviewReport) -> String {
    let mut output = String::new();

    output.push_str(&format!("\n=== Test Review Report ===\n"));
    output.push_str(&format!("Project: {} ({})\n\n", report.project_path, report.project_type));

    // Mutation results
    if let Some(ref results) = report.mutation_results {
        output.push_str("## Mutation Testing Results\n\n");
        output.push_str(&format!("  Total mutants:  {}\n", results.total_mutants));
        output.push_str(&format!("  Killed:         {} ({:.1}%)\n", results.killed,
            if results.total_mutants > 0 { results.killed as f64 / results.total_mutants as f64 * 100.0 } else { 0.0 }));
        output.push_str(&format!("  Survived:       {}\n", results.survived));
        if results.timeout > 0 {
            output.push_str(&format!("  Timeout:        {}\n", results.timeout));
        }
        output.push_str(&format!("\n  Mutation Score: {:.1}%\n", results.score));

        if !results.survivors.is_empty() {
            output.push_str("\n### Surviving Mutants\n\n");
            for (i, survivor) in results.survivors.iter().take(10).enumerate() {
                output.push_str(&format!("  {}. {}:{}\n",
                    i + 1,
                    survivor.file,
                    survivor.line.map(|l| l.to_string()).unwrap_or_default()
                ));
                if !survivor.description.is_empty() {
                    output.push_str(&format!("     {}\n", survivor.description));
                }
            }
            if results.survivors.len() > 10 {
                output.push_str(&format!("\n  ... and {} more\n", results.survivors.len() - 10));
            }
        }
    }

    // Assessment
    output.push_str(&format!("\n## Assessment: Grade {}\n\n", report.assessment.grade));
    output.push_str(&format!("  {}\n", report.assessment.summary));

    if !report.assessment.improvements.is_empty() {
        output.push_str("\n### Recommended Improvements\n\n");
        for improvement in &report.assessment.improvements {
            output.push_str(&format!("  - {}\n", improvement));
        }
    }

    // Suggestions
    if let Some(ref suggestions) = report.suggestions {
        if !suggestions.is_empty() {
            output.push_str("\n## Test Suggestions\n\n");
            for (i, suggestion) in suggestions.iter().enumerate() {
                output.push_str(&format!("  {}. [{}] {} - {}\n",
                    i + 1,
                    suggestion.priority,
                    suggestion.suggestion_type,
                    suggestion.file
                ));
                output.push_str(&format!("     {}\n", suggestion.description));
                if let Some(ref code) = suggestion.example_code {
                    output.push_str(&format!("\n     ```\n{}\n     ```\n\n",
                        code.lines().map(|l| format!("     {}", l)).collect::<Vec<_>>().join("\n")
                    ));
                }
            }
        }
    }

    output
}

/// Format report as JSON
pub fn format_report_json(report: &TestReviewReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grade_calculation() {
        assert_eq!(calculate_grade(95.0), 'A');
        assert_eq!(calculate_grade(90.0), 'A');
        assert_eq!(calculate_grade(85.0), 'B');
        assert_eq!(calculate_grade(75.0), 'C');
        assert_eq!(calculate_grade(65.0), 'D');
        assert_eq!(calculate_grade(50.0), 'F');
    }

    #[test]
    fn test_assessment_generation() {
        let results = MutationResults {
            total_mutants: 100,
            killed: 85,
            survived: 15,
            timeout: 0,
            errors: 0,
            score: 85.0,
            survivors: vec![],
            raw_output: String::new(),
        };

        let assessment = generate_assessment(&results);
        assert_eq!(assessment.grade, 'B');
        assert!(assessment.summary.contains("85.0%"));
        assert!(!assessment.improvements.is_empty());
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::High > Priority::Medium);
        assert!(Priority::Medium > Priority::Low);
    }
}
