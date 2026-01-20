use glam::{Mat4, Vec3};

/// Orbital camera that rotates around a target point
pub struct OrbitalCamera {
    /// Target point the camera looks at
    pub target: Vec3,
    /// Distance from target
    pub distance: f32,
    /// Horizontal angle (radians, 0 = looking from +Z)
    pub azimuth: f32,
    /// Vertical angle (radians, 0 = horizontal)
    pub elevation: f32,
    /// Field of view (radians)
    pub fov: f32,
    /// Near clip plane
    pub near: f32,
    /// Far clip plane
    pub far: f32,
}

impl Default for OrbitalCamera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            distance: 5.0,
            azimuth: 0.0,
            elevation: 0.3,
            fov: std::f32::consts::FRAC_PI_4, // 45 degrees
            near: 0.1,
            far: 100.0,
        }
    }
}

#[allow(dead_code)]
impl OrbitalCamera {
    /// Create a new orbital camera
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute camera position in world space
    pub fn position(&self) -> Vec3 {
        let cos_elev = self.elevation.cos();
        let x = self.distance * cos_elev * self.azimuth.sin();
        let y = self.distance * self.elevation.sin();
        let z = self.distance * cos_elev * self.azimuth.cos();
        self.target + Vec3::new(x, y, z)
    }

    /// Compute view matrix (world -> camera space)
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position(), self.target, Vec3::Y)
    }

    /// Compute projection matrix
    pub fn projection_matrix(&self, aspect_ratio: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov, aspect_ratio, self.near, self.far)
    }

    /// Compute combined view-projection matrix
    pub fn view_projection_matrix(&self, aspect_ratio: f32) -> Mat4 {
        self.projection_matrix(aspect_ratio) * self.view_matrix()
    }

    /// Compute inverse view-projection matrix (for raymarching)
    pub fn inv_view_projection_matrix(&self, aspect_ratio: f32) -> Mat4 {
        self.view_projection_matrix(aspect_ratio).inverse()
    }

    /// Rotate camera horizontally (A/D keys)
    pub fn rotate_horizontal(&mut self, delta: f32) {
        self.azimuth += delta;
    }

    /// Rotate camera vertically (with clamping)
    pub fn rotate_vertical(&mut self, delta: f32) {
        self.elevation = (self.elevation + delta).clamp(-1.4, 1.4);
    }

    /// Zoom camera (W/S keys)
    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance - delta).clamp(1.0, 50.0);
    }
}

/// GPU-compatible uniform structure for the camera and rendering parameters
/// Packed for std140 alignment (vec3 fields packed into vec4)
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    /// Inverse view-projection matrix for ray generation
    pub inv_view_proj: [[f32; 4]; 4],
    /// xyz = camera position, w = time
    pub camera_pos_time: [f32; 4],
    /// xyz = light direction (normalized), w = light intensity
    pub light_dir_intensity: [f32; 4],
    /// xy = resolution, z = ao_radius, w = shadow_softness
    pub render_params: [f32; 4],
}

impl Default for Uniforms {
    fn default() -> Self {
        Self {
            inv_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos_time: [0.0, 0.0, 5.0, 0.0],
            light_dir_intensity: [0.577, 0.577, 0.577, 1.0], // normalized (1,1,1), intensity 1
            render_params: [800.0, 600.0, 0.5, 16.0], // resolution, ao_radius, shadow_softness
        }
    }
}

impl Uniforms {
    /// Update uniforms from camera state
    pub fn update_from_camera(&mut self, camera: &OrbitalCamera, width: u32, height: u32, time: f32) {
        let aspect = width as f32 / height as f32;
        self.inv_view_proj = camera.inv_view_projection_matrix(aspect).to_cols_array_2d();
        let pos = camera.position();
        self.camera_pos_time = [pos.x, pos.y, pos.z, time];
        self.render_params[0] = width as f32;
        self.render_params[1] = height as f32;
    }
}
