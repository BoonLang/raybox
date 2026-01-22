//! Demo 0: Empty scene (baseline)
//!
//! Renders a simple gradient background with minimal GPU resources.
//! Used as a baseline for comparison and performance testing.

use super::{Demo, DemoContext, DemoId, DemoType, KEYBINDINGS_3D};
use std::any::Any;
use crate::camera::FlyCamera;
use crate::input::CameraConfig;
use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Uniforms {
    resolution: [f32; 2],
    time: f32,
    _padding: f32,
}

pub struct EmptyDemo {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
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

        let shader = ctx.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Empty Demo Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let uniforms = Uniforms {
            resolution: [ctx.width as f32, ctx.height as f32],
            time: 0.0,
            _padding: 0.0,
        };

        let uniform_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Empty Demo Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = ctx.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Empty Demo Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Empty Demo Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = ctx.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Empty Demo Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = ctx.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Empty Demo Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: ctx.surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Ok(Self {
            pipeline,
            uniform_buffer,
            bind_group,
            width: ctx.width,
            height: ctx.height,
        })
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
        DemoType::Scene3D
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

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue, time: f32) {
        let uniforms = Uniforms {
            resolution: [self.width as f32, self.height as f32],
            time,
            _padding: 0.0,
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
