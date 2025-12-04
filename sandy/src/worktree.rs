use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Get the current git repository root
pub fn get_repo_root(path: &Path) -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(path)
        .output()
        .context("Failed to execute git rev-parse")?;

    if !output.status.success() {
        bail!(
            "Not a git repository: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(root))
}

/// Get the repository name from its path
pub fn get_repo_name(repo_path: &Path) -> String {
    repo_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    #[test]
    fn test_get_repo_name_simple() {
        let path = Path::new("/home/user/projects/my-repo");
        assert_eq!(get_repo_name(path), "my-repo");
    }

    #[test]
    fn test_get_repo_name_with_dashes() {
        let path = Path::new("/path/to/my-awesome-project");
        assert_eq!(get_repo_name(path), "my-awesome-project");
    }

    #[test]
    fn test_get_repo_name_with_underscores() {
        let path = Path::new("/path/to/my_project_name");
        assert_eq!(get_repo_name(path), "my_project_name");
    }

    #[test]
    fn test_get_repo_name_single_component() {
        let path = Path::new("/repo");
        assert_eq!(get_repo_name(path), "repo");
    }

    #[test]
    fn test_get_repo_name_root_path() {
        let path = Path::new("/");
        // Root path has no file_name, should return default
        assert_eq!(get_repo_name(path), "repo");
    }

    #[test]
    fn test_get_repo_name_relative_path() {
        let path = Path::new("relative/path/project");
        assert_eq!(get_repo_name(path), "project");
    }

    #[test]
    fn test_get_repo_name_with_spaces() {
        let path = Path::new("/path/to/my project");
        assert_eq!(get_repo_name(path), "my project");
    }

    #[test]
    fn test_get_repo_name_with_dots() {
        let path = Path::new("/path/to/project.rs");
        assert_eq!(get_repo_name(path), "project.rs");
    }

    #[test]
    fn test_get_repo_root_in_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Initialize a git repo
        let status = Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output();

        if status.is_err() {
            // Skip test if git is not available
            return;
        }

        let result = get_repo_root(repo_path);
        assert!(result.is_ok());

        let root = result.unwrap();
        // The result should be the canonical path
        assert!(root.ends_with(temp_dir.path().file_name().unwrap()));
    }

    #[test]
    fn test_get_repo_root_in_subdirectory() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Initialize a git repo
        let status = Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output();

        if status.is_err() {
            return;
        }

        // Create a subdirectory
        let subdir = repo_path.join("src").join("lib");
        std::fs::create_dir_all(&subdir).unwrap();

        let result = get_repo_root(&subdir);
        assert!(result.is_ok());

        let root = result.unwrap();
        assert!(root.ends_with(temp_dir.path().file_name().unwrap()));
    }

    #[test]
    fn test_get_repo_root_not_a_repo() {
        let temp_dir = TempDir::new().unwrap();
        // Don't initialize git

        let result = get_repo_root(temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_get_repo_root_nonexistent_path() {
        let result = get_repo_root(Path::new("/nonexistent/path/12345"));
        assert!(result.is_err());
    }
}
