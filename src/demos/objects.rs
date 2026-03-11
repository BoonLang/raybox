//! Demo 1: 3D Objects Scene
//!
//! Sphere, box, and coffee mug with soft shadows and AO.

use super::{
    world3d_runtime::World3dUniformHost, Demo, DemoContext, DemoId, DemoType, KEYBINDINGS_3D,
};
use crate::camera::{FlyCamera, Uniforms};
use crate::input::CameraConfig;
use crate::shader_bindings::sdf_raymarch;
use anyhow::Result;

pub struct ObjectsDemo {
    host: World3dUniformHost<Uniforms>,
}

impl ObjectsDemo {
    pub fn new(ctx: &DemoContext) -> Result<Self> {
        let shader_module = sdf_raymarch::create_shader_module_embed_source(ctx.device);

        let host =
            World3dUniformHost::new(ctx, "Objects Demo", &shader_module, &Uniforms::default())?;
        Ok(Self { host })
    }
}

impl Demo for ObjectsDemo {
    fn name(&self) -> &'static str {
        "Objects"
    }

    fn id(&self) -> DemoId {
        DemoId::Objects
    }

    fn demo_type(&self) -> DemoType {
        DemoType::World3D
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        KEYBINDINGS_3D
    }

    fn camera_config(&self) -> CameraConfig {
        CameraConfig {
            initial_position: glam::Vec3::new(0.0, 1.5, 5.0),
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
        // Note: uniforms are updated by the runner before calling render
        // We just need to draw here
        self.host.render(render_pass);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.host.resize(width, height);
    }
}

impl ObjectsDemo {
    /// Update uniforms with camera state (called by runner)
    pub fn update_uniforms(&self, queue: &wgpu::Queue, camera: &FlyCamera, time: f32) {
        let mut uniforms = Uniforms::default();
        uniforms.update_from_fly_camera(camera, self.host.width(), self.host.height(), time);
        self.host.write_uniforms(queue, &uniforms);
    }
}
