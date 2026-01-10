use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const PLIST_LABEL: &str = "com.cli-programs.zoom-remove";
const ZOOM_AGENT_PREFIX: &str = "us.zoom.updater";

/// Get the LaunchAgents directory path
fn launch_agents_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join("Library").join("LaunchAgents"))
}

/// Get the plist file path for our own scheduler
fn plist_path() -> Result<PathBuf> {
    let dir = launch_agents_dir()?;
    Ok(dir.join(format!("{}.plist", PLIST_LABEL)))
}

/// Find all Zoom updater LaunchAgent plist files
pub fn find_zoom_agents() -> Result<Vec<PathBuf>> {
    let dir = launch_agents_dir()?;

    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut agents = Vec::new();

    for entry in fs::read_dir(&dir).with_context(|| format!("Failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();

        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with(ZOOM_AGENT_PREFIX) && name.ends_with(".plist") {
                agents.push(path);
            }
        }
    }

    agents.sort();
    Ok(agents)
}

/// Extract the service label from a plist filename
fn label_from_path(path: &PathBuf) -> Option<String> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

/// Get the current user's UID for launchctl domain
fn get_uid() -> Result<String> {
    let output = Command::new("id")
        .arg("-u")
        .output()
        .context("Failed to run 'id -u'")?;

    if !output.status.success() {
        anyhow::bail!("'id -u' failed");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Bootout a LaunchAgent and remove its plist file
pub fn bootout_and_remove(path: &PathBuf) -> Result<()> {
    let label = label_from_path(path).context("Could not extract label from plist path")?;
    let uid = get_uid()?;
    let domain_target = format!("gui/{}/{}", uid, label);

    // Try to bootout the service (may fail if not running, that's OK)
    let _ = Command::new("launchctl")
        .args(["bootout", &domain_target])
        .status();

    // Remove the plist file
    fs::remove_file(path).with_context(|| format!("Failed to remove {}", path.display()))?;

    Ok(())
}

/// Generate the launchd plist content for daily scheduling
fn generate_plist() -> Result<String> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let binary_path = home.join(".local").join("bin").join("zoom-remove");
    let log_dir = home.join(".local").join("share").join("zoom-remove");

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
    <key>StartCalendarInterval</key>
    <dict>
        <key>Hour</key>
        <integer>10</integer>
        <key>Minute</key>
        <integer>0</integer>
    </dict>
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

/// Install and load the daily scheduler
pub fn install() -> Result<()> {
    let path = plist_path()?;

    // Check if already installed and unload first
    if path.exists() {
        println!("Existing plist found, updating...");
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
    let log_dir = home.join(".local").join("share").join("zoom-remove");
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

    println!("Installed: {}", path.display());
    println!("zoom-remove will run daily at 10:00 AM");

    // Run immediately to clean up any existing agents
    let agents = find_zoom_agents()?;
    if !agents.is_empty() {
        println!("\nRunning initial cleanup...");
        for agent in agents {
            print!("  {}", agent.display());
            match bootout_and_remove(&agent) {
                Ok(()) => println!(" - removed"),
                Err(e) => println!(" - error: {}", e),
            }
        }
    }

    Ok(())
}

/// Unload and remove the daily scheduler
pub fn uninstall() -> Result<()> {
    let path = plist_path()?;

    if !path.exists() {
        println!("Daily scheduler not installed");
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

/// Check if the daily scheduler is currently installed
pub fn is_installed() -> Result<bool> {
    let path = plist_path()?;
    Ok(path.exists())
}
