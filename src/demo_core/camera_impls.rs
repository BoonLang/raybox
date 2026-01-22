//! CameraController implementations for camera types
//!
//! This is kept in demo_core to ensure it's only compiled as part of the library,
//! not in binary crates like main.rs that include camera.rs directly.

use super::CameraController;
use crate::camera::{FlyCamera, OrbitalCamera};
use glam::{Mat4, Vec3};

impl CameraController for FlyCamera {
    fn position(&self) -> Vec3 {
        self.position
    }

    fn set_position(&mut self, pos: Vec3) {
        self.position = pos;
    }

    fn get_yaw(&self) -> f32 {
        self.yaw
    }

    fn get_pitch(&self) -> f32 {
        self.pitch
    }

    fn get_roll(&self) -> f32 {
        self.roll
    }

    fn forward(&self) -> Vec3 {
        FlyCamera::forward(self)
    }

    fn right(&self) -> Vec3 {
        FlyCamera::right(self)
    }

    fn up(&self) -> Vec3 {
        FlyCamera::up(self)
    }

    fn view_matrix(&self) -> Mat4 {
        FlyCamera::view_matrix(self)
    }

    fn inv_view_projection_matrix(&self, aspect_ratio: f32) -> Mat4 {
        FlyCamera::inv_view_projection_matrix(self, aspect_ratio)
    }
}

impl CameraController for OrbitalCamera {
    fn position(&self) -> Vec3 {
        OrbitalCamera::position(self)
    }

    fn set_position(&mut self, pos: Vec3) {
        // For orbital camera, setting position adjusts distance
        self.distance = (pos - self.target).length();
    }

    fn get_yaw(&self) -> f32 {
        self.azimuth
    }

    fn get_pitch(&self) -> f32 {
        self.elevation
    }

    fn get_roll(&self) -> f32 {
        0.0 // Orbital camera doesn't support roll
    }

    fn forward(&self) -> Vec3 {
        (self.target - OrbitalCamera::position(self)).normalize()
    }

    fn right(&self) -> Vec3 {
        let cos_az = self.azimuth.cos();
        let sin_az = self.azimuth.sin();
        Vec3::new(cos_az, 0.0, -sin_az)
    }

    fn up(&self) -> Vec3 {
        Vec3::Y // Orbital camera always has world Y as up
    }

    fn view_matrix(&self) -> Mat4 {
        OrbitalCamera::view_matrix(self)
    }

    fn inv_view_projection_matrix(&self, aspect_ratio: f32) -> Mat4 {
        OrbitalCamera::inv_view_projection_matrix(self, aspect_ratio)
    }
}
