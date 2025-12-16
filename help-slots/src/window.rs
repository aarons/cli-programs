use anyhow::{Context, Result};
use xcap::Window;

#[derive(Debug, Clone)]
pub struct GameWindow {
    pub window_id: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub app_name: String,
    pub title: String,
}

#[derive(Debug, Clone, Copy)]
pub struct WindowBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl GameWindow {
    /// Find the game window by name (matches app name or window title)
    pub fn find_by_name(name: &str) -> Result<Option<Self>> {
        let windows = Window::all().context("Failed to enumerate windows")?;

        for window in windows {
            let app_name = window.app_name().unwrap_or_default();
            let title = window.title().unwrap_or_default();

            if app_name.contains(name) || title.contains(name) {
                return Ok(Some(Self {
                    window_id: window.id().unwrap_or(0),
                    x: window.x().unwrap_or(0),
                    y: window.y().unwrap_or(0),
                    width: window.width().unwrap_or(0),
                    height: window.height().unwrap_or(0),
                    app_name,
                    title,
                }));
            }
        }

        Ok(None)
    }

    /// Get window bounds
    pub fn bounds(&self) -> WindowBounds {
        WindowBounds {
            x: self.x as f64,
            y: self.y as f64,
            width: self.width as f64,
            height: self.height as f64,
        }
    }

    /// Check if this window is currently focused
    /// Note: This is an approximation - checks if the window is first in the list
    pub fn is_focused(&self) -> Result<bool> {
        let windows = Window::all().context("Failed to enumerate windows")?;

        // The first window that matches our app is likely the focused one
        for window in windows {
            let app_name = window.app_name().unwrap_or_default();
            if app_name == self.app_name {
                let id = window.id().unwrap_or(0);
                return Ok(id == self.window_id);
            }
        }

        Ok(false)
    }

    /// Refresh the window info (in case it was moved/resized)
    pub fn refresh(&mut self) -> Result<()> {
        if let Some(updated) = Self::find_by_name(&self.app_name)? {
            self.x = updated.x;
            self.y = updated.y;
            self.width = updated.width;
            self.height = updated.height;
        }
        Ok(())
    }
}
