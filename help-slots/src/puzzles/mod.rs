use image::{GrayImage, RgbaImage};

pub mod reticle;

/// Types of puzzles the helper can assist with
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PuzzleType {
    Reticle,
    // Future puzzle types can be added here
}

impl std::fmt::Display for PuzzleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PuzzleType::Reticle => write!(f, "Reticle"),
        }
    }
}

/// Action returned by a puzzle handler after processing a frame
#[derive(Debug, Clone, PartialEq)]
pub enum PuzzleAction {
    /// Not yet time to trigger
    Wait,
    /// Send spacebar now
    Trigger,
    /// Puzzle has ended (success or fail)
    PuzzleComplete,
}

/// Trait for puzzle-specific detection and timing logic
pub trait PuzzleHandler: Send + Sync {
    /// Unique identifier for this puzzle type
    fn puzzle_type(&self) -> PuzzleType;

    /// Check if this puzzle is currently active (for classifier)
    /// Takes a preprocessed (edge-detected) frame
    fn detect_active(&self, edges: &GrayImage) -> bool;

    /// Process frame and return action (for active puzzle)
    /// Takes both the original frame and preprocessed edges
    fn process_frame(&mut self, original: &RgbaImage, edges: &GrayImage) -> PuzzleAction;

    /// Reset state when puzzle ends
    fn reset(&mut self);
}

/// Classifier that determines which puzzle is currently active
pub struct PuzzleClassifier {
    handlers: Vec<Box<dyn PuzzleHandler>>,
}

impl PuzzleClassifier {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    pub fn add_handler(&mut self, handler: Box<dyn PuzzleHandler>) {
        self.handlers.push(handler);
    }

    /// Run at ~1Hz when enabled but no puzzle active
    /// Returns the type of puzzle detected, if any
    pub fn detect_active_puzzle(&self, edges: &GrayImage) -> Option<PuzzleType> {
        for handler in &self.handlers {
            if handler.detect_active(edges) {
                return Some(handler.puzzle_type());
            }
        }
        None
    }

    /// Get a mutable reference to a handler by puzzle type
    pub fn get_handler_mut(&mut self, puzzle_type: PuzzleType) -> Option<&mut Box<dyn PuzzleHandler>> {
        self.handlers
            .iter_mut()
            .find(|h| h.puzzle_type() == puzzle_type)
    }
}

impl Default for PuzzleClassifier {
    fn default() -> Self {
        Self::new()
    }
}
