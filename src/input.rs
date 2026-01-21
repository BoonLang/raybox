//! Shared input handling for windowed examples
//!
//! Standardized controls:
//! - WASD: Movement (forward/back/strafe)
//! - Space: Move up (camera-relative)
//! - Ctrl: Move down (camera-relative)
//! - Q/E: Roll camera
//! - R: Reset roll to horizontal
//! - T: Reset camera to initial position
//! - Tab: Toggle mouse capture
//! - Scroll: Adjust movement speed
//! - F: Toggle FPS/debug overlay (via window title)
//! - Esc: Release capture (if captured), else exit

use crate::camera::FlyCamera;
use glam::Vec3;
use std::collections::HashSet;
use std::collections::VecDeque;
use winit::{
    event::{ElementState, KeyEvent, MouseScrollDelta},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window},
};

/// Camera configuration for initial position and orientation
#[derive(Clone, Debug)]
pub struct CameraConfig {
    pub initial_position: Vec3,
    pub look_at_target: Vec3,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            initial_position: Vec3::new(0.0, 0.0, 4.0),
            look_at_target: Vec3::ZERO,
        }
    }
}

/// Actions that require special handling from the application
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputAction {
    Exit,
    ToggleCapture,
    ToggleDebugOverlay,
    ResetRoll,
    ResetCamera,
}

/// Shared input handler for all 3D examples
pub struct InputHandler {
    pub pressed_keys: HashSet<KeyCode>,
    pub mouse_captured: bool,
    pub show_debug_overlay: bool,
    config: CameraConfig,
    frame_times: VecDeque<f32>,
    last_fps: f32,
}

impl InputHandler {
    /// Create a new input handler with the given camera configuration
    pub fn new(config: CameraConfig) -> Self {
        Self {
            pressed_keys: HashSet::new(),
            mouse_captured: false,
            show_debug_overlay: false,
            config,
            frame_times: VecDeque::with_capacity(60),
            last_fps: 0.0,
        }
    }

    /// Initialize camera with the configured position and orientation
    pub fn setup_camera(&self, camera: &mut FlyCamera) {
        camera.position = self.config.initial_position;
        camera.look_at(self.config.look_at_target);
    }

    /// Handle a key event, returning an action if special handling is needed
    pub fn handle_key(&mut self, event: KeyEvent) -> Option<InputAction> {
        let PhysicalKey::Code(key_code) = event.physical_key else {
            return None;
        };

        match event.state {
            ElementState::Pressed => {
                self.pressed_keys.insert(key_code);
            }
            ElementState::Released => {
                self.pressed_keys.remove(&key_code);
                return None; // Only handle press actions
            }
        }

        // Handle special keys on press
        match key_code {
            KeyCode::Escape => {
                if self.mouse_captured {
                    Some(InputAction::ToggleCapture)
                } else {
                    Some(InputAction::Exit)
                }
            }
            KeyCode::Tab => Some(InputAction::ToggleCapture),
            KeyCode::KeyF => Some(InputAction::ToggleDebugOverlay),
            KeyCode::KeyR => Some(InputAction::ResetRoll),
            KeyCode::KeyT | KeyCode::Home => Some(InputAction::ResetCamera),
            _ => None,
        }
    }

    /// Handle mouse motion when captured
    pub fn handle_mouse_motion(&self, camera: &mut FlyCamera, delta: (f64, f64)) {
        if self.mouse_captured {
            camera.look(delta.0 as f32, delta.1 as f32);
        }
    }

    /// Handle scroll wheel for speed adjustment
    pub fn handle_scroll(&self, camera: &mut FlyCamera, delta: MouseScrollDelta) {
        let scroll = match delta {
            MouseScrollDelta::LineDelta(_, y) => y,
            MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.02,
        };
        camera.adjust_speed(scroll);
    }

    /// Update camera based on currently pressed keys
    pub fn update_camera(&self, camera: &mut FlyCamera, dt: f32) {
        // WASD movement
        if self.pressed_keys.contains(&KeyCode::KeyW) {
            camera.move_forward(dt, true);
        }
        if self.pressed_keys.contains(&KeyCode::KeyS) {
            camera.move_forward(dt, false);
        }
        if self.pressed_keys.contains(&KeyCode::KeyA) {
            camera.move_right(dt, false);
        }
        if self.pressed_keys.contains(&KeyCode::KeyD) {
            camera.move_right(dt, true);
        }

        // Up/Down movement (Space/Ctrl)
        if self.pressed_keys.contains(&KeyCode::Space) {
            camera.move_up(dt, true);
        }
        if self.pressed_keys.contains(&KeyCode::ControlLeft)
            || self.pressed_keys.contains(&KeyCode::ControlRight)
        {
            camera.move_up(dt, false);
        }

        // Roll (Q/E)
        if self.pressed_keys.contains(&KeyCode::KeyQ) {
            camera.roll_camera(-dt * 2.0);
        }
        if self.pressed_keys.contains(&KeyCode::KeyE) {
            camera.roll_camera(dt * 2.0);
        }
    }

    /// Reset roll to horizontal
    pub fn reset_roll(&self, camera: &mut FlyCamera) {
        camera.reset_roll();
    }

    /// Reset camera to initial configuration
    pub fn reset_camera(&self, camera: &mut FlyCamera) {
        camera.reset();
        camera.position = self.config.initial_position;
        camera.look_at(self.config.look_at_target);
    }

    /// Toggle mouse capture state
    pub fn toggle_capture(&mut self, window: &Window) {
        self.mouse_captured = !self.mouse_captured;
        if self.mouse_captured {
            // Try Locked first (best for FPS), fall back to Confined
            if window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                let _ = window.set_cursor_grab(CursorGrabMode::Confined);
            }
            window.set_cursor_visible(false);
        } else {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
            window.set_cursor_visible(true);
        }
    }

    /// Toggle debug overlay visibility
    pub fn toggle_debug_overlay(&mut self) {
        self.show_debug_overlay = !self.show_debug_overlay;
    }

    /// Update frame timing for FPS calculation
    pub fn update_frame_time(&mut self, dt: f32) {
        self.frame_times.push_back(dt);
        if self.frame_times.len() > 60 {
            self.frame_times.pop_front();
        }

        // Calculate FPS from average frame time
        if !self.frame_times.is_empty() {
            let avg_dt: f32 = self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32;
            self.last_fps = 1.0 / avg_dt;
        }
    }

    /// Update window title with debug info if overlay is enabled
    pub fn update_window_title(&self, window: &Window, base_title: &str, camera: &FlyCamera) {
        if self.show_debug_overlay {
            let pos = camera.position;
            let yaw_deg = camera.yaw.to_degrees();
            let pitch_deg = camera.pitch.to_degrees();
            let title = format!(
                "{} | FPS: {:.0} | Pos: ({:.1}, {:.1}, {:.1}) | Yaw: {:.0} Pitch: {:.0} | Speed: {:.1}",
                base_title, self.last_fps, pos.x, pos.y, pos.z, yaw_deg, pitch_deg, camera.move_speed
            );
            window.set_title(&title);
        } else {
            window.set_title(base_title);
        }
    }
}

/// Standard window title suffix for all 3D examples
pub const CONTROLS_HINT: &str = "WASD+Space/Ctrl, Q/E, Mouse, Tab, Scroll, R/T, F, Esc";

/// Generate a standard window title for a demo
pub fn demo_title(demo_num: u8, name: &str) -> String {
    format!("Demo {}: {} | {}", demo_num, name, CONTROLS_HINT)
}
