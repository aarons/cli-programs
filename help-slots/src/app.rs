use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use crate::capture::Capturer;
use crate::input::InputHandler;
use crate::preprocessing::Preprocessor;
use crate::puzzles::{PuzzleAction, PuzzleClassifier, PuzzleType};

/// Application state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    /// Helper is disabled, minimal resource usage
    Disabled,
    /// Helper is enabled, looking for puzzles at low frequency
    Enabled,
    /// A puzzle is active, running at high frequency
    PuzzleActive(PuzzleType),
}

/// Main application controller
pub struct App {
    capturer: Capturer,
    preprocessor: Preprocessor,
    classifier: PuzzleClassifier,
    enabled: Arc<AtomicBool>,
    state: AppState,
}

impl App {
    pub fn new(
        window_name: &str,
        preprocessor: Preprocessor,
        classifier: PuzzleClassifier,
        enabled: Arc<AtomicBool>,
    ) -> Self {
        Self {
            capturer: Capturer::new(window_name),
            preprocessor,
            classifier,
            enabled,
            state: AppState::Disabled,
        }
    }

    /// Run the main application loop
    pub async fn run(&mut self) -> Result<()> {
        log::info!("Starting help-slots main loop");
        log::info!("Press 'F' to toggle assistance");

        loop {
            // Check if enabled state changed
            let is_enabled = self.enabled.load(Ordering::SeqCst);

            // Handle state transitions
            match (&self.state, is_enabled) {
                (AppState::Disabled, true) => {
                    log::info!("Assistance enabled - looking for puzzles");
                    self.state = AppState::Enabled;
                }
                (AppState::Enabled | AppState::PuzzleActive(_), false) => {
                    log::info!("Assistance disabled");
                    // Reset any active handler
                    if let AppState::PuzzleActive(puzzle_type) = self.state {
                        if let Some(handler) = self.classifier.get_handler_mut(puzzle_type) {
                            handler.reset();
                        }
                    }
                    self.state = AppState::Disabled;
                }
                _ => {}
            }

            // Main state machine
            match self.state {
                AppState::Disabled => {
                    // Low power mode - just check toggle periodically
                    sleep(Duration::from_millis(100)).await;
                }

                AppState::Enabled => {
                    // Capture and check for puzzle at ~1Hz
                    if let Ok(frame) = self.capturer.capture_full() {
                        let edges = self.preprocessor.process(&frame);

                        if let Some(puzzle_type) = self.classifier.detect_active_puzzle(&edges) {
                            log::info!("Detected {} puzzle!", puzzle_type);
                            self.state = AppState::PuzzleActive(puzzle_type);
                        }
                    }

                    sleep(Duration::from_secs(1)).await;
                }

                AppState::PuzzleActive(puzzle_type) => {
                    // High frequency capture and processing
                    if let Ok(frame) = self.capturer.capture_full() {
                        let edges = self.preprocessor.process(&frame);

                        if let Some(handler) = self.classifier.get_handler_mut(puzzle_type) {
                            match handler.process_frame(&frame, &edges) {
                                PuzzleAction::Trigger => {
                                    if let Err(e) = InputHandler::send_spacebar() {
                                        log::error!("Failed to send spacebar: {}", e);
                                    }
                                    // Brief cooldown after trigger
                                    sleep(Duration::from_millis(100)).await;
                                }
                                PuzzleAction::PuzzleComplete => {
                                    log::info!("Puzzle complete, returning to search mode");
                                    handler.reset();
                                    self.state = AppState::Enabled;
                                }
                                PuzzleAction::Wait => {}
                            }
                        } else {
                            // No handler found, go back to enabled
                            log::warn!("No handler for puzzle type {:?}", puzzle_type);
                            self.state = AppState::Enabled;
                        }
                    }

                    // ~60 FPS
                    sleep(Duration::from_millis(16)).await;
                }
            }
        }
    }

    /// Get current state
    pub fn state(&self) -> AppState {
        self.state
    }
}
