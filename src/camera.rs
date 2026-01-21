use glam::{Mat4, Quat, Vec3};

/// Fly camera with game-style WASD + mouse look controls
/// Uses quaternion for orientation - all rotations are camera-relative
pub struct FlyCamera {
    /// World position
    pub position: Vec3,
    /// Camera orientation (quaternion)
    pub orientation: Quat,
    /// Field of view (radians)
    pub fov: f32,
    /// Near clip plane
    pub near: f32,
    /// Far clip plane
    pub far: f32,
    /// Movement speed (units per second)
    pub move_speed: f32,
    /// Mouse look sensitivity
    pub look_sensitivity: f32,
}

impl Default for FlyCamera {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, 4.0),
            orientation: Quat::IDENTITY,
            fov: std::f32::consts::FRAC_PI_4, // 45 degrees
            near: 0.1,
            far: 100.0,
            move_speed: 3.0,
            look_sensitivity: 0.003,
        }
    }
}

#[allow(dead_code)]
impl FlyCamera {
    /// Create a new fly camera
    pub fn new() -> Self {
        Self::default()
    }

    /// Forward direction (where camera looks)
    pub fn forward(&self) -> Vec3 {
        self.orientation * -Vec3::Z
    }

    /// Right direction
    pub fn right(&self) -> Vec3 {
        self.orientation * Vec3::X
    }

    /// Up direction
    pub fn up(&self) -> Vec3 {
        self.orientation * Vec3::Y
    }

    /// Get current position
    pub fn position(&self) -> Vec3 {
        self.position
    }

    /// Compute view matrix (world -> camera space)
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.position + self.forward(), self.up())
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

    /// Handle mouse look - screen-relative using local axis rotations
    ///
    /// Uses pure local axis rotations which naturally produce screen-relative
    /// controls regardless of roll angle. No complex compensation needed.
    ///
    /// Note: Continuous circular mouse movements can accumulate small roll drift.
    /// Press R to reset roll if needed.
    pub fn look(&mut self, dx: f32, dy: f32) {
        // Yaw: rotate around camera's LOCAL up axis
        let yaw_rot = Quat::from_axis_angle(self.up(), -dx * self.look_sensitivity);

        // Pitch: rotate around camera's LOCAL right axis
        let pitch_rot = Quat::from_axis_angle(self.right(), -dy * self.look_sensitivity);

        // Apply both rotations
        self.orientation = (yaw_rot * pitch_rot * self.orientation).normalize();
    }

    /// Roll camera (Q/E keys) - rotates around forward axis
    pub fn roll_camera(&mut self, delta: f32) {
        let roll_rot = Quat::from_axis_angle(self.forward(), delta);
        self.orientation = (roll_rot * self.orientation).normalize();
    }

    /// Reset roll to horizontal (aligns up with world Y)
    pub fn reset_roll(&mut self) {
        // Extract yaw and pitch from current orientation, reset roll
        let forward = self.forward();
        let yaw = forward.x.atan2(-forward.z);
        let pitch = forward.y.asin();
        self.orientation = Quat::from_rotation_y(-yaw) * Quat::from_rotation_x(pitch);
    }

    /// Move forward/backward (W/S keys)
    pub fn move_forward(&mut self, delta_time: f32, forward: bool) {
        let dir = if forward { 1.0 } else { -1.0 };
        self.position += self.forward() * dir * self.move_speed * delta_time;
    }

    /// Strafe left/right (A/D keys) - always horizontal
    pub fn move_right(&mut self, delta_time: f32, right: bool) {
        let dir = if right { 1.0 } else { -1.0 };
        // Project right vector onto horizontal plane for strafing
        let r = self.right();
        let horiz_right = Vec3::new(r.x, 0.0, r.z).normalize_or_zero();
        self.position += horiz_right * dir * self.move_speed * delta_time;
    }

    /// Move up/down (Space/Ctrl keys) - camera-relative
    pub fn move_up(&mut self, delta_time: f32, up: bool) {
        let dir = if up { 1.0 } else { -1.0 };
        self.position += self.up() * dir * self.move_speed * delta_time;
    }

    /// Adjust movement speed (scroll wheel)
    pub fn adjust_speed(&mut self, delta: f32) {
        self.move_speed = (self.move_speed + delta * 0.5).clamp(0.5, 50.0);
    }

    /// Reset camera to default position and orientation
    pub fn reset(&mut self) {
        self.position = Vec3::new(0.0, 0.0, 4.0);
        self.orientation = Quat::IDENTITY;
        self.move_speed = 3.0;
    }

    /// Point the camera at a target position
    pub fn look_at(&mut self, target: Vec3) {
        let dir = (target - self.position).normalize();
        let yaw = dir.x.atan2(-dir.z);
        let pitch = dir.y.asin();
        self.orientation = Quat::from_rotation_y(-yaw) * Quat::from_rotation_x(pitch);
    }

    /// Get yaw angle (for debug display)
    pub fn yaw(&self) -> f32 {
        let forward = self.forward();
        forward.x.atan2(-forward.z)
    }

    /// Get pitch angle (for debug display)
    pub fn pitch(&self) -> f32 {
        self.forward().y.asin()
    }
}

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

    /// Zoom camera (W/S keys or scroll wheel)
    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance - delta).clamp(0.5, 50.0);
    }

    /// Pan camera (move target point) - like middle mouse drag in 3D tools
    /// dx/dy are in screen-space, scaled by distance for consistent feel
    pub fn pan(&mut self, dx: f32, dy: f32) {
        // Compute camera right and up vectors
        let cos_az = self.azimuth.cos();
        let sin_az = self.azimuth.sin();
        let cos_el = self.elevation.cos();
        let sin_el = self.elevation.sin();

        // Right vector (perpendicular to view direction, horizontal)
        let right = Vec3::new(cos_az, 0.0, -sin_az);

        // Up vector (perpendicular to view and right)
        let up = Vec3::new(sin_az * sin_el, cos_el, cos_az * sin_el);

        // Scale pan by distance for consistent feel
        let scale = self.distance * 0.002;
        self.target += right * dx * scale;
        self.target += up * dy * scale;
    }

    /// Reset camera to default position
    pub fn reset(&mut self) {
        self.target = Vec3::ZERO;
        self.distance = 5.0;
        self.azimuth = 0.0;
        self.elevation = 0.3;
    }

    /// Focus on a specific point
    pub fn focus_on(&mut self, point: Vec3) {
        self.target = point;
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
    /// Update uniforms from orbital camera state
    #[allow(dead_code)]
    pub fn update_from_camera(&mut self, camera: &OrbitalCamera, width: u32, height: u32, time: f32) {
        let aspect = width as f32 / height as f32;
        self.inv_view_proj = camera.inv_view_projection_matrix(aspect).to_cols_array_2d();
        let pos = camera.position();
        self.camera_pos_time = [pos.x, pos.y, pos.z, time];
        self.render_params[0] = width as f32;
        self.render_params[1] = height as f32;
    }

    /// Update uniforms from fly camera state
    #[allow(dead_code)]
    pub fn update_from_fly_camera(&mut self, camera: &FlyCamera, width: u32, height: u32, time: f32) {
        let aspect = width as f32 / height as f32;
        self.inv_view_proj = camera.inv_view_projection_matrix(aspect).to_cols_array_2d();
        let pos = camera.position();
        self.camera_pos_time = [pos.x, pos.y, pos.z, time];
        self.render_params[0] = width as f32;
        self.render_params[1] = height as f32;
    }
}
