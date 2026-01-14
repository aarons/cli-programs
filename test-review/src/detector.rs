//! Project type detection for test-review
//!
//! Detects project type based on manifest files and recommends appropriate testing tools.

use std::path::Path;

/// Supported project types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectType {
    Rust,
    Python,
    Unknown,
}

impl std::fmt::Display for ProjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectType::Rust => write!(f, "Rust"),
            ProjectType::Python => write!(f, "Python"),
            ProjectType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Information about detected testing tools
#[derive(Debug, Clone)]
pub struct TestingTools {
    /// Mutation testing tool command
    pub mutation_tool: Option<MutationTool>,
    /// Property-based testing framework
    pub property_framework: Option<&'static str>,
    /// Snapshot testing framework
    pub snapshot_framework: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub struct MutationTool {
    pub name: &'static str,
    pub command: &'static str,
    pub install_command: &'static str,
}

impl ProjectType {
    /// Get recommended testing tools for this project type
    pub fn testing_tools(&self) -> TestingTools {
        match self {
            ProjectType::Rust => TestingTools {
                mutation_tool: Some(MutationTool {
                    name: "cargo-mutants",
                    command: "cargo mutants",
                    install_command: "cargo install cargo-mutants",
                }),
                property_framework: Some("proptest"),
                snapshot_framework: Some("insta"),
            },
            ProjectType::Python => TestingTools {
                mutation_tool: Some(MutationTool {
                    name: "mutmut",
                    command: "mutmut run",
                    install_command: "pip install mutmut",
                }),
                property_framework: Some("hypothesis"),
                snapshot_framework: Some("syrupy"),
            },
            ProjectType::Unknown => TestingTools {
                mutation_tool: None,
                property_framework: None,
                snapshot_framework: None,
            },
        }
    }
}

/// Detect project type from the given directory
pub fn detect_project_type(path: &Path) -> ProjectType {
    // Check for Rust project
    if path.join("Cargo.toml").exists() {
        return ProjectType::Rust;
    }

    // Check for Python project (various manifest files)
    if path.join("pyproject.toml").exists()
        || path.join("setup.py").exists()
        || path.join("setup.cfg").exists()
        || path.join("requirements.txt").exists()
    {
        return ProjectType::Python;
    }

    // Check for Python files in directory
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension() {
                if ext == "py" {
                    return ProjectType::Python;
                }
            }
        }
    }

    ProjectType::Unknown
}

/// Check if a tool is installed and available on PATH
pub fn is_tool_installed(command: &str) -> bool {
    // Extract the base command (first word)
    let base_cmd = command.split_whitespace().next().unwrap_or(command);

    std::process::Command::new("which")
        .arg(base_cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_rust_project() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

        assert_eq!(detect_project_type(temp_dir.path()), ProjectType::Rust);
    }

    #[test]
    fn test_detect_python_pyproject() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("pyproject.toml"), "[project]\nname = \"test\"").unwrap();

        assert_eq!(detect_project_type(temp_dir.path()), ProjectType::Python);
    }

    #[test]
    fn test_detect_python_requirements() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("requirements.txt"), "requests==2.28.0").unwrap();

        assert_eq!(detect_project_type(temp_dir.path()), ProjectType::Python);
    }

    #[test]
    fn test_detect_python_files() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("main.py"), "print('hello')").unwrap();

        assert_eq!(detect_project_type(temp_dir.path()), ProjectType::Python);
    }

    #[test]
    fn test_detect_unknown() {
        let temp_dir = TempDir::new().unwrap();
        // Empty directory

        assert_eq!(detect_project_type(temp_dir.path()), ProjectType::Unknown);
    }

    #[test]
    fn test_rust_tools() {
        let tools = ProjectType::Rust.testing_tools();
        assert!(tools.mutation_tool.is_some());
        assert_eq!(tools.mutation_tool.unwrap().name, "cargo-mutants");
        assert_eq!(tools.property_framework, Some("proptest"));
        assert_eq!(tools.snapshot_framework, Some("insta"));
    }

    #[test]
    fn test_python_tools() {
        let tools = ProjectType::Python.testing_tools();
        assert!(tools.mutation_tool.is_some());
        assert_eq!(tools.mutation_tool.unwrap().name, "mutmut");
        assert_eq!(tools.property_framework, Some("hypothesis"));
        assert_eq!(tools.snapshot_framework, Some("syrupy"));
    }
}
