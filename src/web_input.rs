//! Web-specific input handling
//!
//! Provides FPS tracking and overlay mode management for the web version.
//! Unlike native InputHandler, this does NOT use sysinfo since that's not available in WASM.

use crate::demo_core::OverlayMode;
use std::collections::{HashSet, VecDeque};

/// Maximum frame times to keep for FPS averaging
const MAX_FRAME_TIMES: usize = 60;

/// Web input handler for managing keyboard state and FPS tracking
pub struct WebInputHandler {
    /// Currently pressed key codes
    pub pressed_keys: HashSet<String>,
    /// Recent frame times for FPS calculation
    frame_times: VecDeque<f32>,
    /// Current overlay display mode
    pub overlay_mode: OverlayMode,
    /// Whether to show keybindings
    pub show_keybindings: bool,
}

impl Default for WebInputHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl WebInputHandler {
    pub fn new() -> Self {
        Self {
            pressed_keys: HashSet::new(),
            frame_times: VecDeque::with_capacity(MAX_FRAME_TIMES),
            overlay_mode: OverlayMode::Off,
            show_keybindings: false,
        }
    }

    /// Record frame time for FPS calculation
    pub fn update_frame_time(&mut self, dt: f32) {
        self.frame_times.push_back(dt);
        while self.frame_times.len() > MAX_FRAME_TIMES {
            self.frame_times.pop_front();
        }
    }

    /// Calculate current FPS from recent frame times
    pub fn fps(&self) -> f32 {
        if self.frame_times.is_empty() {
            return 0.0;
        }
        let avg_dt: f32 = self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32;
        if avg_dt > 0.0 {
            1.0 / avg_dt
        } else {
            0.0
        }
    }

    /// Key pressed event
    pub fn key_down(&mut self, code: &str) {
        self.pressed_keys.insert(code.to_string());
    }

    /// Key released event
    pub fn key_up(&mut self, code: &str) {
        self.pressed_keys.remove(code);
    }

    /// Check if a key is pressed
    pub fn is_key_pressed(&self, code: &str) -> bool {
        self.pressed_keys.contains(code)
    }

    /// Toggle app-only stats overlay (F key)
    pub fn toggle_overlay_app(&mut self) {
        self.overlay_mode = match self.overlay_mode {
            OverlayMode::Off => OverlayMode::App,
            OverlayMode::App => OverlayMode::Off,
            OverlayMode::Full => OverlayMode::App,
        };
    }

    /// Toggle full stats overlay (G key) - same as App on web since no system stats
    pub fn toggle_overlay_full(&mut self) {
        self.overlay_mode = match self.overlay_mode {
            OverlayMode::Off => OverlayMode::App,
            OverlayMode::App => OverlayMode::Off,
            OverlayMode::Full => OverlayMode::Off,
        };
    }

    /// Toggle keybindings display (K key)
    pub fn toggle_keybindings(&mut self) {
        self.show_keybindings = !self.show_keybindings;
    }

    /// Format stats for overlay display
    pub fn format_stats(&self) -> String {
        if self.overlay_mode == OverlayMode::Off {
            return String::new();
        }

        format!("FPS: {:.0}", self.fps())
    }

    /// Check if overlay should be visible
    pub fn overlay_visible(&self) -> bool {
        self.overlay_mode != OverlayMode::Off
    }
}
