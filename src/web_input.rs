//! Web-specific input handling
//!
//! Keeps lightweight keyboard/mouse state and overlay timing for the web runtime.
//! Unlike the native input path, this does not depend on sysinfo.

use crate::demo_core::OverlayMode;
use std::collections::{HashSet, VecDeque};

/// Maximum frame times to keep for FPS averaging
const MAX_FRAME_TIMES: usize = 60;

/// Web input handler for managing keyboard/mouse state and FPS tracking.
pub struct WebInputHandler {
    /// Currently pressed key codes
    pub pressed_keys: HashSet<String>,
    /// When true, keyboard input is paused until the canvas is clicked again
    pub keyboard_paused: bool,
    /// Whether the mouse is currently captured/locked for look controls
    pub mouse_captured: bool,
    /// Whether the primary mouse button is dragging for fallback look controls
    mouse_dragging: bool,
    /// Last known mouse position for drag-based controls
    last_mouse_position: Option<[f32; 2]>,
    /// Accumulated mouse delta since the last frame
    pending_mouse_delta: [f32; 2],
    /// Accumulated wheel delta since the last frame
    pending_wheel_delta: f32,
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
            keyboard_paused: true,
            mouse_captured: false,
            mouse_dragging: false,
            last_mouse_position: None,
            pending_mouse_delta: [0.0, 0.0],
            pending_wheel_delta: 0.0,
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

    pub fn take_key_pressed(&mut self, code: &str) -> bool {
        self.pressed_keys.take(code).is_some()
    }

    pub fn clear_pressed_keys(&mut self) {
        self.pressed_keys.clear();
    }

    pub fn camera_controls_active(&self) -> bool {
        self.mouse_captured || self.mouse_dragging
    }

    pub fn set_mouse_captured(&mut self, captured: bool) {
        self.mouse_captured = captured;
        if !captured {
            self.mouse_dragging = false;
            self.last_mouse_position = None;
        }
    }

    pub fn mouse_down(&mut self, x: f32, y: f32) {
        self.mouse_dragging = true;
        self.last_mouse_position = Some([x, y]);
    }

    pub fn mouse_up(&mut self) {
        self.mouse_dragging = false;
        self.last_mouse_position = None;
    }

    pub fn mouse_move(&mut self, x: f32, y: f32, movement_x: f32, movement_y: f32) {
        if self.mouse_captured {
            self.pending_mouse_delta[0] += movement_x;
            self.pending_mouse_delta[1] += movement_y;
            return;
        }

        if !self.mouse_dragging {
            return;
        }

        if let Some([last_x, last_y]) = self.last_mouse_position {
            self.pending_mouse_delta[0] += x - last_x;
            self.pending_mouse_delta[1] += y - last_y;
        }
        self.last_mouse_position = Some([x, y]);
    }

    pub fn take_mouse_delta(&mut self) -> [f32; 2] {
        let delta = self.pending_mouse_delta;
        self.pending_mouse_delta = [0.0, 0.0];
        delta
    }

    pub fn push_wheel_delta(&mut self, delta_y: f32) {
        self.pending_wheel_delta += delta_y;
    }

    pub fn take_wheel_delta(&mut self) -> f32 {
        let delta = self.pending_wheel_delta;
        self.pending_wheel_delta = 0.0;
        delta
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
