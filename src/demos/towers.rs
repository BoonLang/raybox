//! Demo 3: Towers Scene
//!
//! Abstract cityscape with randomly-sized towers.

use super::{
    world3d_runtime::World3dUniformHost, Demo, DemoContext, DemoId, DemoType, KEYBINDINGS_3D,
};
use crate::camera::{towers_uniforms_from_fly, FlyCamera};
use crate::input::CameraConfig;
use crate::shader_bindings::sdf_towers;
use anyhow::Result;

pub struct TowersDemo {
    host: World3dUniformHost<sdf_towers::Uniforms_std140_0>,
}

impl TowersDemo {
    pub fn new(ctx: &DemoContext) -> Result<Self> {
        let shader_module = sdf_towers::create_shader_module_embed_source(ctx.device);

        let uniforms = towers_uniforms_from_fly(&FlyCamera::default(), ctx.width, ctx.height, 0.0);
        let host = World3dUniformHost::new(ctx, "Towers Demo", &shader_module, &uniforms)?;
        Ok(Self { host })
    }

    pub fn update_uniforms(&self, queue: &wgpu::Queue, camera: &FlyCamera, time: f32) {
        let uniforms =
            towers_uniforms_from_fly(camera, self.host.width(), self.host.height(), time);
        self.host.write_uniforms(queue, &uniforms);
    }
}

impl Demo for TowersDemo {
    fn name(&self) -> &'static str {
        "Towers"
    }

    fn id(&self) -> DemoId {
        DemoId::Towers
    }

    fn demo_type(&self) -> DemoType {
        DemoType::World3D
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        KEYBINDINGS_3D
    }

    fn camera_config(&self) -> CameraConfig {
        CameraConfig {
            initial_position: glam::Vec3::new(4.0, 6.0, 10.0),
            look_at_target: glam::Vec3::new(0.0, 0.0, 0.0),
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

    fn resize(&mut self, width: u32, height: u32) {
        self.host.resize(width, height);
    }
}
