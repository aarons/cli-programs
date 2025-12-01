use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::config::Config;
use crate::state::{load_template_hash, save_template_hash};

/// Status of a sandbox container
#[derive(Debug, Clone, PartialEq)]
pub enum SandboxStatus {
    Running,
    Stopped,
    NotFound,
}

/// Get the sandbox container name for a workspace path
fn get_container_name(workspace: &Path) -> String {
    // Create a deterministic name based on workspace path
    let mut hasher = Sha256::new();
    hasher.update(workspace.to_string_lossy().as_bytes());
    let hash = hex::encode(hasher.finalize());
    format!("sandbox-{}", &hash[..12])
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

/// Calculate hash of a Dockerfile
pub fn hash_dockerfile(dockerfile_path: &Path) -> Result<String> {
    let content = fs::read_to_string(dockerfile_path)
        .with_context(|| format!("Failed to read Dockerfile: {}", dockerfile_path.display()))?;

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

            fs::copy(&path, &dst)
                .with_context(|| format!("Failed to copy {}", path.display()))?;

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
    let dockerfile_dir = dockerfile_path.parent().unwrap_or(Path::new("."));

    // Prepare assets before building
    prepare_template_assets(dockerfile_dir, config)?;

    println!("Building custom template image: {}", image_name);

    let status = Command::new("docker")
        .args([
            "build",
            "-t",
            image_name,
            "-f",
            &dockerfile_path.to_string_lossy(),
            &dockerfile_dir.to_string_lossy(),
        ])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("Failed to execute docker build")?;

    if !status.success() {
        bail!("Failed to build template image");
    }

    // Save the hash after successful build
    let hash = hash_dockerfile(dockerfile_path)?;
    save_template_hash(&hash)?;

    println!("Template image built successfully: {}", image_name);
    Ok(())
}

/// Get the status of a sandbox
pub fn sandbox_status(workspace: &Path) -> Result<SandboxStatus> {
    let container_name = get_container_name(workspace);

    let output = Command::new("docker")
        .args(["ps", "-a", "--filter", &format!("name={}", container_name), "--format", "{{.Status}}"])
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

/// Start a new sandbox with the given configuration
pub fn start_sandbox(workspace: &Path, config: &Config) -> Result<()> {
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
        if let Ok(expanded) = Config::expand_env(value) {
            if !expanded.is_empty() {
                cmd.args(["-e", &format!("{}={}", key, expanded)]);
            }
        }
    }

    // Custom template if configured
    if let Some(ref template) = config.template_image {
        cmd.args(["--template", template]);
    }

    // Credentials none since we mount ~/.claude
    cmd.args(["--credentials=none"]);

    // Name the container for tracking
    let container_name = get_container_name(workspace);
    cmd.args(["--name", &container_name]);

    // Workspace
    cmd.args(["-w", &workspace.display().to_string()]);

    // Agent and permissions
    cmd.args(["claude", "--dangerously-skip-permissions"]);

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

/// Attach to a running sandbox
pub fn attach_sandbox(workspace: &Path) -> Result<()> {
    let container_name = get_container_name(workspace);

    let status = Command::new("docker")
        .args(["attach", &container_name])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit())
        .status()
        .context("Failed to attach to sandbox")?;

    if !status.success() {
        bail!("Failed to attach to sandbox");
    }

    Ok(())
}

