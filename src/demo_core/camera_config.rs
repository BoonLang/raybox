//! Camera configuration for demos
//!
//! Platform-agnostic camera configuration that works on both native and web.

use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UiPhysicalCameraPreset {
    pub fallback_offset: Vec3,
    pub min_distance: f32,
    pub max_distance: f32,
    pub min_elevation: f32,
    pub max_elevation: f32,
    pub clamp_x: f32,
    pub min_height: f32,
    pub max_height: f32,
    pub clamp_z: f32,
}

impl Default for UiPhysicalCameraPreset {
    fn default() -> Self {
        Self {
            fallback_offset: Vec3::new(0.0, 6.5, 7.0),
            min_distance: 4.0,
            max_distance: 16.0,
            min_elevation: -0.95,
            max_elevation: 0.45,
            clamp_x: 8.0,
            min_height: 1.5,
            max_height: 14.0,
            clamp_z: 12.0,
        }
    }
}

pub fn ui_physical_card_camera_preset(card_size: [f32; 2]) -> UiPhysicalCameraPreset {
    let max_dim = card_size[0].max(card_size[1]).max(1.0);
    let scale = (max_dim / 360.0).clamp(0.7, 2.0);
    UiPhysicalCameraPreset {
        fallback_offset: Vec3::new(0.0, 5.4 * scale + 0.8, 6.2 * scale + 0.8),
        min_distance: 3.2 * scale + 0.6,
        max_distance: 9.0 * scale + 1.5,
        min_elevation: -0.85,
        max_elevation: 0.35,
        clamp_x: 4.8 * scale + 0.8,
        min_height: 1.5,
        max_height: 7.4 * scale + 1.2,
        clamp_z: 6.6 * scale + 1.0,
    }
}

/// Camera configuration for initial position and orientation
#[derive(Clone, Debug, Serialize, Deserialize)]
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

impl CameraConfig {
    pub fn new(initial_position: Vec3, look_at_target: Vec3) -> Self {
        Self {
            initial_position,
            look_at_target,
        }
    }
}

pub fn ui_physical_card_camera_config(card_size: [f32; 2]) -> CameraConfig {
    let preset = ui_physical_card_camera_preset(card_size);
    CameraConfig::new(preset.fallback_offset, Vec3::ZERO)
}
