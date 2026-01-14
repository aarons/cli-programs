//! test-review - Analyze test quality using mutation testing and LLM suggestions
//!
//! Supports Rust (cargo-mutants) and Python (mutmut) projects.

mod detector;
mod report;
mod runners;
mod suggestions;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use detector::{detect_project_type, is_tool_installed, ProjectType};
use report::{format_report_json, format_report_terminal, generate_assessment, TestReviewReport};
use runners::run_mutation_testing;
use std::path::PathBuf;
use suggestions::{generate_suggestions, read_source_context};

const EXAMPLES: &str = r#"
EXAMPLES:
    # Run mutation testing on current directory
    test-review

    # Run on a specific project
    test-review /path/to/project

    # Run with LLM suggestions for failing tests
    test-review --suggest

    # Run on specific package (Rust workspace)
    test-review -p my-crate

    # Output as JSON for automation
    test-review --format json

    # Check tool availability without running tests
    test-review check

    # Show recommended tools for project
    test-review info
"#;

#[derive(Parser, Debug)]
#[command(name = "test-review")]
#[command(about = "Analyze test quality using mutation testing and LLM suggestions")]
#[command(version)]
#[command(after_help = EXAMPLES)]
struct Args {
    /// Path to the project directory (defaults to current directory)
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,

    /// Specific package to test (for Rust workspaces)
    #[arg(short, long)]
    package: Option<String>,

    /// Generate LLM suggestions for improving tests
    #[arg(short, long)]
    suggest: bool,

    /// Model preset to use for suggestions
    #[arg(short, long)]
    model: Option<String>,

    /// Output format
    #[arg(short, long, value_enum, default_value = "terminal")]
    format: OutputFormat,

    /// Skip mutation testing, only show project info
    #[arg(long)]
    info_only: bool,

    /// Subcommands
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Clone, ValueEnum)]
enum OutputFormat {
    Terminal,
    Json,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Check if required tools are installed
    Check,
    /// Show project info and recommended tools
    Info,
}

fn print_project_info(project_type: &ProjectType, path: &PathBuf) {
    println!("Project: {}", path.display());
    println!("Type: {}\n", project_type);

    let tools = project_type.testing_tools();

    println!("Recommended Testing Tools:");
    println!("==========================\n");

    if let Some(mut_tool) = &tools.mutation_tool {
        let installed = is_tool_installed(mut_tool.command);
        let status = if installed { "installed" } else { "not installed" };
        println!("Mutation Testing: {} ({})", mut_tool.name, status);
        if !installed {
            println!("  Install: {}", mut_tool.install_command);
        }
        println!("  Run: {}\n", mut_tool.command);
    }

    if let Some(prop_fw) = tools.property_framework {
        println!("Property-Based Testing: {}", prop_fw);
        match project_type {
            ProjectType::Rust => {
                println!("  Add to Cargo.toml: proptest = \"1.4\"");
            }
            ProjectType::Python => {
                println!("  Install: pip install hypothesis");
            }
            _ => {}
        }
        println!();
    }

    if let Some(snap_fw) = tools.snapshot_framework {
        println!("Snapshot Testing: {}", snap_fw);
        match project_type {
            ProjectType::Rust => {
                println!("  Add to Cargo.toml: insta = \"1.40\"");
            }
            ProjectType::Python => {
                println!("  Install: pip install syrupy");
            }
            _ => {}
        }
        println!();
    }
}

fn check_tools(project_type: &ProjectType) -> bool {
    let tools = project_type.testing_tools();
    let mut all_installed = true;

    println!("Checking required tools...\n");

    if let Some(mut_tool) = &tools.mutation_tool {
        let installed = is_tool_installed(mut_tool.command);
        if installed {
            println!("[OK] {} is installed", mut_tool.name);
        } else {
            println!("[MISSING] {} - install with: {}", mut_tool.name, mut_tool.install_command);
            all_installed = false;
        }
    }

    println!();

    if all_installed {
        println!("All required tools are installed!");
    } else {
        println!("Some tools are missing. Install them to use test-review.");
    }

    all_installed
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let project_path = args
        .path
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let project_path = project_path
        .canonicalize()
        .context("Failed to resolve project path")?;

    let project_type = detect_project_type(&project_path);

    // Handle subcommands
    match args.command {
        Some(Commands::Check) => {
            let all_ok = check_tools(&project_type);
            if !all_ok {
                std::process::exit(1);
            }
            return Ok(());
        }
        Some(Commands::Info) => {
            print_project_info(&project_type, &project_path);
            return Ok(());
        }
        None => {}
    }

    // Info only mode
    if args.info_only {
        print_project_info(&project_type, &project_path);
        return Ok(());
    }

    // Validate project type
    if project_type == ProjectType::Unknown {
        anyhow::bail!(
            "Could not detect project type. Supported: Rust (Cargo.toml), Python (pyproject.toml, setup.py, requirements.txt)"
        );
    }

    // Check tools are installed
    let tools = project_type.testing_tools();
    if let Some(ref mut_tool) = tools.mutation_tool {
        if !is_tool_installed(mut_tool.command) {
            anyhow::bail!(
                "{} is not installed. Install with:\n  {}",
                mut_tool.name,
                mut_tool.install_command
            );
        }
    }

    eprintln!("Analyzing {} project at {}", project_type, project_path.display());
    eprintln!();

    // Run mutation testing
    let mutation_results = run_mutation_testing(
        &project_type,
        &project_path,
        args.package.as_deref(),
    )
    .await
    .context("Mutation testing failed")?;

    // Generate assessment
    let assessment = generate_assessment(&mutation_results);

    // Optionally generate LLM suggestions
    let suggestions = if args.suggest && !mutation_results.survivors.is_empty() {
        eprintln!("\nGenerating test suggestions...\n");
        let source_context = read_source_context(&project_path, &mutation_results.survivors, 3);
        match generate_suggestions(
            &project_type,
            &mutation_results,
            source_context.as_deref(),
            args.model.as_deref(),
        )
        .await
        {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("Warning: Failed to generate suggestions: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Build report
    let report = TestReviewReport {
        project_type: project_type.to_string(),
        project_path: project_path.display().to_string(),
        mutation_results: Some(mutation_results),
        suggestions,
        assessment,
    };

    // Output report
    match args.format {
        OutputFormat::Terminal => {
            println!("{}", format_report_terminal(&report));
        }
        OutputFormat::Json => {
            println!("{}", format_report_json(&report));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_current_project() {
        // This project should be detected as Rust
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
        let project_type = detect_project_type(&path);
        assert_eq!(project_type, ProjectType::Rust);
    }
}
