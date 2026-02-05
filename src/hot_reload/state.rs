//! Reloadable state for hot-reload
//!
//! Serializable state that can be preserved across reloads.

use crate::demo_core::OverlayMode;
use serde::{Deserialize, Serialize};

/// Serializable state that persists across hot-reloads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadableState {
    /// Current demo ID
    pub current_demo: u8,
    /// Camera position
    pub camera_position: [f32; 3],
    /// Camera yaw (radians)
    pub camera_yaw: f32,
    /// Camera pitch (radians)
    pub camera_pitch: f32,
    /// Camera roll (radians)
    pub camera_roll: f32,
    /// Movement speed
    pub move_speed: f32,
    /// Overlay mode
    pub overlay_mode: OverlayModeState,
    /// Whether keybindings are shown
    pub show_keybindings: bool,
    /// Time offset for animation continuity
    pub time_offset: f32,
    /// Window size
    pub window_size: [u32; 2],
}

/// Serializable overlay mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OverlayModeState {
    Off,
    App,
    Full,
}

impl From<OverlayMode> for OverlayModeState {
    fn from(mode: OverlayMode) -> Self {
        match mode {
            OverlayMode::Off => Self::Off,
            OverlayMode::App => Self::App,
            OverlayMode::Full => Self::Full,
        }
    }
}

impl From<OverlayModeState> for OverlayMode {
    fn from(state: OverlayModeState) -> Self {
        match state {
            OverlayModeState::Off => Self::Off,
            OverlayModeState::App => Self::App,
            OverlayModeState::Full => Self::Full,
        }
    }
}

impl Default for ReloadableState {
    fn default() -> Self {
        Self {
            current_demo: 1, // Start with Objects demo
            camera_position: [0.0, 0.0, 4.0],
            camera_yaw: 0.0,
            camera_pitch: 0.0,
            camera_roll: 0.0,
            move_speed: 3.0,
            overlay_mode: OverlayModeState::App,
            show_keybindings: false,
            time_offset: 0.0,
            window_size: [800, 600],
        }
    }
}

impl ReloadableState {
    /// Save state to a JSON file
    pub fn save_to_file(&self, path: &str) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load state from a JSON file
    pub fn load_from_file(path: &str) -> anyhow::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let state: Self = serde_json::from_str(&json)?;
        Ok(state)
    }

    /// Get the default state file path
    pub fn default_path() -> String {
        ".raybox_state.json".to_string()
    }

    /// Try to load from default path, or return default state
    pub fn load_or_default() -> Self {
        Self::load_from_file(&Self::default_path()).unwrap_or_default()
    }

    /// Save to default path
    pub fn save_default(&self) -> anyhow::Result<()> {
        self.save_to_file(&Self::default_path())
    }
}
