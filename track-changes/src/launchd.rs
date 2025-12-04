use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const PLIST_LABEL: &str = "com.cli-programs.track-changes";

/// Get the plist file path
pub fn plist_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{}.plist", PLIST_LABEL)))
}

/// Generate the launchd plist content
pub fn generate_plist() -> Result<String> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let binary_path = home.join(".local").join("bin").join("track-changes");
    let log_dir = home.join(".local").join("share").join("track-changes");

    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{binary}</string>
    </array>
    <key>StartInterval</key>
    <integer>3600</integer>
    <key>StandardOutPath</key>
    <string>{log_dir}/launchd-stdout.log</string>
    <key>StandardErrorPath</key>
    <string>{log_dir}/launchd-stderr.log</string>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
"#,
        label = PLIST_LABEL,
        binary = binary_path.display(),
        log_dir = log_dir.display()
    );

    Ok(plist)
}

/// Install and load the launchd plist
pub fn install() -> Result<()> {
    let path = plist_path()?;

    // Check if already installed and unload first
    if path.exists() {
        eprintln!("Existing plist found, updating...");
        let _ = Command::new("launchctl")
            .args(["unload", path.to_str().unwrap()])
            .status();
    }

    // Ensure LaunchAgents directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create LaunchAgents directory: {}",
                parent.display()
            )
        })?;
    }

    // Ensure log directory exists
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let log_dir = home.join(".local").join("share").join("track-changes");
    fs::create_dir_all(&log_dir)
        .with_context(|| format!("Failed to create log directory: {}", log_dir.display()))?;

    // Write plist
    let plist = generate_plist()?;
    fs::write(&path, &plist)
        .with_context(|| format!("Failed to write plist: {}", path.display()))?;

    // Load the launch agent
    let status = Command::new("launchctl")
        .args(["load", path.to_str().unwrap()])
        .status()
        .context("Failed to run launchctl load")?;

    if !status.success() {
        anyhow::bail!("launchctl load failed");
    }

    println!("Installed and loaded: {}", path.display());
    println!("track-changes will run every hour");
    Ok(())
}

/// Unload and remove the launchd plist
pub fn uninstall() -> Result<()> {
    let path = plist_path()?;

    if !path.exists() {
        println!("Launch agent not installed");
        return Ok(());
    }

    // Unload the launch agent
    let status = Command::new("launchctl")
        .args(["unload", path.to_str().unwrap()])
        .status()
        .context("Failed to run launchctl unload")?;

    if !status.success() {
        eprintln!("Warning: launchctl unload may have failed");
    }

    // Remove plist file
    fs::remove_file(&path)
        .with_context(|| format!("Failed to remove plist: {}", path.display()))?;

    println!("Uninstalled: {}", path.display());
    Ok(())
}

/// Check if the launch agent is currently installed
pub fn is_installed() -> Result<bool> {
    let path = plist_path()?;
    Ok(path.exists())
}
