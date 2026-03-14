//! Demo 0: Empty scene (baseline)
//!
//! Renders a simple gradient background with minimal GPU resources.
//! Used as a baseline for comparison and performance testing.

use super::{
    world3d_runtime::World3dUniformHost, Demo, DemoContext, DemoId, DemoType, KEYBINDINGS_3D,
};
use crate::camera::FlyCamera;
use crate::input::CameraConfig;
use crate::shader_bindings::empty;
use anyhow::Result;

pub struct EmptyDemo {
    host: World3dUniformHost<empty::Uniforms_std140_0>,
}

fn empty_uniforms(width: u32, height: u32, time: f32) -> empty::Uniforms_std140_0 {
    empty::Uniforms_std140_0::new([width as f32, height as f32], time, 0.0)
}

impl EmptyDemo {
    pub fn new(ctx: &DemoContext) -> Result<Self> {
        let shader = empty::create_shader_module_embed_source(ctx.device);
        let uniforms = empty_uniforms(ctx.width, ctx.height, 0.0);
        let host = World3dUniformHost::new(ctx, "Empty Demo", &shader, &uniforms)?;
        Ok(Self { host })
    }
}

impl Demo for EmptyDemo {
    fn name(&self) -> &'static str {
        "Empty"
    }

    fn id(&self) -> DemoId {
        DemoId::Empty
    }

    fn demo_type(&self) -> DemoType {
        DemoType::World3D
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        KEYBINDINGS_3D
    }

    fn camera_config(&self) -> CameraConfig {
        CameraConfig::default()
    }

    fn update(&mut self, _dt: f32, _camera: &mut FlyCamera) {
        // No updates needed
    }

    fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        time: f32,
    ) {
        let uniforms = empty_uniforms(self.host.width(), self.host.height(), time);
        self.host.write_uniforms(queue, &uniforms);
        self.host.render(render_pass);
    }

    fn wants_continuous_redraw(&self) -> bool {
        true
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.host.resize(width, height);
    }
}
