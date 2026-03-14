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

fn spheres_uniforms(
    camera: &FlyCamera,
    width: u32,
    height: u32,
    time: f32,
) -> sdf_spheres::Uniforms_std140_0 {
    let aspect = width as f32 / height as f32;
    let position = camera.position();
    sdf_spheres::Uniforms_std140_0::new(
        sdf_spheres::_MatrixStorage_float4x4_ColMajorstd140_0::new(
            camera.inv_view_projection_matrix(aspect).to_cols_array_2d(),
        ),
        [position.x, position.y, position.z, time],
        [0.577, 0.577, 0.577, 1.0],
        [width as f32, height as f32, 0.5, 16.0],
    )
}

pub struct SpheresDemo {
    host: World3dUniformHost<sdf_spheres::Uniforms_std140_0>,
}

impl SpheresDemo {
    pub fn new(ctx: &DemoContext) -> Result<Self> {
        let shader_module = sdf_spheres::create_shader_module_embed_source(ctx.device);

        let uniforms = spheres_uniforms(&FlyCamera::default(), ctx.width, ctx.height, 0.0);
        let host = World3dUniformHost::new(ctx, "Spheres Demo", &shader_module, &uniforms)?;
        Ok(Self { host })
    }

    pub fn update_uniforms(&self, queue: &wgpu::Queue, camera: &FlyCamera, time: f32) {
        let uniforms = spheres_uniforms(camera, self.host.width(), self.host.height(), time);
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
