use anyhow::{Context, Result};
use image::{ImageBuffer, RgbaImage};
use xcap::Window;

use crate::window::WindowBounds;

pub struct Capturer {
    window_name: String,
}

impl Capturer {
    pub fn new(window_name: &str) -> Self {
        Self {
            window_name: window_name.to_string(),
        }
    }

    /// Capture the full game window
    pub fn capture_full(&self) -> Result<RgbaImage> {
        let window = self.find_xcap_window()?;
        let capture = window.capture_image().context("Failed to capture window")?;

        // Convert xcap image to image::RgbaImage
        let width = capture.width();
        let height = capture.height();
        let raw = capture.into_raw();

        ImageBuffer::from_raw(width, height, raw)
            .context("Failed to create image buffer from capture")
    }

    /// Capture a specific region of the game window
    pub fn capture_region(&self, _bounds: &WindowBounds, roi: &Region) -> Result<RgbaImage> {
        let full = self.capture_full()?;
        self.extract_region(&full, roi)
    }

    /// Extract a region from an already-captured image
    pub fn extract_region(&self, image: &RgbaImage, roi: &Region) -> Result<RgbaImage> {
        let (img_width, img_height) = image.dimensions();

        // Clamp region to image bounds
        let x = roi.x.min(img_width as i32).max(0) as u32;
        let y = roi.y.min(img_height as i32).max(0) as u32;
        let width = roi.width.min(img_width - x);
        let height = roi.height.min(img_height - y);

        let mut region_image: RgbaImage = ImageBuffer::new(width, height);

        for dy in 0..height {
            for dx in 0..width {
                let pixel = image.get_pixel(x + dx, y + dy);
                region_image.put_pixel(dx, dy, *pixel);
            }
        }

        Ok(region_image)
    }

    fn find_xcap_window(&self) -> Result<Window> {
        let windows = Window::all().context("Failed to enumerate windows")?;

        for window in windows {
            let title = window.title().unwrap_or_default();
            let app_name = window.app_name().unwrap_or_default();

            if title.contains(&self.window_name) || app_name.contains(&self.window_name) {
                return Ok(window);
            }
        }

        anyhow::bail!("Window '{}' not found", self.window_name)
    }
}

/// A rectangular region within an image
#[derive(Debug, Clone, Copy)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Region {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create a region from window bounds (typically for full window capture)
    pub fn from_bounds(bounds: &WindowBounds) -> Self {
        Self {
            x: 0,
            y: 0,
            width: bounds.width as u32,
            height: bounds.height as u32,
        }
    }

    /// Create a centered region of given size
    pub fn centered(center_x: i32, center_y: i32, width: u32, height: u32) -> Self {
        Self {
            x: center_x - (width as i32 / 2),
            y: center_y - (height as i32 / 2),
            width,
            height,
        }
    }
}

/// Save an image to disk for debugging
pub fn save_debug_image(image: &RgbaImage, path: &str) -> Result<()> {
    image.save(path).context("Failed to save debug image")?;
    Ok(())
}
