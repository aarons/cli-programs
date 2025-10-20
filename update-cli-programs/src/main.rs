use anyhow::{Context, Result};
use clap::Parser;
use serde::Deserialize;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const WORKSPACE_ROOT: &str = env!("CARGO_MANIFEST_DIR");

const EXCLUDED_PACKAGES: &[&str] = &[
    "changelog-validator",
];

#[derive(Parser)]
#[command(name = "update-cli-programs")]
#[command(about = "Update all cli-programs binaries in ~/.local/bin")]
struct Cli {
    /// Target directory (defaults to ~/.local/bin)
    #[arg(short, long)]
    target: Option<PathBuf>,
}

#[derive(Deserialize)]
struct WorkspaceToml {
    workspace: Workspace,
}

#[derive(Deserialize)]
struct Workspace {
    members: Vec<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let workspace_root = Path::new(WORKSPACE_ROOT)
        .parent()
        .context("Failed to determine workspace root")?;

    // Determine target directory
    let target_dir = cli.target.unwrap_or_else(|| {
        let home = std::env::var("HOME").expect("HOME environment variable not set");
        PathBuf::from(home).join(".local").join("bin")
    });

    // Create target directory if it doesn't exist
    fs::create_dir_all(&target_dir)
        .context("Failed to create target directory")?;

    // Read workspace Cargo.toml
    let workspace_toml_path = workspace_root.join("Cargo.toml");
    let workspace_toml_content = fs::read_to_string(&workspace_toml_path)
        .context("Failed to read workspace Cargo.toml")?;

    let workspace_toml: WorkspaceToml = toml::from_str(&workspace_toml_content)
        .context("Failed to parse workspace Cargo.toml")?;

    // Get all workspace members, excluding those in EXCLUDED_PACKAGES
    let programs: Vec<String> = workspace_toml
        .workspace
        .members
        .into_iter()
        .filter(|p| !EXCLUDED_PACKAGES.contains(&p.as_str()))
        .collect();

    if programs.is_empty() {
        println!("No programs to install");
        return Ok(());
    }

    println!("Building Rust tools...");

    let build_status = Command::new("cargo")
        .args(&["build", "--release", "--workspace"])
        .current_dir(workspace_root)
        .status()
        .context("Failed to run cargo build")?;

    if !build_status.success() {
        anyhow::bail!("Failed to build Rust tools");
    }

    println!("\nInstalling programs:");

    // Install each program
    for program in &programs {
        let binary_path = workspace_root
            .join("target")
            .join("release")
            .join(program);

        if !binary_path.exists() {
            continue;
        }

        let target_path = target_dir.join(program);

        // Copy binary
        fs::copy(&binary_path, &target_path)
            .with_context(|| format!("Failed to copy {} to {}", program, target_path.display()))?;

        // Make executable
        let mut perms = fs::metadata(&target_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&target_path, perms)
            .with_context(|| format!("Failed to set permissions on {}", target_path.display()))?;

        println!("  - {}", program);
    }

    println!("\nPrograms installed to {}", target_dir.display());

    Ok(())
}
