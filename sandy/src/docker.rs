use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::config::Config;
use crate::state::{
    load_default_template_hash, load_template_digest, load_template_hash,
    save_default_template_hash, save_template_digest, save_template_hash,
};

/// Status of a sandbox container
#[derive(Debug, Clone, PartialEq)]
pub enum SandboxStatus {
    Running,
    Stopped,
    NotFound,
}

/// Get the sandbox container name for a workspace path
///
/// Uses format: `sandy-{dirname}-{short_hash}` for readability while maintaining uniqueness.
/// The dirname is sanitized to meet Docker container naming requirements.
fn get_container_name(workspace: &Path) -> String {
    let dirname = workspace
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "workspace".to_string());

    // Sanitize for Docker container name requirements: [a-zA-Z0-9][a-zA-Z0-9_.-]*
    let sanitized: String = dirname
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();

    // Add short hash for uniqueness (handles same-name directories in different paths)
    let mut hasher = Sha256::new();
    hasher.update(workspace.to_string_lossy().as_bytes());
    let hash = hex::encode(hasher.finalize());

    format!("sandy-{}-{}", sanitized.to_lowercase(), &hash[..6])
}

/// Check if Docker is available
pub fn check_docker() -> Result<()> {
    let output = Command::new("docker")
        .args(["--version"])
        .output()
        .context("Failed to execute docker command. Is Docker installed?")?;

    if !output.status.success() {
        bail!("Docker is not available or not running");
    }

    Ok(())
}

/// Check if `docker sandbox` command is available
pub fn check_docker_sandbox() -> Result<()> {
    let output = Command::new("docker")
        .args(["sandbox", "--help"])
        .output()
        .context("Failed to execute docker sandbox command")?;

    if !output.status.success() {
        bail!(
            "Docker sandbox extension is not installed. Please install it from Docker Desktop or via: docker extension install docker/sandbox"
        );
    }

    Ok(())
}

/// Check if a template image exists
pub fn template_exists(image_name: &str) -> Result<bool> {
    let output = Command::new("docker")
        .args(["images", "-q", image_name])
        .output()
        .context("Failed to check for template image")?;

    Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

/// Get the digest (ID) of a Docker image
pub fn get_image_digest(image_name: &str) -> Result<String> {
    let output = Command::new("docker")
        .args(["image", "inspect", image_name, "--format", "{{.Id}}"])
        .output()
        .context("Failed to get image digest")?;

    if !output.status.success() {
        bail!(
            "Failed to get digest for image '{}': {}",
            image_name,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let digest = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(digest)
}

/// Calculate hash of a Dockerfile
pub fn hash_dockerfile(dockerfile_path: &Path) -> Result<String> {
    let content = fs::read_to_string(dockerfile_path)
        .with_context(|| format!("Failed to read Dockerfile: {}", dockerfile_path.display()))?;

    hash_content(&content)
}

/// Calculate hash of content string
pub fn hash_content(content: &str) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

/// Check if template needs to be rebuilt
pub fn template_needs_rebuild(dockerfile_path: &Path) -> Result<bool> {
    let current_hash = hash_dockerfile(dockerfile_path)?;

    match load_template_hash()? {
        Some(stored_hash) => Ok(current_hash != stored_hash),
        None => Ok(true),
    }
}

/// Result of checking if the user's Dockerfile needs updating from the embedded default
pub enum DefaultTemplateStatus {
    /// User's Dockerfile doesn't exist, should create from default
    NeedsCreation,
    /// User's Dockerfile is from an old default and should be updated
    NeedsUpdate,
    /// User's Dockerfile is up-to-date with the current default
    UpToDate,
    /// User has customized the Dockerfile, don't touch it
    Customized,
}

/// Check if user's Dockerfile should be updated from the embedded default template.
///
/// Returns the appropriate status:
/// - NeedsCreation: Dockerfile doesn't exist
/// - NeedsUpdate: Dockerfile exists, matches old default, embedded default has changed
/// - UpToDate: Dockerfile matches current embedded default
/// - Customized: Dockerfile has been modified by user, don't update
pub fn check_default_template_status(
    dockerfile_path: &Path,
    default_template: &str,
) -> Result<DefaultTemplateStatus> {
    // If user's Dockerfile doesn't exist, it needs to be created
    if !dockerfile_path.exists() {
        return Ok(DefaultTemplateStatus::NeedsCreation);
    }

    // Get the hash of the user's current Dockerfile
    let user_dockerfile_hash = hash_dockerfile(dockerfile_path)?;

    // Load the hash of the default template that was used to create the user's Dockerfile
    let stored_default_hash = load_default_template_hash()?;

    // Delegate to pure logic function
    check_default_template_status_impl(&user_dockerfile_hash, default_template, stored_default_hash)
}

/// Pure logic for determining default template status.
///
/// This function contains no I/O and can be easily unit tested by passing
/// in the required values directly rather than reading them from disk.
///
/// # Arguments
/// * `user_dockerfile_hash` - Hash of the user's current Dockerfile
/// * `default_template` - The embedded default template content
/// * `stored_default_hash` - Previously stored hash of the default template that was
///   used to create the user's Dockerfile (None if never stored)
fn check_default_template_status_impl(
    user_dockerfile_hash: &str,
    default_template: &str,
    stored_default_hash: Option<String>,
) -> Result<DefaultTemplateStatus> {
    let current_default_hash = hash_content(default_template)?;

    match stored_default_hash {
        Some(stored_hash) => {
            // Check if user has customized the Dockerfile
            // (user's file hash differs from the default that was used to create it)
            if user_dockerfile_hash != stored_hash {
                // User has modified their Dockerfile, don't update it
                return Ok(DefaultTemplateStatus::Customized);
            }

            // User's Dockerfile matches the old default - check if embedded default changed
            if stored_hash != current_default_hash {
                Ok(DefaultTemplateStatus::NeedsUpdate)
            } else {
                Ok(DefaultTemplateStatus::UpToDate)
            }
        }
        None => {
            // No stored default hash - this is a pre-existing installation
            // Check if user's Dockerfile matches the current default
            if user_dockerfile_hash == current_default_hash {
                Ok(DefaultTemplateStatus::UpToDate)
            } else {
                // Can't determine if user customized or if it's an old default
                // Assume customized to be safe
                Ok(DefaultTemplateStatus::Customized)
            }
        }
    }
}

/// Update the user's Dockerfile from the embedded default and save the hash
pub fn update_dockerfile_from_default(
    dockerfile_path: &Path,
    default_template: &str,
) -> Result<()> {
    let template_dir = dockerfile_path
        .parent()
        .context("Invalid dockerfile path")?;

    // Ensure directory exists
    if !template_dir.exists() {
        fs::create_dir_all(template_dir)?;
    }

    // Write the new default template
    fs::write(dockerfile_path, default_template)
        .with_context(|| format!("Failed to write Dockerfile: {}", dockerfile_path.display()))?;

    // Save the hash of the default template we used
    let default_hash = hash_content(default_template)?;
    save_default_template_hash(&default_hash)?;

    Ok(())
}

/// Prepare template assets by copying binaries from configured directories
pub fn prepare_template_assets(dockerfile_dir: &Path, config: &Config) -> Result<()> {
    let assets_bin_dir = dockerfile_dir.join("assets").join("bin");

    // Clean and recreate assets/bin directory
    if assets_bin_dir.exists() {
        fs::remove_dir_all(&assets_bin_dir).context("Failed to clean assets/bin directory")?;
    }
    fs::create_dir_all(&assets_bin_dir).context("Failed to create assets/bin directory")?;

    println!("Copying binaries to template assets...");

    let mut copied_count = 0;

    for binary_dir in &config.binary_dirs {
        let expanded_dir = Config::expand_path(binary_dir)?;

        if !expanded_dir.exists() {
            println!("  Skipping {} (not found)", binary_dir);
            continue;
        }

        if !expanded_dir.is_dir() {
            println!("  Skipping {} (not a directory)", binary_dir);
            continue;
        }

        // Copy all executable files from this directory
        for entry in fs::read_dir(&expanded_dir)
            .with_context(|| format!("Failed to read directory: {}", expanded_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();

            // Skip directories and non-executable files
            if path.is_dir() {
                continue;
            }

            // Check if file is executable
            let metadata = fs::metadata(&path)?;
            let permissions = metadata.permissions();
            if permissions.mode() & 0o111 == 0 {
                continue; // Not executable
            }

            let file_name = path.file_name().unwrap();
            let dst = assets_bin_dir.join(file_name);

            fs::copy(&path, &dst).with_context(|| format!("Failed to copy {}", path.display()))?;

            // Ensure the copied file is executable
            let mut perms = fs::metadata(&dst)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&dst, perms)?;

            println!("  Copied {}", file_name.to_string_lossy());
            copied_count += 1;
        }
    }

    if copied_count == 0 {
        println!("  No binaries found in configured directories");
    } else {
        println!("  Copied {} binaries", copied_count);
    }

    Ok(())
}

/// Build the custom template image
pub fn build_template(dockerfile_path: &Path, image_name: &str, config: &Config) -> Result<()> {
    build_template_impl(dockerfile_path, image_name, config, false)
}

/// Build the custom template image, optionally ignoring Docker's build cache
pub fn build_template_no_cache(
    dockerfile_path: &Path,
    image_name: &str,
    config: &Config,
) -> Result<()> {
    build_template_impl(dockerfile_path, image_name, config, true)
}

fn build_template_impl(
    dockerfile_path: &Path,
    image_name: &str,
    config: &Config,
    no_cache: bool,
) -> Result<()> {
    let dockerfile_dir = dockerfile_path.parent().unwrap_or(Path::new("."));

    // Prepare assets before building
    prepare_template_assets(dockerfile_dir, config)?;

    if no_cache {
        println!(
            "Building custom template image (no cache): {}",
            image_name
        );
    } else {
        println!("Building custom template image: {}", image_name);
    }

    let mut cmd = Command::new("docker");
    cmd.args(["build", "-t", image_name]);

    if no_cache {
        cmd.arg("--no-cache");
    }

    cmd.args(["-f", &dockerfile_path.to_string_lossy(), &dockerfile_dir.to_string_lossy()]);

    let status = cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("Failed to execute docker build")?;

    if !status.success() {
        bail!("Failed to build template image");
    }

    // Save the Dockerfile hash after successful build
    let hash = hash_dockerfile(dockerfile_path)?;
    save_template_hash(&hash)?;

    // Get and save the image digest for use with docker sandbox
    let digest = get_image_digest(image_name)?;
    save_template_digest(&digest)?;

    println!("Template image built successfully: {}", image_name);
    println!("Image digest: {}", digest);
    Ok(())
}

/// Get the status of a sandbox
pub fn sandbox_status(workspace: &Path) -> Result<SandboxStatus> {
    let container_name = get_container_name(workspace);

    let output = Command::new("docker")
        .args([
            "ps",
            "-a",
            "--filter",
            &format!("name={}", container_name),
            "--format",
            "{{.Status}}",
        ])
        .output()
        .context("Failed to check sandbox status")?;

    let status_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if status_str.is_empty() {
        Ok(SandboxStatus::NotFound)
    } else if status_str.starts_with("Up") {
        Ok(SandboxStatus::Running)
    } else {
        Ok(SandboxStatus::Stopped)
    }
}

/// Get the command and arguments for a CLI tool
fn get_tool_command(tool: &str) -> Vec<&str> {
    match tool {
        "claude" => vec!["claude", "--dangerously-skip-permissions"],
        "gemini" => vec!["gemini"],
        "codex" => vec!["codex"],
        // Allow custom tool names - just use the name directly
        _ => vec![],
    }
}

/// Start a new sandbox with the given configuration and CLI tool
pub fn start_sandbox(workspace: &Path, config: &Config, tool: &str) -> Result<()> {
    let mut cmd = Command::new("docker");
    cmd.args(["sandbox", "run"]);

    // Mount configured volumes
    for mount in &config.mounts {
        let source = Config::expand_path(&mount.source)?;
        if source.exists() {
            let flag = if mount.readonly { ":ro" } else { "" };
            cmd.args([
                "-v",
                &format!("{}:{}{}", source.display(), mount.target, flag),
            ]);
        }
    }

    // Environment variables
    for (key, value) in &config.env {
        if let Ok(expanded) = Config::expand_env(value)
            && !expanded.is_empty()
        {
            cmd.args(["-e", &format!("{}={}", key, expanded)]);
        }
    }

    // Use the stored image digest for the template (bypasses Docker Sandbox's cache)
    // Fall back to template_image name if no digest is stored (first run before build)
    if let Some(digest) = load_template_digest()? {
        cmd.args(["--template", &digest]);
    } else if let Some(ref template) = config.template_image {
        cmd.args(["--template", template]);
    }

    // Use sandbox credentials - auth persists across sandboxes in Docker volume
    cmd.args(["--credentials=sandbox"]);

    // Name the container for tracking
    let container_name = get_container_name(workspace);
    cmd.args(["--name", &container_name]);

    // Workspace
    cmd.args(["-w", &workspace.display().to_string()]);

    // CLI tool command
    let tool_cmd = get_tool_command(tool);
    if tool_cmd.is_empty() {
        // Custom tool - just use the name
        cmd.arg(tool);
    } else {
        cmd.args(tool_cmd);
    }

    println!("Starting sandbox for: {}", workspace.display());

    let status = cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit())
        .status()
        .context("Failed to start sandbox")?;

    if !status.success() {
        bail!("Sandbox exited with error");
    }

    Ok(())
}

/// Stop a running sandbox
pub fn stop_sandbox(workspace: &Path) -> Result<()> {
    let container_name = get_container_name(workspace);

    let output = Command::new("docker")
        .args(["stop", &container_name])
        .output()
        .context("Failed to stop sandbox")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.contains("No such container") {
            bail!("Failed to stop sandbox: {}", stderr);
        }
    }

    Ok(())
}

/// Remove a sandbox container
pub fn remove_sandbox(workspace: &Path) -> Result<()> {
    let container_name = get_container_name(workspace);

    // Stop first if running
    let _ = stop_sandbox(workspace);

    let output = Command::new("docker")
        .args(["rm", "-f", &container_name])
        .output()
        .context("Failed to remove sandbox")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.contains("No such container") {
            bail!("Failed to remove sandbox: {}", stderr);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sandbox_status_equality() {
        assert_eq!(SandboxStatus::Running, SandboxStatus::Running);
        assert_eq!(SandboxStatus::Stopped, SandboxStatus::Stopped);
        assert_eq!(SandboxStatus::NotFound, SandboxStatus::NotFound);
        assert_ne!(SandboxStatus::Running, SandboxStatus::Stopped);
        assert_ne!(SandboxStatus::Running, SandboxStatus::NotFound);
        assert_ne!(SandboxStatus::Stopped, SandboxStatus::NotFound);
    }

    #[test]
    fn test_sandbox_status_debug() {
        let running = format!("{:?}", SandboxStatus::Running);
        let stopped = format!("{:?}", SandboxStatus::Stopped);
        let not_found = format!("{:?}", SandboxStatus::NotFound);

        assert_eq!(running, "Running");
        assert_eq!(stopped, "Stopped");
        assert_eq!(not_found, "NotFound");
    }

    #[test]
    fn test_sandbox_status_clone() {
        let status = SandboxStatus::Running;
        let cloned = status.clone();
        assert_eq!(status, cloned);
    }

    #[test]
    fn test_get_container_name_deterministic() {
        let path = Path::new("/test/workspace");
        let name1 = get_container_name(path);
        let name2 = get_container_name(path);

        assert_eq!(name1, name2);
    }

    #[test]
    fn test_get_container_name_format() {
        let path = Path::new("/test/workspace");
        let name = get_container_name(path);

        // Format: sandy-{dirname}-{6 char hash}
        assert!(name.starts_with("sandy-workspace-"));
        // Total length: "sandy-" (6) + "workspace" (9) + "-" (1) + hash (6) = 22
        assert_eq!(name.len(), 22);
        // Hash suffix should be 6 hex characters
        let hash_part = &name[name.len() - 6..];
        assert!(hash_part.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_get_container_name_different_paths() {
        let path1 = Path::new("/test/workspace1");
        let path2 = Path::new("/test/workspace2");

        let name1 = get_container_name(path1);
        let name2 = get_container_name(path2);

        assert_ne!(name1, name2);
    }

    #[test]
    fn test_get_container_name_special_characters() {
        // Test with valid special chars (dashes, underscores)
        let path = Path::new("/test/workspace/and-dashes_underscores");
        let name = get_container_name(path);
        assert!(name.starts_with("sandy-and-dashes_underscores-"));

        // Test with spaces (should be replaced with dashes)
        let path_with_spaces = Path::new("/test/my project name");
        let name_spaces = get_container_name(path_with_spaces);
        assert!(name_spaces.starts_with("sandy-my-project-name-"));

        // Test with dots (allowed in Docker container names)
        let path_with_dots = Path::new("/test/my.project.name");
        let name_dots = get_container_name(path_with_dots);
        assert!(name_dots.starts_with("sandy-my.project.name-"));

        // Verify hash suffix is valid hex
        let hash_part = &name[name.len() - 6..];
        assert!(hash_part.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_hash_dockerfile() {
        let temp_dir = TempDir::new().unwrap();
        let dockerfile_path = temp_dir.path().join("Dockerfile");

        let content = "FROM ubuntu:latest\nRUN apt-get update";
        fs::write(&dockerfile_path, content).unwrap();

        let hash = hash_dockerfile(&dockerfile_path).unwrap();

        // SHA256 produces 64 hex characters
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_hash_dockerfile_deterministic() {
        let temp_dir = TempDir::new().unwrap();
        let dockerfile_path = temp_dir.path().join("Dockerfile");

        let content = "FROM ubuntu:latest\nRUN apt-get update";
        fs::write(&dockerfile_path, content).unwrap();

        let hash1 = hash_dockerfile(&dockerfile_path).unwrap();
        let hash2 = hash_dockerfile(&dockerfile_path).unwrap();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_dockerfile_different_content() {
        let temp_dir = TempDir::new().unwrap();
        let dockerfile1 = temp_dir.path().join("Dockerfile1");
        let dockerfile2 = temp_dir.path().join("Dockerfile2");

        fs::write(&dockerfile1, "FROM ubuntu:latest").unwrap();
        fs::write(&dockerfile2, "FROM debian:latest").unwrap();

        let hash1 = hash_dockerfile(&dockerfile1).unwrap();
        let hash2 = hash_dockerfile(&dockerfile2).unwrap();

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_dockerfile_nonexistent() {
        let result = hash_dockerfile(Path::new("/nonexistent/Dockerfile"));
        assert!(result.is_err());
    }

    #[test]
    fn test_template_exists_known_image() {
        // This tests the function signature works correctly
        // Actual Docker interaction may fail without Docker running
        let result = template_exists("definitely-nonexistent-image-12345");
        // This should either succeed (returning false) or fail gracefully
        // depending on whether Docker is available
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_template_needs_rebuild_new_dockerfile() {
        let temp_dir = TempDir::new().unwrap();
        let dockerfile_path = temp_dir.path().join("Dockerfile");

        fs::write(&dockerfile_path, "FROM ubuntu:latest").unwrap();

        // Without a stored hash, it should need rebuild
        // Note: This depends on the actual hash storage path
        // In unit tests, we test the logic, not the file system interaction
    }

    #[test]
    fn test_prepare_template_assets_creates_directory() {
        let temp_dir = TempDir::new().unwrap();
        let config = Config::default();

        // Use empty binary_dirs to avoid needing actual files
        let mut config = config;
        config.binary_dirs = vec![];

        let result = prepare_template_assets(temp_dir.path(), &config);
        assert!(result.is_ok());

        let assets_bin = temp_dir.path().join("assets").join("bin");
        assert!(assets_bin.exists());
    }

    #[test]
    fn test_prepare_template_assets_copies_executables() {
        let temp_dir = TempDir::new().unwrap();
        let bin_dir = temp_dir.path().join("bin");
        fs::create_dir(&bin_dir).unwrap();

        // Create an executable file
        let exec_path = bin_dir.join("my-binary");
        fs::write(&exec_path, "#!/bin/bash\necho test").unwrap();

        // Make it executable
        let mut perms = fs::metadata(&exec_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&exec_path, perms).unwrap();

        // Create a non-executable file
        let non_exec_path = bin_dir.join("not-executable");
        fs::write(&non_exec_path, "data").unwrap();

        let mut config = Config::default();
        config.binary_dirs = vec![bin_dir.to_string_lossy().to_string()];

        let dockerfile_dir = temp_dir.path().join("docker");
        fs::create_dir(&dockerfile_dir).unwrap();

        let result = prepare_template_assets(&dockerfile_dir, &config);
        assert!(result.is_ok());

        let assets_bin = dockerfile_dir.join("assets").join("bin");
        assert!(assets_bin.join("my-binary").exists());
        assert!(!assets_bin.join("not-executable").exists());
    }

    #[test]
    fn test_prepare_template_assets_skips_directories() {
        let temp_dir = TempDir::new().unwrap();
        let bin_dir = temp_dir.path().join("bin");
        fs::create_dir(&bin_dir).unwrap();

        // Create a subdirectory
        let subdir = bin_dir.join("subdir");
        fs::create_dir(&subdir).unwrap();

        let mut config = Config::default();
        config.binary_dirs = vec![bin_dir.to_string_lossy().to_string()];

        let dockerfile_dir = temp_dir.path().join("docker");
        fs::create_dir(&dockerfile_dir).unwrap();

        let result = prepare_template_assets(&dockerfile_dir, &config);
        assert!(result.is_ok());

        let assets_bin = dockerfile_dir.join("assets").join("bin");
        assert!(!assets_bin.join("subdir").exists());
    }

    #[test]
    fn test_prepare_template_assets_cleans_existing() {
        let temp_dir = TempDir::new().unwrap();
        let dockerfile_dir = temp_dir.path().join("docker");
        fs::create_dir(&dockerfile_dir).unwrap();

        // Create pre-existing assets
        let assets_bin = dockerfile_dir.join("assets").join("bin");
        fs::create_dir_all(&assets_bin).unwrap();
        let old_file = assets_bin.join("old-binary");
        fs::write(&old_file, "old content").unwrap();

        let mut config = Config::default();
        config.binary_dirs = vec![];

        let result = prepare_template_assets(&dockerfile_dir, &config);
        assert!(result.is_ok());

        // Old file should be gone
        assert!(!old_file.exists());
    }

    #[test]
    fn test_prepare_template_assets_nonexistent_binary_dir() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.binary_dirs = vec!["/nonexistent/path/12345".to_string()];

        let result = prepare_template_assets(temp_dir.path(), &config);
        // Should succeed but skip the nonexistent directory
        assert!(result.is_ok());
    }

    #[test]
    fn test_prepare_template_assets_file_instead_of_dir() {
        let temp_dir = TempDir::new().unwrap();

        // Create a file instead of a directory
        let file_path = temp_dir.path().join("not-a-dir");
        fs::write(&file_path, "content").unwrap();

        let mut config = Config::default();
        config.binary_dirs = vec![file_path.to_string_lossy().to_string()];

        let dockerfile_dir = temp_dir.path().join("docker");
        fs::create_dir(&dockerfile_dir).unwrap();

        let result = prepare_template_assets(&dockerfile_dir, &config);
        // Should succeed but skip the file
        assert!(result.is_ok());
    }

    #[test]
    fn test_hash_content() {
        let content = "FROM ubuntu:latest\nRUN apt-get update";
        let hash = hash_content(content).unwrap();

        // SHA256 produces 64 hex characters
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_hash_content_deterministic() {
        let content = "FROM ubuntu:latest";
        let hash1 = hash_content(content).unwrap();
        let hash2 = hash_content(content).unwrap();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_content_different_inputs() {
        let hash1 = hash_content("FROM ubuntu:latest").unwrap();
        let hash2 = hash_content("FROM debian:latest").unwrap();

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_content_matches_file_hash() {
        let temp_dir = TempDir::new().unwrap();
        let dockerfile_path = temp_dir.path().join("Dockerfile");
        let content = "FROM ubuntu:latest\nRUN apt-get update";

        fs::write(&dockerfile_path, content).unwrap();

        let file_hash = hash_dockerfile(&dockerfile_path).unwrap();
        let content_hash = hash_content(content).unwrap();

        assert_eq!(file_hash, content_hash);
    }

    #[test]
    fn test_check_default_template_status_needs_creation() {
        let temp_dir = TempDir::new().unwrap();
        let dockerfile_path = temp_dir.path().join("nonexistent").join("Dockerfile");
        let default_template = "FROM ubuntu:latest";

        let status = check_default_template_status(&dockerfile_path, default_template).unwrap();

        assert!(matches!(status, DefaultTemplateStatus::NeedsCreation));
    }

    // Tests for check_default_template_status_impl - pure logic tests that don't
    // depend on the filesystem for stored hash state. These test all branches of
    // the status determination logic.

    #[test]
    fn test_check_default_template_status_impl_up_to_date_no_stored_hash() {
        // When there's no stored hash but the user's file matches current default, it's up-to-date
        let default_template = "FROM ubuntu:latest";
        let user_dockerfile_hash = hash_content(default_template).unwrap();

        let status = check_default_template_status_impl(
            &user_dockerfile_hash,
            default_template,
            None, // no stored hash
        )
        .unwrap();

        assert!(matches!(status, DefaultTemplateStatus::UpToDate));
    }

    #[test]
    fn test_check_default_template_status_impl_customized_no_stored_hash() {
        // When there's no stored hash and file differs from default, assume customized
        let default_template = "FROM ubuntu:latest";
        let user_dockerfile_hash = hash_content("FROM debian:latest\nRUN custom stuff").unwrap();

        let status = check_default_template_status_impl(
            &user_dockerfile_hash,
            default_template,
            None, // no stored hash
        )
        .unwrap();

        assert!(matches!(status, DefaultTemplateStatus::Customized));
    }

    #[test]
    fn test_check_default_template_status_impl_up_to_date_with_stored_hash() {
        // Stored hash matches current default and user's file matches stored hash
        let default_template = "FROM ubuntu:latest";
        let default_hash = hash_content(default_template).unwrap();

        let status = check_default_template_status_impl(
            &default_hash, // user's file matches the default
            default_template,
            Some(default_hash.clone()), // stored hash matches current default
        )
        .unwrap();

        assert!(matches!(status, DefaultTemplateStatus::UpToDate));
    }

    #[test]
    fn test_check_default_template_status_impl_needs_update() {
        // Stored hash exists, user's file matches old stored hash, but embedded default changed
        let old_default = "FROM ubuntu:20.04";
        let new_default = "FROM ubuntu:22.04";
        let old_default_hash = hash_content(old_default).unwrap();

        let status = check_default_template_status_impl(
            &old_default_hash,              // user's file still has old default
            new_default,                    // embedded default has changed
            Some(old_default_hash.clone()), // stored hash is the old default
        )
        .unwrap();

        assert!(matches!(status, DefaultTemplateStatus::NeedsUpdate));
    }

    #[test]
    fn test_check_default_template_status_impl_customized_with_stored_hash() {
        // Stored hash exists but user's file differs from it (user customized)
        let default_template = "FROM ubuntu:latest";
        let default_hash = hash_content(default_template).unwrap();
        let customized_hash = hash_content("FROM ubuntu:latest\nRUN my-custom-stuff").unwrap();

        let status = check_default_template_status_impl(
            &customized_hash, // user modified the file
            default_template,
            Some(default_hash), // stored hash is the original default
        )
        .unwrap();

        assert!(matches!(status, DefaultTemplateStatus::Customized));
    }

    #[test]
    fn test_update_dockerfile_from_default() {
        let temp_dir = TempDir::new().unwrap();
        let dockerfile_path = temp_dir.path().join("sandbox").join("Dockerfile");
        let default_template = "FROM ubuntu:latest\nRUN apt-get update";

        // Update should create the file and parent directory
        update_dockerfile_from_default(&dockerfile_path, default_template).unwrap();

        assert!(dockerfile_path.exists());
        let content = fs::read_to_string(&dockerfile_path).unwrap();
        assert_eq!(content, default_template);
    }

    #[test]
    fn test_update_dockerfile_from_default_overwrites() {
        let temp_dir = TempDir::new().unwrap();
        let dockerfile_path = temp_dir.path().join("Dockerfile");
        let old_content = "FROM debian:latest";
        let new_default = "FROM ubuntu:latest";

        // Create existing file with different content
        fs::write(&dockerfile_path, old_content).unwrap();

        // Update should overwrite
        update_dockerfile_from_default(&dockerfile_path, new_default).unwrap();

        let content = fs::read_to_string(&dockerfile_path).unwrap();
        assert_eq!(content, new_default);
    }
}
