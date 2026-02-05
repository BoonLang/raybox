//! Web (WASM) version of the unified demo system
//!
//! Supports all 7 demos with switching (0-6), overlay, and keybindings (K).
//! Uses OrbitalCamera for simpler 3D navigation on web.
//! Optionally connects to control server when ?control=1 URL parameter is present.
//! Supports hot-reload via WASM module reloading with state preservation.

#![allow(dead_code)]

use crate::camera::{OrbitalCamera, Uniforms};
use crate::constants::{HEIGHT, WIDTH};
use crate::demo_core::{DemoId, DemoType, OverlayMode, KEYBINDINGS_COMMON};
use crate::shader_bindings::{
    sdf_clay_vector, sdf_raymarch, sdf_spheres, sdf_text2d_vector, sdf_text_shadow_vector,
    sdf_towers,
};
use crate::web_control::{
    self, SharedWebControlState, WebCommand, WebWsClient,
    error_response, pong_response, status_response, success_response,
};
use crate::web_input::WebInputHandler;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wgpu::util::DeviceExt;

// Global state for hot-reload preservation
thread_local! {
    static SAVED_STATE: RefCell<Option<WebReloadableState>> = const { RefCell::new(None) };
    static RENDERER_REF: RefCell<Option<Rc<RefCell<WebRenderer>>>> = const { RefCell::new(None) };
}

/// Serializable state for web hot-reload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebReloadableState {
    pub current_demo: u8,
    pub camera_distance: f32,
    pub camera_azimuth: f32,
    pub camera_elevation: f32,
    pub camera_target: [f32; 3],
    pub overlay_mode: String,
    pub show_keybindings: bool,
    pub text2d_offset: [f32; 2],
    pub text2d_scale: f32,
    pub text2d_rotation: f32,
    pub time_offset: f32,
}

impl Default for WebReloadableState {
    fn default() -> Self {
        Self {
            current_demo: 1,
            camera_distance: 5.0,
            camera_azimuth: 0.0,
            camera_elevation: 0.0,
            camera_target: [0.0, 0.0, 0.0],
            overlay_mode: "off".to_string(),
            show_keybindings: false,
            text2d_offset: [0.0, 0.0],
            text2d_scale: 1.0,
            text2d_rotation: 0.0,
            time_offset: 0.0,
        }
    }
}

/// Web demo implementation (simplified version of native Demo trait)
trait WebDemo {
    fn name(&self) -> &'static str;
    fn id(&self) -> DemoId;
    fn demo_type(&self) -> DemoType;
    fn keybindings(&self) -> &'static [(&'static str, &'static str)];
    fn camera_config(&self) -> (glam::Vec3, glam::Vec3); // (position, target)
    fn update(&mut self, dt: f32);
    fn update_uniforms(&self, queue: &wgpu::Queue, camera: &OrbitalCamera, time: f32);
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>);
    fn resize(&mut self, width: u32, height: u32);
}

// Standard 3D keybindings for web (orbital camera)
const KEYBINDINGS_3D_WEB: &[(&str, &str)] = &[
    ("A/D", "Rotate horizontal"),
    ("W/S", "Zoom"),
    ("Q/E", "Rotate vertical"),
];

// Standard 2D keybindings for web
const KEYBINDINGS_2D_WEB: &[(&str, &str)] = &[
    ("A/D", "Pan horizontal"),
    ("W/S", "Pan vertical"),
    ("Arrows", "Zoom"),
    ("Q/E", "Rotate"),
];

// ============== EMPTY DEMO ==============

struct EmptyDemo {
    pipeline: wgpu::RenderPipeline,
    width: u32,
    height: u32,
}

impl EmptyDemo {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        // Simple pass-through shader for empty scene
        let shader_module = sdf_raymarch::create_shader_module_embed_source(device);

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Empty Bind Group Layout"),
            entries: &[],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Empty Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Empty Pipeline"),
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
                    format,
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

        Self {
            pipeline,
            width: WIDTH,
            height: HEIGHT,
        }
    }
}

impl WebDemo for EmptyDemo {
    fn name(&self) -> &'static str {
        "Empty"
    }
    fn id(&self) -> DemoId {
        DemoId::Empty
    }
    fn demo_type(&self) -> DemoType {
        DemoType::Scene3D
    }
    fn keybindings(&self) -> &'static [(&'static str, &'static str)] {
        KEYBINDINGS_3D_WEB
    }
    fn camera_config(&self) -> (glam::Vec3, glam::Vec3) {
        (glam::Vec3::new(0.0, 0.0, 5.0), glam::Vec3::ZERO)
    }
    fn update(&mut self, _dt: f32) {}
    fn update_uniforms(&self, _queue: &wgpu::Queue, _camera: &OrbitalCamera, _time: f32) {}
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        // Don't draw anything - just show the clear color
    }
    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
}

// ============== OBJECTS DEMO (sdf_raymarch) ==============

struct ObjectsDemo {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

impl ObjectsDemo {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader_module = sdf_raymarch::create_shader_module_embed_source(device);

        let uniforms = Uniforms::default();
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Objects Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Objects Bind Group Layout"),
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Objects Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Objects Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Objects Pipeline"),
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
                    format,
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

        Self {
            pipeline,
            uniform_buffer,
            bind_group,
            width: WIDTH,
            height: HEIGHT,
        }
    }
}

impl WebDemo for ObjectsDemo {
    fn name(&self) -> &'static str {
        "Objects"
    }
    fn id(&self) -> DemoId {
        DemoId::Objects
    }
    fn demo_type(&self) -> DemoType {
        DemoType::Scene3D
    }
    fn keybindings(&self) -> &'static [(&'static str, &'static str)] {
        KEYBINDINGS_3D_WEB
    }
    fn camera_config(&self) -> (glam::Vec3, glam::Vec3) {
        (glam::Vec3::new(0.0, 1.5, 5.0), glam::Vec3::new(0.0, 0.5, 0.0))
    }
    fn update(&mut self, _dt: f32) {}
    fn update_uniforms(&self, queue: &wgpu::Queue, camera: &OrbitalCamera, time: f32) {
        let mut uniforms = Uniforms::default();
        uniforms.update_from_camera(camera, self.width, self.height, time);
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
}

// ============== SPHERES DEMO ==============

struct SpheresDemo {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

impl SpheresDemo {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader_module = sdf_spheres::create_shader_module_embed_source(device);

        let uniforms = Uniforms::default();
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Spheres Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Spheres Bind Group Layout"),
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Spheres Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Spheres Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Spheres Pipeline"),
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
                    format,
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

        Self {
            pipeline,
            uniform_buffer,
            bind_group,
            width: WIDTH,
            height: HEIGHT,
        }
    }
}

impl WebDemo for SpheresDemo {
    fn name(&self) -> &'static str {
        "Spheres"
    }
    fn id(&self) -> DemoId {
        DemoId::Spheres
    }
    fn demo_type(&self) -> DemoType {
        DemoType::Scene3D
    }
    fn keybindings(&self) -> &'static [(&'static str, &'static str)] {
        KEYBINDINGS_3D_WEB
    }
    fn camera_config(&self) -> (glam::Vec3, glam::Vec3) {
        (glam::Vec3::new(0.0, 2.0, 6.0), glam::Vec3::new(0.0, 0.0, 0.0))
    }
    fn update(&mut self, _dt: f32) {}
    fn update_uniforms(&self, queue: &wgpu::Queue, camera: &OrbitalCamera, time: f32) {
        let mut uniforms = Uniforms::default();
        uniforms.update_from_camera(camera, self.width, self.height, time);
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
}

// ============== TOWERS DEMO ==============

struct TowersDemo {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

impl TowersDemo {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader_module = sdf_towers::create_shader_module_embed_source(device);

        let uniforms = Uniforms::default();
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Towers Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Towers Bind Group Layout"),
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Towers Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Towers Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Towers Pipeline"),
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
                    format,
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

        Self {
            pipeline,
            uniform_buffer,
            bind_group,
            width: WIDTH,
            height: HEIGHT,
        }
    }
}

impl WebDemo for TowersDemo {
    fn name(&self) -> &'static str {
        "Towers"
    }
    fn id(&self) -> DemoId {
        DemoId::Towers
    }
    fn demo_type(&self) -> DemoType {
        DemoType::Scene3D
    }
    fn keybindings(&self) -> &'static [(&'static str, &'static str)] {
        KEYBINDINGS_3D_WEB
    }
    fn camera_config(&self) -> (glam::Vec3, glam::Vec3) {
        (
            glam::Vec3::new(0.0, 3.0, 8.0),
            glam::Vec3::new(0.0, 1.0, 0.0),
        )
    }
    fn update(&mut self, _dt: f32) {}
    fn update_uniforms(&self, queue: &wgpu::Queue, camera: &OrbitalCamera, time: f32) {
        let mut uniforms = Uniforms::default();
        uniforms.update_from_camera(camera, self.width, self.height, time);
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
}

// ============== TEXT2D DEMO ==============
// Note: The Text2D demo uses a vector text renderer which requires storage buffers.
// WebGL2 may not support this. For now, we use the same shader pattern.

struct Text2DDemo {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    offset: [f32; 2],
    scale: f32,
    rotation: f32,
    width: u32,
    height: u32,
}

impl Text2DDemo {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        // Use the text2d vector shader
        let shader_module = sdf_text2d_vector::create_shader_module_embed_source(device);

        // Create uniform buffer with 2D transform data
        let uniform_data: [f32; 8] = [
            0.0, 0.0, // offset
            1.0, 0.0, // scale, rotation
            WIDTH as f32,
            HEIGHT as f32, // resolution
            0.0, 0.0, // padding
        ];
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Text2D Uniform Buffer"),
            contents: bytemuck::cast_slice(&uniform_data),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Text2D Bind Group Layout"),
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Text2D Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Text2D Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Text2D Pipeline"),
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
                    format,
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

        Self {
            pipeline,
            uniform_buffer,
            bind_group,
            offset: [0.0, 0.0],
            scale: 1.0,
            rotation: 0.0,
            width: WIDTH,
            height: HEIGHT,
        }
    }
}

impl WebDemo for Text2DDemo {
    fn name(&self) -> &'static str {
        "2D Text"
    }
    fn id(&self) -> DemoId {
        DemoId::Text2D
    }
    fn demo_type(&self) -> DemoType {
        DemoType::Scene2D
    }
    fn keybindings(&self) -> &'static [(&'static str, &'static str)] {
        KEYBINDINGS_2D_WEB
    }
    fn camera_config(&self) -> (glam::Vec3, glam::Vec3) {
        (glam::Vec3::ZERO, glam::Vec3::ZERO)
    }
    fn update(&mut self, _dt: f32) {}
    fn update_uniforms(&self, queue: &wgpu::Queue, _camera: &OrbitalCamera, _time: f32) {
        let uniform_data: [f32; 8] = [
            self.offset[0],
            self.offset[1],
            self.scale,
            self.rotation,
            self.width as f32,
            self.height as f32,
            0.0,
            0.0,
        ];
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&uniform_data));
    }
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
}

// ============== CLAY DEMO ==============

struct ClayDemo {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

impl ClayDemo {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader_module = sdf_clay_vector::create_shader_module_embed_source(device);

        let uniforms = Uniforms::default();
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Clay Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Clay Bind Group Layout"),
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Clay Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Clay Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Clay Pipeline"),
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
                    format,
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

        Self {
            pipeline,
            uniform_buffer,
            bind_group,
            width: WIDTH,
            height: HEIGHT,
        }
    }
}

impl WebDemo for ClayDemo {
    fn name(&self) -> &'static str {
        "Clay Tablet"
    }
    fn id(&self) -> DemoId {
        DemoId::Clay
    }
    fn demo_type(&self) -> DemoType {
        DemoType::Scene3D
    }
    fn keybindings(&self) -> &'static [(&'static str, &'static str)] {
        KEYBINDINGS_3D_WEB
    }
    fn camera_config(&self) -> (glam::Vec3, glam::Vec3) {
        (glam::Vec3::new(0.0, 0.0, 2.0), glam::Vec3::ZERO)
    }
    fn update(&mut self, _dt: f32) {}
    fn update_uniforms(&self, queue: &wgpu::Queue, camera: &OrbitalCamera, time: f32) {
        let mut uniforms = Uniforms::default();
        uniforms.update_from_camera(camera, self.width, self.height, time);
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
}

// ============== TEXT SHADOW DEMO ==============

struct TextShadowDemo {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

impl TextShadowDemo {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader_module = sdf_text_shadow_vector::create_shader_module_embed_source(device);

        let uniforms = Uniforms::default();
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("TextShadow Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TextShadow Bind Group Layout"),
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TextShadow Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("TextShadow Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("TextShadow Pipeline"),
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
                    format,
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

        Self {
            pipeline,
            uniform_buffer,
            bind_group,
            width: WIDTH,
            height: HEIGHT,
        }
    }
}

impl WebDemo for TextShadowDemo {
    fn name(&self) -> &'static str {
        "Text Shadow"
    }
    fn id(&self) -> DemoId {
        DemoId::TextShadow
    }
    fn demo_type(&self) -> DemoType {
        DemoType::Scene3D
    }
    fn keybindings(&self) -> &'static [(&'static str, &'static str)] {
        KEYBINDINGS_3D_WEB
    }
    fn camera_config(&self) -> (glam::Vec3, glam::Vec3) {
        (
            glam::Vec3::new(0.0, 1.5, 4.0),
            glam::Vec3::new(0.0, 0.5, 0.0),
        )
    }
    fn update(&mut self, _dt: f32) {}
    fn update_uniforms(&self, queue: &wgpu::Queue, camera: &OrbitalCamera, time: f32) {
        let mut uniforms = Uniforms::default();
        uniforms.update_from_camera(camera, self.width, self.height, time);
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
}

// ============== DEMO FACTORY ==============

fn create_web_demo(
    id: DemoId,
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> Box<dyn WebDemo> {
    match id {
        DemoId::Empty => Box::new(EmptyDemo::new(device, format)),
        DemoId::Objects => Box::new(ObjectsDemo::new(device, format)),
        DemoId::Spheres => Box::new(SpheresDemo::new(device, format)),
        DemoId::Towers => Box::new(TowersDemo::new(device, format)),
        DemoId::Text2D => Box::new(Text2DDemo::new(device, format)),
        DemoId::Clay => Box::new(ClayDemo::new(device, format)),
        DemoId::TextShadow => Box::new(TextShadowDemo::new(device, format)),
    }
}

// ============== WEB RENDERER ==============

struct WebRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    surface_format: wgpu::TextureFormat,

    // Current demo
    current_demo: Box<dyn WebDemo>,
    current_demo_id: DemoId,

    // Camera
    camera: OrbitalCamera,

    // Input
    input: WebInputHandler,

    // 2D demo state
    text2d_offset: [f32; 2],
    text2d_scale: f32,
    text2d_rotation: f32,

    // Timing
    start_time: f64,
    last_frame_time: f64,

    // Control server integration
    control_state: Option<SharedWebControlState>,
    control_client: Option<WebWsClient>,
}

impl WebRenderer {
    async fn new(canvas: web_sys::HtmlCanvasElement, initial_demo: DemoId) -> Result<Self, JsValue> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| JsValue::from_str(&format!("Failed to create surface: {}", e)))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("No suitable GPU adapter found: {:?}", e)))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("RayBox Web Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("Failed to create device: {}", e)))?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: WIDTH,
            height: HEIGHT,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Create initial demo
        let current_demo = create_web_demo(initial_demo, &device, surface_format);

        // Setup camera for this demo
        let (pos, target) = current_demo.camera_config();
        let mut camera = OrbitalCamera::default();
        camera.target = target;
        camera.distance = (pos - target).length();
        if camera.distance > 0.0 {
            let dir = (pos - target) / camera.distance;
            camera.azimuth = dir.x.atan2(dir.z);
            camera.elevation = dir.y.asin();
        }

        let start_time = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            surface_format,
            current_demo,
            current_demo_id: initial_demo,
            camera,
            input: WebInputHandler::new(),
            text2d_offset: [0.0, 0.0],
            text2d_scale: 1.0,
            text2d_rotation: 0.0,
            start_time,
            last_frame_time: start_time,
            control_state: None,
            control_client: None,
        })
    }

    /// Connect to control server
    fn connect_control(&mut self, url: &str) {
        let state = web_control::new_shared_state();
        match WebWsClient::connect(url, state.clone()) {
            Ok(client) => {
                self.control_state = Some(state);
                self.control_client = Some(client);
                log::info!("Connected to control server at {}", url);
            }
            Err(e) => {
                log::warn!("Failed to connect to control server: {:?}", e);
            }
        }
    }

    /// Process pending control commands
    fn process_control_commands(&mut self) {
        let state = match &self.control_state {
            Some(s) => s.clone(),
            None => return,
        };

        // Process all pending commands
        while let Some((id, cmd)) = state.borrow_mut().pop_command() {
            let response = self.handle_control_command(id, cmd);
            state.borrow_mut().push_response(response);
        }

        // Flush responses to server
        if let Some(ref client) = self.control_client {
            client.flush_responses();
        }
    }

    /// Handle a single control command
    fn handle_control_command(&mut self, id: u64, command: WebCommand) -> web_control::WebResponse {
        match command {
            WebCommand::SwitchDemo(demo_id) => {
                if let Some(new_id) = DemoId::from_u8(demo_id) {
                    self.switch_demo(new_id);
                    success_response(id, Some(&format!(r#"{{"demo":{},"name":"{}"}}"#, demo_id, new_id.name())))
                } else {
                    error_response(id, "InvalidDemoId", &format!("Invalid demo ID: {}", demo_id))
                }
            }
            WebCommand::SetCamera { position, yaw, pitch } => {
                if let Some(pos) = position {
                    let target = glam::Vec3::from_array(pos);
                    self.camera.distance = (target - self.camera.target).length().max(0.1);
                }
                if let Some(y) = yaw {
                    self.camera.azimuth = y;
                }
                if let Some(p) = pitch {
                    self.camera.elevation = p;
                }
                success_response(id, None)
            }
            WebCommand::Screenshot => {
                // For web, we can't easily capture the canvas, so return an error for now
                error_response(id, "NotSupported", "Screenshot not implemented for web yet")
            }
            WebCommand::GetStatus => {
                let pos = self.camera.position();
                let overlay_mode = if self.input.overlay_visible() { "app" } else { "off" };
                status_response(
                    id,
                    self.current_demo_id as u8,
                    self.current_demo.name(),
                    [pos.x, pos.y, pos.z],
                    self.input.fps(),
                    overlay_mode,
                    self.input.show_keybindings,
                )
            }
            WebCommand::ToggleOverlay(mode) => {
                match mode.as_str() {
                    "off" => self.input.overlay_mode = crate::demo_core::OverlayMode::Off,
                    "app" => self.input.overlay_mode = crate::demo_core::OverlayMode::App,
                    "full" => self.input.overlay_mode = crate::demo_core::OverlayMode::App, // Web doesn't have full system stats
                    _ => {}
                }
                success_response(id, None)
            }
            WebCommand::PressKey(key) => {
                match key.as_str() {
                    "k" | "K" => self.input.toggle_keybindings(),
                    "f" | "F" => self.input.toggle_overlay_app(),
                    "g" | "G" => self.input.toggle_overlay_full(),
                    _ => {}
                }
                success_response(id, None)
            }
            WebCommand::Ping => pong_response(id),
            WebCommand::Reload => {
                // Signal that reload was requested - JavaScript will handle the actual reload
                if let Some(ref state) = self.control_state {
                    state.borrow_mut().request_reload();
                }
                success_response(id, Some(r#"{"message":"reload_requested"}"#))
            }
        }
    }

    fn switch_demo(&mut self, new_id: DemoId) {
        if new_id == self.current_demo_id {
            return;
        }

        // Create new demo
        let new_demo = create_web_demo(new_id, &self.device, self.surface_format);

        // Setup camera for this demo
        let (pos, target) = new_demo.camera_config();
        self.camera = OrbitalCamera::default();
        self.camera.target = target;
        self.camera.distance = (pos - target).length().max(0.1);
        if self.camera.distance > 0.0 {
            let dir = (pos - target) / self.camera.distance;
            self.camera.azimuth = dir.x.atan2(dir.z);
            self.camera.elevation = dir.y.asin();
        }

        // Reset 2D controls
        self.text2d_offset = [0.0, 0.0];
        self.text2d_scale = 1.0;
        self.text2d_rotation = 0.0;

        self.current_demo = new_demo;
        self.current_demo_id = new_id;

        log::info!("Switched to demo {}: {}", new_id as u8, self.current_demo.name());
    }

    fn update(&mut self) {
        let now = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);
        let dt = ((now - self.last_frame_time) / 1000.0) as f32;
        self.last_frame_time = now;

        self.input.update_frame_time(dt);

        // Process control commands
        self.process_control_commands();

        // Handle demo switching (0-6 keys)
        for i in 0..7 {
            let key = format!("Digit{}", i);
            if self.input.is_key_pressed(&key) {
                if let Some(id) = DemoId::from_u8(i) {
                    self.switch_demo(id);
                }
            }
        }

        // Handle toggle keys
        if self.input.is_key_pressed("KeyK") {
            self.input.pressed_keys.remove("KeyK");
            self.input.toggle_keybindings();
        }
        if self.input.is_key_pressed("KeyF") {
            self.input.pressed_keys.remove("KeyF");
            self.input.toggle_overlay_app();
        }
        if self.input.is_key_pressed("KeyG") {
            self.input.pressed_keys.remove("KeyG");
            self.input.toggle_overlay_full();
        }

        match self.current_demo.demo_type() {
            DemoType::Scene3D => {
                // 3D camera controls
                const ROTATION_SPEED: f32 = 0.03;
                const ZOOM_SPEED: f32 = 0.1;

                if self.input.is_key_pressed("KeyA") {
                    self.camera.rotate_horizontal(-ROTATION_SPEED);
                }
                if self.input.is_key_pressed("KeyD") {
                    self.camera.rotate_horizontal(ROTATION_SPEED);
                }
                if self.input.is_key_pressed("KeyW") {
                    self.camera.zoom(ZOOM_SPEED);
                }
                if self.input.is_key_pressed("KeyS") {
                    self.camera.zoom(-ZOOM_SPEED);
                }
                if self.input.is_key_pressed("KeyQ") {
                    self.camera.rotate_vertical(ROTATION_SPEED);
                }
                if self.input.is_key_pressed("KeyE") {
                    self.camera.rotate_vertical(-ROTATION_SPEED);
                }
            }
            DemoType::Scene2D => {
                // 2D controls
                const PAN_SPEED: f32 = 5.0;
                const ZOOM_SPEED: f32 = 0.02;
                const ROT_SPEED: f32 = 0.05;

                if self.input.is_key_pressed("KeyA") {
                    self.text2d_offset[0] -= PAN_SPEED / self.text2d_scale;
                }
                if self.input.is_key_pressed("KeyD") {
                    self.text2d_offset[0] += PAN_SPEED / self.text2d_scale;
                }
                if self.input.is_key_pressed("KeyW") {
                    self.text2d_offset[1] += PAN_SPEED / self.text2d_scale;
                }
                if self.input.is_key_pressed("KeyS") {
                    self.text2d_offset[1] -= PAN_SPEED / self.text2d_scale;
                }
                if self.input.is_key_pressed("ArrowUp") {
                    self.text2d_scale *= 1.0 + ZOOM_SPEED;
                }
                if self.input.is_key_pressed("ArrowDown") {
                    self.text2d_scale *= 1.0 - ZOOM_SPEED;
                }
                if self.input.is_key_pressed("KeyQ") {
                    self.text2d_rotation += ROT_SPEED;
                }
                if self.input.is_key_pressed("KeyE") {
                    self.text2d_rotation -= ROT_SPEED;
                }
                self.text2d_scale = self.text2d_scale.clamp(0.1, 10.0);

                // Update Text2D demo if that's the current one
                if self.current_demo_id == DemoId::Text2D {
                    // We need to pass the values through update_uniforms
                }
            }
        }

        self.current_demo.update(dt);
    }

    fn render(&self) -> Result<(), wgpu::SurfaceError> {
        let time = {
            let now = web_sys::window()
                .and_then(|w| w.performance())
                .map(|p| p.now())
                .unwrap_or(0.0);
            ((now - self.start_time) / 1000.0) as f32
        };

        // Update uniforms
        self.current_demo.update_uniforms(&self.queue, &self.camera, time);

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Web Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Web Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.current_demo.render(&mut render_pass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    fn key_down(&mut self, code: String) {
        self.input.key_down(&code);
    }

    fn key_up(&mut self, code: String) {
        self.input.key_up(&code);
    }

    /// Extract current state for hot-reload preservation
    fn extract_state(&self) -> WebReloadableState {
        let now = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);
        let elapsed = ((now - self.start_time) / 1000.0) as f32;

        let overlay_mode = match self.input.overlay_mode {
            OverlayMode::Off => "off",
            OverlayMode::App => "app",
            OverlayMode::Full => "full",
        };

        WebReloadableState {
            current_demo: self.current_demo_id as u8,
            camera_distance: self.camera.distance,
            camera_azimuth: self.camera.azimuth,
            camera_elevation: self.camera.elevation,
            camera_target: [
                self.camera.target.x,
                self.camera.target.y,
                self.camera.target.z,
            ],
            overlay_mode: overlay_mode.to_string(),
            show_keybindings: self.input.show_keybindings,
            text2d_offset: self.text2d_offset,
            text2d_scale: self.text2d_scale,
            text2d_rotation: self.text2d_rotation,
            time_offset: elapsed,
        }
    }

    /// Apply saved state after hot-reload
    fn apply_state(&mut self, state: WebReloadableState) {
        // Switch demo if different
        if let Some(demo_id) = DemoId::from_u8(state.current_demo) {
            if demo_id != self.current_demo_id {
                self.switch_demo(demo_id);
            }
        }

        // Restore camera state
        self.camera.distance = state.camera_distance;
        self.camera.azimuth = state.camera_azimuth;
        self.camera.elevation = state.camera_elevation;
        self.camera.target = glam::Vec3::from_array(state.camera_target);

        // Restore overlay state
        self.input.overlay_mode = match state.overlay_mode.as_str() {
            "app" => OverlayMode::App,
            "full" => OverlayMode::Full,
            _ => OverlayMode::Off,
        };
        self.input.show_keybindings = state.show_keybindings;

        // Restore 2D state
        self.text2d_offset = state.text2d_offset;
        self.text2d_scale = state.text2d_scale;
        self.text2d_rotation = state.text2d_rotation;

        // Adjust start time to maintain animation continuity
        if let Some(window) = web_sys::window() {
            if let Some(perf) = window.performance() {
                let now = perf.now();
                self.start_time = now - (state.time_offset as f64 * 1000.0);
            }
        }

        log::info!("Applied saved state: demo={}, camera distance={:.2}",
            state.current_demo, state.camera_distance);
    }

    /// Check if reload was requested and handle it
    fn check_reload_requested(&self) -> bool {
        if let Some(ref state) = self.control_state {
            state.borrow_mut().take_reload_request()
        } else {
            false
        }
    }

    fn get_overlay_text(&self) -> String {
        let mut lines = Vec::new();

        // Demo info
        lines.push(format!(
            "Demo {}: {}",
            self.current_demo_id as u8,
            self.current_demo.name()
        ));

        // Stats (if enabled)
        if self.input.overlay_visible() {
            lines.push(self.input.format_stats());
        }

        // Keybindings (if enabled)
        if self.input.show_keybindings {
            lines.push(String::new()); // blank line
            lines.push("=== Keybindings ===".to_string());
            for (key, desc) in self.current_demo.keybindings() {
                lines.push(format!("{}: {}", key, desc));
            }
            lines.push("---".to_string());
            for (key, desc) in KEYBINDINGS_COMMON {
                lines.push(format!("{}: {}", key, desc));
            }
        }

        lines.join("\n")
    }
}

fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    web_sys::window()
        .unwrap()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .unwrap();
}

/// Check if control mode is enabled via URL parameter (?control=1)
fn is_control_mode_enabled() -> bool {
    if let Some(window) = web_sys::window() {
        if let Ok(search) = window.location().search() {
            if let Some(params) = search.strip_prefix('?') {
                for param in params.split('&') {
                    if param == "control=1" || param == "control=true" {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Get control server URL from URL parameter or use default
fn get_control_url() -> String {
    if let Some(window) = web_sys::window() {
        if let Ok(search) = window.location().search() {
            if let Some(params) = search.strip_prefix('?') {
                for param in params.split('&') {
                    if let Some(url) = param.strip_prefix("control_url=") {
                        return url.to_string();
                    }
                }
            }
        }
    }
    "ws://127.0.0.1:9300".to_string()
}

/// Parse initial demo from URL parameter (?demo=X)
fn get_initial_demo_from_url() -> DemoId {
    if let Some(window) = web_sys::window() {
        if let Ok(search) = window.location().search() {
            if let Some(params) = search.strip_prefix('?') {
                for param in params.split('&') {
                    if let Some(value) = param.strip_prefix("demo=") {
                        if let Ok(id) = value.parse::<u8>() {
                            if let Some(demo_id) = DemoId::from_u8(id) {
                                return demo_id;
                            }
                        }
                    }
                }
            }
        }
    }
    DemoId::Objects // Default to Objects demo
}

#[wasm_bindgen(start)]
pub async fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).unwrap();

    let initial_demo = get_initial_demo_from_url();
    log::info!(
        "Initializing raybox web renderer with demo {}: {}",
        initial_demo as u8,
        initial_demo.name()
    );

    let window = web_sys::window().ok_or("No window found")?;
    let document = window.document().ok_or("No document found")?;
    let canvas = document
        .get_element_by_id("canvas")
        .ok_or("No canvas element found")?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    let mut renderer = WebRenderer::new(canvas, initial_demo).await?;
    log::info!("Web renderer initialized successfully");
    log::info!("Controls: 0-6 switch demos, A/D rotate, W/S zoom, Q/E tilt, K keybindings, F stats");

    // Apply saved state if available (for hot-reload)
    SAVED_STATE.with(|s| {
        if let Some(state) = s.borrow_mut().take() {
            log::info!("Restoring saved state after hot-reload");
            renderer.apply_state(state);
        }
    });

    // Connect to control server if enabled
    if is_control_mode_enabled() {
        let control_url = get_control_url();
        log::info!("Control mode enabled, connecting to {}", control_url);
        renderer.connect_control(&control_url);
    }

    let renderer = Rc::new(RefCell::new(renderer));

    // Set up keyboard event listeners
    let renderer_keydown = renderer.clone();
    let keydown_closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
        renderer_keydown.borrow_mut().key_down(event.code());
    }) as Box<dyn FnMut(_)>);

    let renderer_keyup = renderer.clone();
    let keyup_closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
        renderer_keyup.borrow_mut().key_up(event.code());
    }) as Box<dyn FnMut(_)>);

    window.add_event_listener_with_callback("keydown", keydown_closure.as_ref().unchecked_ref())?;
    window.add_event_listener_with_callback("keyup", keyup_closure.as_ref().unchecked_ref())?;

    // Keep closures alive
    keydown_closure.forget();
    keyup_closure.forget();

    // Update overlay text element
    let overlay_element = document.get_element_by_id("overlay");

    // Store renderer reference for hot-reload state extraction
    RENDERER_REF.with(|r| {
        *r.borrow_mut() = Some(renderer.clone());
    });

    // Animation loop
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();

    let renderer_clone = renderer.clone();
    *g.borrow_mut() = Some(Closure::new(move || {
        renderer_clone.borrow_mut().update();
        if let Err(e) = renderer_clone.borrow().render() {
            log::error!("Render error: {:?}", e);
        }

        // Update overlay text
        if let Some(ref elem) = overlay_element {
            let text = renderer_clone.borrow().get_overlay_text();
            elem.set_text_content(Some(&text));
        }

        request_animation_frame(f.borrow().as_ref().unwrap());
    }));

    request_animation_frame(g.borrow().as_ref().unwrap());

    Ok(())
}

/// Start with a specific demo (callable from JavaScript)
#[wasm_bindgen]
pub async fn start_with_demo(demo_id: u8) -> Result<(), JsValue> {
    let initial_demo = DemoId::from_u8(demo_id).unwrap_or(DemoId::Objects);

    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).ok(); // ok() because it might already be initialized

    log::info!(
        "Starting raybox with demo {}: {}",
        initial_demo as u8,
        initial_demo.name()
    );

    let window = web_sys::window().ok_or("No window found")?;
    let document = window.document().ok_or("No document found")?;
    let canvas = document
        .get_element_by_id("canvas")
        .ok_or("No canvas element found")?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    let mut renderer = WebRenderer::new(canvas, initial_demo).await?;

    // Apply saved state if available (for hot-reload)
    SAVED_STATE.with(|s| {
        if let Some(state) = s.borrow_mut().take() {
            log::info!("Restoring saved state after hot-reload");
            renderer.apply_state(state);
        }
    });

    // Connect to control server if enabled
    if is_control_mode_enabled() {
        let control_url = get_control_url();
        log::info!("Control mode enabled, connecting to {}", control_url);
        renderer.connect_control(&control_url);
    }

    let renderer = Rc::new(RefCell::new(renderer));

    // Store renderer reference for hot-reload state extraction
    RENDERER_REF.with(|r| {
        *r.borrow_mut() = Some(renderer.clone());
    });

    // Set up keyboard event listeners
    let renderer_keydown = renderer.clone();
    let keydown_closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
        renderer_keydown.borrow_mut().key_down(event.code());
    }) as Box<dyn FnMut(_)>);

    let renderer_keyup = renderer.clone();
    let keyup_closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
        renderer_keyup.borrow_mut().key_up(event.code());
    }) as Box<dyn FnMut(_)>);

    window.add_event_listener_with_callback("keydown", keydown_closure.as_ref().unchecked_ref())?;
    window.add_event_listener_with_callback("keyup", keyup_closure.as_ref().unchecked_ref())?;

    keydown_closure.forget();
    keyup_closure.forget();

    // Update overlay text element
    let overlay_element = document.get_element_by_id("overlay");

    // Animation loop
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();

    let renderer_clone = renderer.clone();
    *g.borrow_mut() = Some(Closure::new(move || {
        renderer_clone.borrow_mut().update();
        if let Err(e) = renderer_clone.borrow().render() {
            log::error!("Render error: {:?}", e);
        }

        if let Some(ref elem) = overlay_element {
            let text = renderer_clone.borrow().get_overlay_text();
            elem.set_text_content(Some(&text));
        }

        request_animation_frame(f.borrow().as_ref().unwrap());
    }));

    request_animation_frame(g.borrow().as_ref().unwrap());

    Ok(())
}

// ============== HOT-RELOAD WASM_BINDGEN EXPORTS ==============

/// Serialize current renderer state for hot-reload preservation
/// Returns JSON string that can be passed to restore_state after WASM reload
#[wasm_bindgen]
pub fn serialize_state() -> String {
    // The state is stored globally when save_state_for_reload is called
    SAVED_STATE.with(|s| {
        if let Some(ref state) = *s.borrow() {
            serde_json::to_string(state).unwrap_or_else(|_| "{}".to_string())
        } else {
            "{}".to_string()
        }
    })
}

/// Restore renderer state after hot-reload
/// Called from JavaScript with the JSON from serialize_state
#[wasm_bindgen]
pub fn restore_state(json: &str) {
    if json.is_empty() || json == "{}" {
        return;
    }

    match serde_json::from_str::<WebReloadableState>(json) {
        Ok(state) => {
            SAVED_STATE.with(|s| {
                *s.borrow_mut() = Some(state);
            });
            log::info!("Saved state will be applied on next start");
        }
        Err(e) => {
            log::warn!("Failed to parse state JSON: {}", e);
        }
    }
}

/// Check if there's saved state to restore
#[wasm_bindgen]
pub fn has_saved_state() -> bool {
    SAVED_STATE.with(|s| s.borrow().is_some())
}

/// Clear saved state after it's been applied
#[wasm_bindgen]
pub fn clear_saved_state() {
    SAVED_STATE.with(|s| {
        *s.borrow_mut() = None;
    });
}

/// Save current state for hot-reload (called from JavaScript before reload)
/// Returns JSON string of the saved state
#[wasm_bindgen]
pub fn save_state_for_reload() -> String {
    RENDERER_REF.with(|r| {
        if let Some(ref renderer_rc) = *r.borrow() {
            let renderer = renderer_rc.borrow();
            let state = renderer.extract_state();
            SAVED_STATE.with(|s| {
                *s.borrow_mut() = Some(state.clone());
            });
            serde_json::to_string(&state).unwrap_or_else(|_| "{}".to_string())
        } else {
            log::warn!("No renderer available for state extraction");
            "{}".to_string()
        }
    })
}

/// Clean up renderer before hot-reload (called from JavaScript)
#[wasm_bindgen]
pub fn cleanup_for_reload() {
    // Save state first
    save_state_for_reload();

    // Clear the renderer reference to allow GPU resources to be freed
    RENDERER_REF.with(|r| {
        *r.borrow_mut() = None;
    });

    log::info!("Cleaned up for hot-reload");
}
