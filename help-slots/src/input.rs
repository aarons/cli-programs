use anyhow::Result;
use rdev::{listen, simulate, Event, EventType, Key};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Handles hotkey listening and key injection
pub struct InputHandler {
    enabled: Arc<AtomicBool>,
    toggle_key: Key,
}

impl InputHandler {
    pub fn new(toggle_key: Key) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(false)),
            toggle_key,
        }
    }

    /// Get a clone of the enabled flag for checking state
    pub fn enabled_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.enabled)
    }

    /// Check if the helper is currently enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    /// Toggle the enabled state
    pub fn toggle(&self) {
        let current = self.enabled.load(Ordering::SeqCst);
        self.enabled.store(!current, Ordering::SeqCst);
        log::info!(
            "Helper {}",
            if !current { "enabled" } else { "disabled" }
        );
    }

    /// Start listening for the toggle hotkey in a background thread
    pub fn start_hotkey_listener(&self) -> thread::JoinHandle<()> {
        let enabled = Arc::clone(&self.enabled);
        let toggle_key = self.toggle_key;

        thread::spawn(move || {
            let callback = move |event: Event| {
                if let EventType::KeyPress(key) = event.event_type {
                    if key == toggle_key {
                        let current = enabled.load(Ordering::SeqCst);
                        enabled.store(!current, Ordering::SeqCst);
                        log::info!(
                            "Helper {}",
                            if !current { "enabled" } else { "disabled" }
                        );
                    }
                }
            };

            if let Err(e) = listen(callback) {
                log::error!("Hotkey listener error: {:?}", e);
            }
        })
    }

    /// Send a spacebar keypress using rdev
    pub fn send_spacebar() -> Result<()> {
        Self::send_key(Key::Space)
    }

    /// Send a specific key press and release
    pub fn send_key(key: Key) -> Result<()> {
        // Key down
        simulate(&EventType::KeyPress(key))
            .map_err(|e| anyhow::anyhow!("Failed to simulate key press: {:?}", e))?;

        // Small delay between down and up
        thread::sleep(Duration::from_millis(10));

        // Key up
        simulate(&EventType::KeyRelease(key))
            .map_err(|e| anyhow::anyhow!("Failed to simulate key release: {:?}", e))?;

        log::debug!("Key {:?} sent", key);
        Ok(())
    }
}

/// Convert rdev Key to a display string
pub fn key_name(key: Key) -> &'static str {
    match key {
        Key::KeyF => "F",
        Key::KeyG => "G",
        Key::Space => "Space",
        Key::Escape => "Escape",
        _ => "Unknown",
    }
}
