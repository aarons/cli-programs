use image::{GrayImage, RgbaImage};
use std::time::Instant;

use super::{PuzzleAction, PuzzleHandler, PuzzleType};
use crate::preprocessing::template_match;

/// Configuration for reticle puzzle detection
#[derive(Debug, Clone)]
pub struct ReticleConfig {
    /// Confidence threshold for detecting the puzzle is active
    pub activation_threshold: f32,
    /// Distance threshold (in pixels) for triggering spacebar
    pub trigger_distance: u32,
    /// Cooldown after triggering (prevents double-trigger)
    pub cooldown_ms: u64,
    /// Expected center of target zone (relative to puzzle ROI)
    pub target_center: (u32, u32),
}

impl Default for ReticleConfig {
    fn default() -> Self {
        Self {
            activation_threshold: 0.7,
            trigger_distance: 20,
            cooldown_ms: 500,
            // Default target center - should be calibrated
            target_center: (100, 100),
        }
    }
}

/// Handler for the reticle/aiming timing puzzle
pub struct ReticleHandler {
    config: ReticleConfig,
    /// Template for detecting if the puzzle UI is active
    puzzle_template: Option<GrayImage>,
    /// Template for the reticle/crosshair
    reticle_template: Option<GrayImage>,
    /// Last trigger time (for cooldown)
    last_trigger: Option<Instant>,
    /// Whether we've detected the puzzle as active this session
    is_active: bool,
    /// Count of frames where puzzle was not detected (for exit detection)
    inactive_frames: u32,
}

impl ReticleHandler {
    pub fn new(config: ReticleConfig) -> Self {
        Self {
            config,
            puzzle_template: None,
            reticle_template: None,
            last_trigger: None,
            is_active: false,
            inactive_frames: 0,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(ReticleConfig::default())
    }

    /// Load templates from files
    pub fn load_templates(
        &mut self,
        puzzle_template_path: Option<&str>,
        reticle_template_path: Option<&str>,
    ) -> anyhow::Result<()> {
        if let Some(path) = puzzle_template_path {
            let img = image::open(path)?;
            self.puzzle_template = Some(img.to_luma8());
            log::info!("Loaded puzzle template from {}", path);
        }

        if let Some(path) = reticle_template_path {
            let img = image::open(path)?;
            self.reticle_template = Some(img.to_luma8());
            log::info!("Loaded reticle template from {}", path);
        }

        Ok(())
    }

    /// Set templates directly (for testing or programmatic use)
    pub fn set_templates(
        &mut self,
        puzzle_template: Option<GrayImage>,
        reticle_template: Option<GrayImage>,
    ) {
        self.puzzle_template = puzzle_template;
        self.reticle_template = reticle_template;
    }

    /// Calculate distance from reticle position to target center
    fn distance_to_target(&self, reticle_pos: (u32, u32)) -> f64 {
        let dx = reticle_pos.0 as f64 - self.config.target_center.0 as f64;
        let dy = reticle_pos.1 as f64 - self.config.target_center.1 as f64;
        (dx * dx + dy * dy).sqrt()
    }

    /// Check if cooldown has elapsed
    fn cooldown_elapsed(&self) -> bool {
        match self.last_trigger {
            Some(t) => t.elapsed().as_millis() as u64 >= self.config.cooldown_ms,
            None => true,
        }
    }
}

impl PuzzleHandler for ReticleHandler {
    fn puzzle_type(&self) -> PuzzleType {
        PuzzleType::Reticle
    }

    fn detect_active(&self, edges: &GrayImage) -> bool {
        // If no template, can't detect
        let template = match &self.puzzle_template {
            Some(t) => t,
            None => {
                log::debug!("No puzzle template loaded, skipping detection");
                return false;
            }
        };

        // Template match
        if let Some((_, _, confidence)) = template_match(edges, template) {
            let is_match = confidence >= self.config.activation_threshold;
            log::debug!(
                "Puzzle detection: confidence={:.3}, threshold={:.3}, match={}",
                confidence,
                self.config.activation_threshold,
                is_match
            );
            is_match
        } else {
            false
        }
    }

    fn process_frame(&mut self, _original: &RgbaImage, edges: &GrayImage) -> PuzzleAction {
        // Check for puzzle exit (no puzzle template match for several frames)
        if let Some(template) = &self.puzzle_template {
            if let Some((_, _, confidence)) = template_match(edges, template) {
                if confidence < self.config.activation_threshold * 0.8 {
                    self.inactive_frames += 1;
                    if self.inactive_frames > 30 {
                        // ~0.5 seconds at 60fps
                        log::info!("Puzzle appears to have ended");
                        return PuzzleAction::PuzzleComplete;
                    }
                } else {
                    self.inactive_frames = 0;
                }
            }
        }

        self.is_active = true;

        // Find reticle position
        let reticle_template = match &self.reticle_template {
            Some(t) => t,
            None => {
                log::debug!("No reticle template loaded, cannot track");
                return PuzzleAction::Wait;
            }
        };

        if let Some((x, y, confidence)) = template_match(edges, reticle_template) {
            if confidence < 0.5 {
                log::debug!("Reticle match confidence too low: {:.3}", confidence);
                return PuzzleAction::Wait;
            }

            // Calculate distance to target
            let distance = self.distance_to_target((x, y));
            log::debug!(
                "Reticle at ({}, {}), distance to target: {:.1}px",
                x,
                y,
                distance
            );

            // Check if within trigger distance and cooldown elapsed
            if distance <= self.config.trigger_distance as f64 && self.cooldown_elapsed() {
                log::info!("Triggering! Distance: {:.1}px", distance);
                self.last_trigger = Some(Instant::now());
                return PuzzleAction::Trigger;
            }
        }

        PuzzleAction::Wait
    }

    fn reset(&mut self) {
        self.is_active = false;
        self.inactive_frames = 0;
        self.last_trigger = None;
        log::debug!("Reticle handler reset");
    }
}
