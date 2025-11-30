use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::config::Config;
use crate::state::{load_template_hash, save_template_hash};

/// Binaries to build and include in the template image
const TEMPLATE_BINARIES: &[&str] = &["gc"];

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

/// Find the workspace root by looking for Cargo.toml with [workspace]
fn find_workspace_root() -> Result<PathBuf> {
    let exe_path = std::env::current_exe().context("Failed to get current executable path")?;

    // Walk up from the executable location to find workspace root
    let mut current = exe_path.parent();
    while let Some(dir) = current {
        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.exists() {
            let content = fs::read_to_string(&cargo_toml).unwrap_or_default();
            if content.contains("[workspace]") {
                return Ok(dir.to_path_buf());
            }
        }
        current = dir.parent();
    }

    // Fallback: try current directory and walk up
    let mut current = std::env::current_dir().ok();
    while let Some(dir) = current {
        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.exists() {
            let content = fs::read_to_string(&cargo_toml).unwrap_or_default();
            if content.contains("[workspace]") {
                return Ok(dir);
            }
        }
        current = dir.parent().map(|p| p.to_path_buf());
    }

    bail!("Could not find workspace root (Cargo.toml with [workspace])")
}

/// Prepare template assets by building required binaries
pub fn prepare_template_assets(dockerfile_dir: &Path) -> Result<()> {
    let assets_dir = dockerfile_dir.join("assets");
    fs::create_dir_all(&assets_dir).context("Failed to create assets directory")?;

    let workspace_root = find_workspace_root()?;

    println!("Building template binaries...");

    // Build all binaries in release mode
    let packages: Vec<_> = TEMPLATE_BINARIES.iter().flat_map(|p| ["-p", p]).collect();
    let status = Command::new("cargo")
        .current_dir(&workspace_root)
        .args(["build", "--release"])
        .args(&packages)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("Failed to run cargo build")?;

    if !status.success() {
        bail!("Failed to build template binaries");
    }

    // Copy binaries to assets directory
    let target_dir = workspace_root.join("target/release");
    for binary in TEMPLATE_BINARIES {
        let src = target_dir.join(binary);
        let dst = assets_dir.join(binary);

        if !src.exists() {
            bail!("Binary not found after build: {}", src.display());
        }

        fs::copy(&src, &dst)
            .with_context(|| format!("Failed to copy {} to assets", binary))?;

        println!("  Copied {} to assets/", binary);
    }

    Ok(())
}

/// Build the custom template image
pub fn build_template(dockerfile_path: &Path, image_name: &str) -> Result<()> {
    let dockerfile_dir = dockerfile_path.parent().unwrap_or(Path::new("."));

    // Prepare assets before building
    prepare_template_assets(dockerfile_dir)?;

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

    // Mount ~/.claude for authentication
    let claude_dir = dirs::home_dir()
        .context("Could not determine home directory")?
        .join(".claude");
    cmd.args([
        "-v",
        &format!("{}:/home/agent/.claude", claude_dir.display()),
    ]);

    // Mount additional configured volumes
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

