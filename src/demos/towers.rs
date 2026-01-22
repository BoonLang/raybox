//! Demo 3: Towers Scene
//!
//! Abstract cityscape with randomly-sized towers.

use super::{Demo, DemoContext, DemoId, DemoType, KEYBINDINGS_3D};
use std::any::Any;
use crate::camera::{FlyCamera, Uniforms};
use crate::input::CameraConfig;
use crate::shader_bindings::sdf_towers;
use anyhow::Result;
use wgpu::util::DeviceExt;

pub struct TowersDemo {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

impl TowersDemo {
    pub fn new(ctx: &DemoContext) -> Result<Self> {
        let shader_module = sdf_towers::create_shader_module_embed_source(ctx.device);

        let uniforms = Uniforms::default();
        let uniform_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Towers Demo Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = ctx.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Towers Demo Bind Group Layout"),
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
            label: Some("Towers Demo Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = ctx.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Towers Demo Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = ctx.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Towers Demo Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
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

    pub fn update_uniforms(&self, queue: &wgpu::Queue, camera: &FlyCamera, time: f32) {
        let mut uniforms = Uniforms::default();
        uniforms.update_from_fly_camera(camera, self.width, self.height, time);
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
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
        DemoType::Scene3D
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

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, _queue: &wgpu::Queue, _time: f32) {
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
