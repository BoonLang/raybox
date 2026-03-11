//! Demo 0: Empty scene (baseline)
//!
//! Renders a simple gradient background with minimal GPU resources.
//! Used as a baseline for comparison and performance testing.

use super::{
    world3d_runtime::World3dUniformHost, Demo, DemoContext, DemoId, DemoType, KEYBINDINGS_3D,
};
use crate::camera::FlyCamera;
use crate::input::CameraConfig;
use anyhow::Result;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Uniforms {
    resolution: [f32; 2],
    time: f32,
    _padding: f32,
}

pub struct EmptyDemo {
    host: World3dUniformHost<Uniforms>,
}

impl EmptyDemo {
    pub fn new(ctx: &DemoContext) -> Result<Self> {
        let shader_source = r#"
struct Uniforms {
    resolution: vec2<f32>,
    time: f32,
    _padding: f32,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Fullscreen triangle
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );
    var out: VertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = positions[vertex_index] * 0.5 + 0.5;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Simple animated gradient
    let t = uniforms.time * 0.1;
    let grad = mix(
        vec3<f32>(0.05, 0.05, 0.1),  // Dark blue
        vec3<f32>(0.1, 0.05, 0.15),   // Dark purple
        in.uv.y + sin(t) * 0.1
    );
    return vec4<f32>(grad, 1.0);
}
"#;

        let shader = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Empty Demo Shader"),
                source: wgpu::ShaderSource::Wgsl(shader_source.into()),
            });

        let uniforms = Uniforms {
            resolution: [ctx.width as f32, ctx.height as f32],
            time: 0.0,
            _padding: 0.0,
        };
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
        let uniforms = Uniforms {
            resolution: [self.host.width() as f32, self.host.height() as f32],
            time,
            _padding: 0.0,
        };
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
