//! Demo 2: Animated Spheres Scene
//!
//! Grid of bouncing colorful spheres.

use super::{
    world3d_runtime::World3dUniformHost, Demo, DemoContext, DemoId, DemoType, KEYBINDINGS_3D,
};
use crate::camera::FlyCamera;
use crate::input::CameraConfig;
use crate::shader_bindings::sdf_spheres;
use anyhow::Result;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Uniforms {
    inv_view_proj: [[f32; 4]; 4],
    camera_pos_time: [f32; 4],
    light_dir_intensity: [f32; 4],
    render_params: [f32; 4],
}

impl Default for Uniforms {
    fn default() -> Self {
        Self {
            inv_view_proj: [[0.0; 4]; 4],
            camera_pos_time: [0.0, 2.0, 8.0, 0.0],
            light_dir_intensity: [0.577, 0.577, 0.577, 1.0],
            render_params: [800.0, 600.0, 0.5, 16.0],
        }
    }
}

impl Uniforms {
    fn update_from_camera(&mut self, camera: &FlyCamera, width: u32, height: u32, time: f32) {
        let aspect = width as f32 / height as f32;
        self.inv_view_proj = camera.inv_view_projection_matrix(aspect).to_cols_array_2d();
        self.camera_pos_time = [
            camera.position().x,
            camera.position().y,
            camera.position().z,
            time,
        ];
        self.render_params[0] = width as f32;
        self.render_params[1] = height as f32;
    }
}

pub struct SpheresDemo {
    host: World3dUniformHost<Uniforms>,
}

impl SpheresDemo {
    pub fn new(ctx: &DemoContext) -> Result<Self> {
        let shader_module = sdf_spheres::create_shader_module_embed_source(ctx.device);

        let host =
            World3dUniformHost::new(ctx, "Spheres Demo", &shader_module, &Uniforms::default())?;
        Ok(Self { host })
    }

    pub fn update_uniforms(&self, queue: &wgpu::Queue, camera: &FlyCamera, time: f32) {
        let mut uniforms = Uniforms::default();
        uniforms.update_from_camera(camera, self.host.width(), self.host.height(), time);
        self.host.write_uniforms(queue, &uniforms);
    }
}

impl Demo for SpheresDemo {
    fn name(&self) -> &'static str {
        "Spheres"
    }

    fn id(&self) -> DemoId {
        DemoId::Spheres
    }

    fn demo_type(&self) -> DemoType {
        DemoType::World3D
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        KEYBINDINGS_3D
    }

    fn camera_config(&self) -> CameraConfig {
        CameraConfig {
            initial_position: glam::Vec3::new(0.0, 2.0, 8.0),
            look_at_target: glam::Vec3::new(0.0, 0.5, 0.0),
        }
    }

    fn update(&mut self, _dt: f32, _camera: &mut FlyCamera) {
        // No updates needed
    }

    fn update_camera_uniforms(&self, queue: &wgpu::Queue, camera: &FlyCamera, time: f32) {
        self.update_uniforms(queue, camera, time);
    }

    fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        _queue: &wgpu::Queue,
        _time: f32,
    ) {
        self.host.render(render_pass);
    }

    fn wants_continuous_redraw(&self) -> bool {
        true
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.host.resize(width, height);
    }
}
