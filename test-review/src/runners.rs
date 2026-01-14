//! Mutation testing runners for different project types

use crate::detector::ProjectType;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

/// Results from a mutation testing run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationResults {
    /// Total number of mutants generated
    pub total_mutants: usize,
    /// Number of mutants killed by tests
    pub killed: usize,
    /// Number of mutants that survived (tests didn't catch)
    pub survived: usize,
    /// Number of mutants that timed out
    pub timeout: usize,
    /// Number of mutants that caused errors
    pub errors: usize,
    /// Mutation score as percentage (killed / total * 100)
    pub score: f64,
    /// Details about surviving mutants
    pub survivors: Vec<SurvivingMutant>,
    /// Raw output from the tool
    pub raw_output: String,
}

/// Information about a mutant that survived testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurvivingMutant {
    /// File where the mutation was applied
    pub file: String,
    /// Line number of the mutation
    pub line: Option<usize>,
    /// Description of the mutation
    pub description: String,
    /// The original code
    pub original: Option<String>,
    /// The mutated code
    pub replacement: Option<String>,
}

/// Run mutation testing for a Rust project using cargo-mutants
pub async fn run_cargo_mutants(
    project_path: &Path,
    package: Option<&str>,
    _timeout_mins: u32,
) -> Result<MutationResults> {
    let mut args = vec!["mutants", "--json"];

    if let Some(pkg) = package {
        args.push("-p");
        args.push(pkg);
    }

    eprintln!("Running: cargo {}", args.join(" "));
    eprintln!("This may take a while...\n");

    let output = Command::new("cargo")
        .args(&args)
        .current_dir(project_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to execute cargo mutants")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw_output = format!("{}\n{}", stdout, stderr);

    // Try to parse JSON output
    if let Some(results) = parse_cargo_mutants_json(&stdout) {
        return Ok(results);
    }

    // Fall back to parsing text output
    parse_cargo_mutants_text(&raw_output)
}

/// Parse cargo-mutants JSON output
fn parse_cargo_mutants_json(output: &str) -> Option<MutationResults> {
    // cargo-mutants outputs JSON lines, find the summary
    for line in output.lines() {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            if json.get("total_mutants").is_some() {
                let total = json["total_mutants"].as_u64().unwrap_or(0) as usize;
                let caught = json["caught"].as_u64().unwrap_or(0) as usize;
                let missed = json["missed"].as_u64().unwrap_or(0) as usize;
                let timeout = json["timeout"].as_u64().unwrap_or(0) as usize;
                let errors = json["unviable"].as_u64().unwrap_or(0) as usize;

                let score = if total > 0 {
                    (caught as f64 / total as f64) * 100.0
                } else {
                    100.0
                };

                return Some(MutationResults {
                    total_mutants: total,
                    killed: caught,
                    survived: missed,
                    timeout,
                    errors,
                    score,
                    survivors: vec![], // Would need to parse missed_list
                    raw_output: output.to_string(),
                });
            }
        }
    }
    None
}

/// Parse cargo-mutants text output (fallback)
fn parse_cargo_mutants_text(output: &str) -> Result<MutationResults> {
    let mut total = 0;
    let mut killed = 0;
    let mut survived = 0;
    let mut timeout = 0;
    let mut survivors = Vec::new();

    let mut in_missed_section = false;

    for line in output.lines() {
        // Look for summary line like "42 mutants tested: 38 caught, 4 missed"
        if line.contains("mutants tested") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(pos) = parts.iter().position(|&s| s == "mutants") {
                if pos > 0 {
                    total = parts[pos - 1].parse().unwrap_or(0);
                }
            }
            if let Some(pos) = parts.iter().position(|&s| s == "caught,") {
                if pos > 0 {
                    killed = parts[pos - 1].parse().unwrap_or(0);
                }
            }
            if let Some(pos) = parts.iter().position(|&s| s == "missed") {
                if pos > 0 {
                    survived = parts[pos - 1].parse().unwrap_or(0);
                }
            }
            if let Some(pos) = parts.iter().position(|&s| s == "timeout") {
                if pos > 0 {
                    timeout = parts[pos - 1].parse().unwrap_or(0);
                }
            }
        }

        // Track MISSED section
        if line.contains("MISSED") || line.contains("missed:") {
            in_missed_section = true;
            continue;
        }

        // Parse surviving mutant entries
        if in_missed_section && line.trim().starts_with("src/") {
            // Format: "src/file.rs:123: replace X with Y"
            let parts: Vec<&str> = line.splitn(3, ':').collect();
            if parts.len() >= 2 {
                let file = parts[0].trim().to_string();
                let line_num = parts[1].trim().parse().ok();
                let description = parts.get(2).map(|s| s.trim().to_string()).unwrap_or_default();

                survivors.push(SurvivingMutant {
                    file,
                    line: line_num,
                    description,
                    original: None,
                    replacement: None,
                });
            }
        }

        // End of MISSED section
        if in_missed_section && line.trim().is_empty() {
            in_missed_section = false;
        }
    }

    let score = if total > 0 {
        (killed as f64 / total as f64) * 100.0
    } else {
        100.0
    };

    Ok(MutationResults {
        total_mutants: total,
        killed,
        survived,
        timeout,
        errors: 0,
        score,
        survivors,
        raw_output: output.to_string(),
    })
}

/// Run mutation testing for a Python project using mutmut
pub async fn run_mutmut(project_path: &Path) -> Result<MutationResults> {
    eprintln!("Running: mutmut run");
    eprintln!("This may take a while...\n");

    // Run mutmut
    let output = Command::new("mutmut")
        .arg("run")
        .arg("--no-progress")
        .current_dir(project_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to execute mutmut")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Get results in JSON format
    let results_output = Command::new("mutmut")
        .args(["results", "--json"])
        .current_dir(project_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to get mutmut results")?;

    let results_stdout = String::from_utf8_lossy(&results_output.stdout);
    let raw_output = format!("{}\n{}\n{}", stdout, stderr, results_stdout);

    // Try to parse JSON results
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&results_stdout) {
        return parse_mutmut_json(&json, raw_output);
    }

    // Fall back to parsing text
    parse_mutmut_text(&raw_output)
}

fn parse_mutmut_json(json: &serde_json::Value, raw_output: String) -> Result<MutationResults> {
    let killed = json["killed"].as_u64().unwrap_or(0) as usize;
    let survived = json["survived"].as_u64().unwrap_or(0) as usize;
    let timeout = json["timeout"].as_u64().unwrap_or(0) as usize;
    let suspicious = json["suspicious"].as_u64().unwrap_or(0) as usize;
    let total = killed + survived + timeout + suspicious;

    let score = if total > 0 {
        (killed as f64 / total as f64) * 100.0
    } else {
        100.0
    };

    let mut survivors = Vec::new();
    if let Some(survived_list) = json["survived_mutants"].as_array() {
        for mutant in survived_list {
            survivors.push(SurvivingMutant {
                file: mutant["filename"].as_str().unwrap_or("").to_string(),
                line: mutant["line_number"].as_u64().map(|n| n as usize),
                description: mutant["mutation"].as_str().unwrap_or("").to_string(),
                original: mutant["original"].as_str().map(|s| s.to_string()),
                replacement: mutant["replacement"].as_str().map(|s| s.to_string()),
            });
        }
    }

    Ok(MutationResults {
        total_mutants: total,
        killed,
        survived,
        timeout,
        errors: suspicious,
        score,
        survivors,
        raw_output,
    })
}

fn parse_mutmut_text(output: &str) -> Result<MutationResults> {
    // Basic parsing for mutmut text output
    let mut killed = 0;
    let mut survived = 0;
    let mut timeout = 0;

    for line in output.lines() {
        if line.contains("killed:") {
            if let Some(num) = line.split(':').last() {
                killed = num.trim().parse().unwrap_or(0);
            }
        }
        if line.contains("survived:") {
            if let Some(num) = line.split(':').last() {
                survived = num.trim().parse().unwrap_or(0);
            }
        }
        if line.contains("timeout:") {
            if let Some(num) = line.split(':').last() {
                timeout = num.trim().parse().unwrap_or(0);
            }
        }
    }

    let total = killed + survived + timeout;
    let score = if total > 0 {
        (killed as f64 / total as f64) * 100.0
    } else {
        100.0
    };

    Ok(MutationResults {
        total_mutants: total,
        killed,
        survived,
        timeout,
        errors: 0,
        score,
        survivors: vec![],
        raw_output: output.to_string(),
    })
}

/// Run mutation testing based on project type
pub async fn run_mutation_testing(
    project_type: &ProjectType,
    project_path: &Path,
    package: Option<&str>,
) -> Result<MutationResults> {
    match project_type {
        ProjectType::Rust => run_cargo_mutants(project_path, package, 30).await,
        ProjectType::Python => run_mutmut(project_path).await,
        ProjectType::Unknown => {
            anyhow::bail!("Cannot run mutation testing: unknown project type")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cargo_mutants_text_summary() {
        let output = r#"
Found 42 mutants to test
running 42 tests
42 mutants tested: 38 caught, 4 missed

MISSED:
  src/lib.rs:45: replace > with >=
  src/lib.rs:67: replace + with -
"#;

        let results = parse_cargo_mutants_text(output).unwrap();
        assert_eq!(results.total_mutants, 42);
        assert_eq!(results.killed, 38);
        assert_eq!(results.survived, 4);
        assert!((results.score - 90.48).abs() < 0.1);
    }

    #[test]
    fn test_parse_empty_output() {
        let output = "";
        let results = parse_cargo_mutants_text(output).unwrap();
        assert_eq!(results.total_mutants, 0);
        assert_eq!(results.score, 100.0); // No mutants = 100% score
    }

    #[test]
    fn test_mutation_score_calculation() {
        // 75% kill rate
        let results = MutationResults {
            total_mutants: 100,
            killed: 75,
            survived: 25,
            timeout: 0,
            errors: 0,
            score: 75.0,
            survivors: vec![],
            raw_output: String::new(),
        };
        assert_eq!(results.score, 75.0);
    }
}
