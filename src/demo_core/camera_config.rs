//! Camera configuration for demos
//!
//! Platform-agnostic camera configuration that works on both native and web.

use glam::Vec3;
use serde::{Deserialize, Serialize};

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
