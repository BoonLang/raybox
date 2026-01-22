//! Demo 4: 2D Vector SDF Text Rendering
//!
//! High-quality vector SDF text with 1000+ words using exact Bezier computation.

use super::{Demo, DemoContext, DemoId, DemoType, KEYBINDINGS_2D};
use std::any::Any;
use crate::camera::FlyCamera;
use crate::input::CameraConfig;
use crate::shader_bindings::sdf_text2d_vector;
use crate::text::{VectorFont, VectorFontAtlas};
use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

const LOREM: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum. Curabitur pretium tincidunt lacus. Nulla gravida orci a odio. Nullam varius, turpis et commodo pharetra, est eros bibendum elit, nec luctus magna felis sollicitudin mauris. Integer in mauris eu nibh euismod gravida. Duis ac tellus et risus vulputate vehicula. Donec lobortis risus a elit. Etiam tempor. Ut ullamcorper, ligula eu tempor congue, eros est euismod turpis, id tincidunt sapien risus a quam. Maecenas fermentum consequat mi. Donec fermentum. Pellentesque malesuada nulla a mi. Duis sapien sem, aliquet sed, vulputate eget, feugiat non, orci. Sed neque. Sed eget lacus. Mauris non dui nec urna suscipit nonummy. Fusce fermentum fermentum arcu. Vestibulum ante ipsum primis in faucibus orci luctus et ultrices posuere cubilia curae.";

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
    padding: [f32; 2],
    text_params: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GpuGridCell {
    curve_start_and_count: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GpuBezierCurve {
    points01: [f32; 4],
    points2bbox: [f32; 4],
    bbox_flags: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GpuGlyphData {
    bounds: [f32; 4],
    grid_info: [u32; 4],
    curve_info: [u32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GpuCharInstance {
    pos_and_char: [f32; 4],
}

fn build_text_layout(atlas: &VectorFontAtlas, width: f32, height: f32) -> Vec<GpuCharInstance> {
    let mut instances = Vec::new();

    let full_text = format!(
        "VECTOR SDF TEXT ENGINE\n\n{}",
        format!("{} {} {} {} {} {}", LOREM, LOREM, LOREM, LOREM, LOREM, LOREM)
    );

    let font_size = 16.0;
    let line_height = font_size * 1.4;
    let margin = 20.0;

    let start_x = margin;
    let start_y = height - margin - font_size;
    let max_x = width - margin;
    let min_y = margin;

    let mut x = start_x;
    let mut y = start_y;

    for ch in full_text.chars() {
        if y < min_y {
            break;
        }

        if ch == '\n' {
            x = start_x;
            y -= line_height;
            continue;
        }

        let codepoint = ch as u32;
        if let Some(entry) = atlas.glyphs.get(&codepoint) {
            let glyph_idx = atlas
                .glyph_list
                .iter()
                .position(|(cp, _)| *cp == codepoint)
                .unwrap_or(0) as u32;

            let advance = entry.advance * font_size;

            if x + advance > max_x {
                x = start_x;
                y -= line_height;
                if y < min_y {
                    break;
                }
            }

            instances.push(GpuCharInstance {
                pos_and_char: [x, y, font_size, glyph_idx as f32],
            });

            x += advance;
        } else if ch == ' ' {
            x += 0.3 * font_size;
            if x > max_x {
                x = start_x;
                y -= line_height;
            }
        }
    }

    instances
}

pub struct Text2DDemo {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    char_count: u32,
    width: u32,
    height: u32,
    // 2D controls state
    pub offset: [f32; 2],
    pub scale: f32,
    pub rotation: f32,
}

impl Text2DDemo {
    pub fn new(ctx: &DemoContext) -> Result<Self> {
        // Load vector font
        let font_data = std::fs::read("assets/fonts/DejaVuSans.ttf")
            .context("Failed to load font file")?;
        let font = VectorFont::from_ttf(&font_data).map_err(|e| anyhow::anyhow!(e))?;
        let atlas = VectorFontAtlas::from_font(&font, 12);

        // Build text layout
        let char_instances = build_text_layout(&atlas, ctx.width as f32, ctx.height as f32);
        let char_count = char_instances.len() as u32;

        // Prepare GPU data
        let gpu_grid_cells: Vec<GpuGridCell> = atlas
            .grid_cells
            .iter()
            .map(|c| GpuGridCell {
                curve_start_and_count: (c.curve_start as u32)
                    | ((c.curve_count as u32) << 16)
                    | ((c.flags as u32) << 24),
            })
            .collect();

        let gpu_curve_indices: Vec<u32> = atlas.curve_indices.iter().map(|&i| i as u32).collect();

        let gpu_curves: Vec<GpuBezierCurve> = atlas
            .curves
            .iter()
            .map(|c| {
                let p0 = c.p0();
                let p1 = c.p1();
                let p2 = c.p2();
                GpuBezierCurve {
                    points01: [p0.0, p0.1, p1.0, p1.1],
                    points2bbox: [p2.0, p2.1, c.bbox[0], c.bbox[1]],
                    bbox_flags: [c.bbox[2], c.bbox[3], c.flags as f32, 0.0],
                }
            })
            .collect();

        let gpu_glyph_data: Vec<GpuGlyphData> = atlas
            .glyph_list
            .iter()
            .map(|(_, entry)| GpuGlyphData {
                bounds: entry.bounds,
                grid_info: [
                    entry.grid_offset,
                    entry.grid_size[0],
                    entry.grid_size[1],
                    0,
                ],
                curve_info: [
                    entry.curve_offset,
                    entry.curve_count,
                    0,
                    0,
                ],
            })
            .collect();

        // Create buffers
        let uniforms = Uniforms {
            screen_size: [ctx.width as f32, ctx.height as f32],
            padding: [0.0, 0.0],
            text_params: [char_count as f32, 1.0, 0.0, 0.0],
        };

        let uniform_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Text2D Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let grid_cells_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Grid Cells Buffer"),
            contents: bytemuck::cast_slice(if gpu_grid_cells.is_empty() {
                &[GpuGridCell { curve_start_and_count: 0 }]
            } else {
                &gpu_grid_cells
            }),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let curve_indices_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Curve Indices Buffer"),
            contents: bytemuck::cast_slice(if gpu_curve_indices.is_empty() {
                &[0u32]
            } else {
                &gpu_curve_indices
            }),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let curves_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Curves Buffer"),
            contents: bytemuck::cast_slice(if gpu_curves.is_empty() {
                &[GpuBezierCurve {
                    points01: [0.0; 4],
                    points2bbox: [0.0; 4],
                    bbox_flags: [0.0; 4],
                }]
            } else {
                &gpu_curves
            }),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let glyph_data_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Glyph Data Buffer"),
            contents: bytemuck::cast_slice(if gpu_glyph_data.is_empty() {
                &[GpuGlyphData {
                    bounds: [0.0; 4],
                    grid_info: [0; 4],
                    curve_info: [0; 4],
                }]
            } else {
                &gpu_glyph_data
            }),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let char_instances_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Char Instances Buffer"),
            contents: bytemuck::cast_slice(if char_instances.is_empty() {
                &[GpuCharInstance { pos_and_char: [0.0; 4] }]
            } else {
                &char_instances
            }),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // Create bind group layout
        let bind_group_layout = ctx.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Text2D Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Text2D Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: grid_cells_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: curve_indices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: curves_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: glyph_data_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: char_instances_buffer.as_entire_binding(),
                },
            ],
        });

        // Create pipeline
        let shader_module = sdf_text2d_vector::create_shader_module_embed_source(ctx.device);

        let pipeline_layout = ctx.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Text2D Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = ctx.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
            char_count,
            width: ctx.width,
            height: ctx.height,
            offset: [0.0, 0.0],
            scale: 1.0,
            rotation: 0.0,
        })
    }

    fn update_uniforms(&self, queue: &wgpu::Queue) {
        let uniforms = Uniforms {
            screen_size: [self.width as f32, self.height as f32],
            padding: self.offset,
            text_params: [self.char_count as f32, self.scale, self.rotation, 0.0],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    pub fn reset_rotation(&mut self) {
        self.rotation = 0.0;
    }

    pub fn reset_all(&mut self) {
        self.offset = [0.0, 0.0];
        self.scale = 1.0;
        self.rotation = 0.0;
    }
}

impl Demo for Text2DDemo {
    fn name(&self) -> &'static str {
        "2D Text"
    }

    fn id(&self) -> DemoId {
        DemoId::Text2D
    }

    fn demo_type(&self) -> DemoType {
        DemoType::Scene2D
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        KEYBINDINGS_2D
    }

    fn camera_config(&self) -> CameraConfig {
        CameraConfig::default()
    }

    fn update(&mut self, _dt: f32, _camera: &mut FlyCamera) {
        // 2D controls are handled by the runner
    }

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue, _time: f32) {
        self.update_uniforms(queue);
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
