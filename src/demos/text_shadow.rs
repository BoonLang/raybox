//! Demo 6: Floating 3D Text with Shadows using Vector SDF
//!
//! 3D extruded text casting soft shadows on ground plane.

use super::{Demo, DemoContext, DemoId, DemoType, KEYBINDINGS_3D};
use std::any::Any;
use crate::camera::FlyCamera;
use crate::input::CameraConfig;
use crate::shader_bindings::sdf_text_shadow_vector;
use crate::text::{VectorFont, VectorFontAtlas, build_char_grid};
use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

const LOREM: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum. Curabitur pretium tincidunt lacus. Nulla gravida orci a odio. Nullam varius, turpis et commodo pharetra, est eros bibendum elit, nec luctus magna felis sollicitudin mauris. Integer in mauris eu nibh euismod gravida. Duis ac tellus et risus vulputate vehicula. Donec lobortis risus a elit. Etiam tempor. Ut ullamcorper, ligula eu tempor congue, eros est euismod turpis, id tincidunt sapien risus a quam. Maecenas fermentum consequat mi. Donec fermentum. Pellentesque malesuada nulla a mi. Duis sapien sem, aliquet sed, vulputate eget, feugiat non, orci. Sed neque. Sed eget lacus. Mauris non dui nec urna suscipit nonummy. Fusce fermentum fermentum arcu. Vestibulum ante ipsum primis in faucibus orci luctus et ultrices posuere cubilia curae.";

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Uniforms {
    inv_view_proj: [[f32; 4]; 4],
    camera_pos_time: [f32; 4],
    light_dir_intensity: [f32; 4],
    render_params: [f32; 4],
    text_params: [f32; 4],
    text_aabb: [f32; 4],
    char_grid_params: [f32; 4],
    char_grid_bounds: [f32; 4],
}

impl Default for Uniforms {
    fn default() -> Self {
        Self {
            inv_view_proj: [[0.0; 4]; 4],
            camera_pos_time: [0.0, 0.5, 3.0, 0.0],
            light_dir_intensity: [0.4, 0.8, 0.5, 1.3],
            render_params: [800.0, 600.0, 0.15, 1.0],
            text_params: [0.0, 0.0, 0.4, 0.0],
            text_aabb: [0.0; 4],
            char_grid_params: [0.0; 4],
            char_grid_bounds: [0.0; 4],
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

fn build_shadow_text_layout(atlas: &VectorFontAtlas) -> Vec<GpuCharInstance> {
    let mut instances = Vec::new();

    let full_text = format!(
        "VECTOR SDF TEXT\n\n{}",
        format!("{} {} {}", LOREM, LOREM, LOREM)
    );

    let scale = 0.12;
    let line_height = 0.18;
    let margin = 0.1;

    let panel_width = 2.4;
    let panel_height = 1.8;
    let start_x = -panel_width / 2.0 + margin;
    let start_y = panel_height / 2.0 - margin;
    let max_x = panel_width / 2.0 - margin;

    let mut x = start_x;
    let mut y = start_y;
    let mut line_num = 0;
    let max_lines = 20;

    for ch in full_text.chars() {
        if line_num >= max_lines {
            break;
        }

        if ch == '\n' {
            x = start_x;
            y -= line_height;
            line_num += 1;
            continue;
        }

        let codepoint = ch as u32;
        if let Some(entry) = atlas.glyphs.get(&codepoint) {
            let glyph_idx = atlas
                .glyph_list
                .iter()
                .position(|(cp, _)| *cp == codepoint)
                .unwrap_or(0) as u32;

            let advance = entry.advance * scale;

            if x + advance > max_x {
                x = start_x;
                y -= line_height;
                line_num += 1;
                if line_num >= max_lines {
                    break;
                }
            }

            instances.push(GpuCharInstance {
                pos_and_char: [x, y, scale, glyph_idx as f32],
            });

            x += advance;
        } else if ch == ' ' {
            x += 0.3 * scale;
            if x > max_x {
                x = start_x;
                y -= line_height;
                line_num += 1;
            }
        }
    }

    instances
}

fn compute_text_aabb(instances: &[GpuCharInstance], atlas: &VectorFontAtlas) -> [f32; 4] {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for inst in instances {
        let x = inst.pos_and_char[0];
        let y = inst.pos_and_char[1];
        let scale = inst.pos_and_char[2];
        let glyph_idx = inst.pos_and_char[3] as usize;

        if glyph_idx < atlas.glyph_list.len() {
            let (_, entry) = &atlas.glyph_list[glyph_idx];
            let bounds = entry.bounds;
            min_x = min_x.min(x + bounds[0] * scale);
            min_y = min_y.min(y + bounds[1] * scale);
            max_x = max_x.max(x + bounds[2] * scale);
            max_y = max_y.max(y + bounds[3] * scale);
        }
    }

    // Add small margin
    [min_x - 0.05, min_y - 0.05, max_x + 0.05, max_y + 0.05]
}

pub struct TextShadowDemo {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    char_count: u32,
    text_aabb: [f32; 4],
    char_grid_params: [f32; 4],
    char_grid_bounds: [f32; 4],
    width: u32,
    height: u32,
}

impl TextShadowDemo {
    pub fn new(ctx: &DemoContext) -> Result<Self> {
        // Load vector font
        let font_data = std::fs::read("assets/fonts/DejaVuSans.ttf")
            .context("Failed to load font file")?;
        let font = VectorFont::from_ttf(&font_data).map_err(|e| anyhow::anyhow!(e))?;
        let atlas = VectorFontAtlas::from_font(&font, 32);

        // Build text layout
        let char_instances = build_shadow_text_layout(&atlas);
        let char_count = char_instances.len() as u32;
        let text_aabb = compute_text_aabb(&char_instances, &atlas);

        // Build character spatial grid
        let instance_data: Vec<[f32; 4]> = char_instances.iter().map(|c| c.pos_and_char).collect();
        let char_grid = build_char_grid(&instance_data, &atlas, [48, 32]);

        let char_grid_params = [
            char_grid.dims[0] as f32,
            char_grid.dims[1] as f32,
            char_grid.cell_size[0],
            char_grid.cell_size[1],
        ];
        let char_grid_bounds = char_grid.bounds;

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

        let mut uniforms = Uniforms::default();
        uniforms.text_params[0] = char_count as f32;
        uniforms.text_aabb = text_aabb;
        uniforms.char_grid_params = char_grid_params;
        uniforms.char_grid_bounds = char_grid_bounds;

        let uniform_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("TextShadow Demo Uniform Buffer"),
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

        let char_grid_cells_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Char Grid Cells Buffer"),
            contents: bytemuck::cast_slice(&char_grid.cells),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let char_grid_indices_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Char Grid Indices Buffer"),
            contents: bytemuck::cast_slice(&char_grid.char_indices),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // Create bind group layout
        let storage_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };

        let bind_group_layout = ctx.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TextShadow Demo Bind Group Layout"),
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
                storage_entry(1),
                storage_entry(2),
                storage_entry(3),
                storage_entry(4),
                storage_entry(5),
                storage_entry(6),
                storage_entry(7),
            ],
        });

        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TextShadow Demo Bind Group"),
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
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: char_grid_cells_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: char_grid_indices_buffer.as_entire_binding(),
                },
            ],
        });

        // Create pipeline
        let shader_module = sdf_text_shadow_vector::create_shader_module_embed_source(ctx.device);

        let pipeline_layout = ctx.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("TextShadow Demo Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = ctx.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("TextShadow Demo Pipeline"),
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
            text_aabb,
            char_grid_params,
            char_grid_bounds,
            width: ctx.width,
            height: ctx.height,
        })
    }

    pub fn update_uniforms(&self, queue: &wgpu::Queue, camera: &FlyCamera, time: f32) {
        let mut uniforms = Uniforms::default();
        uniforms.update_from_camera(camera, self.width, self.height, time);
        uniforms.text_params[0] = self.char_count as f32;
        uniforms.text_aabb = self.text_aabb;
        uniforms.char_grid_params = self.char_grid_params;
        uniforms.char_grid_bounds = self.char_grid_bounds;
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }
}

impl Demo for TextShadowDemo {
    fn name(&self) -> &'static str {
        "Text Shadow"
    }

    fn id(&self) -> DemoId {
        DemoId::TextShadow
    }

    fn demo_type(&self) -> DemoType {
        DemoType::Scene3D
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        KEYBINDINGS_3D
    }

    fn camera_config(&self) -> CameraConfig {
        CameraConfig {
            initial_position: glam::Vec3::new(0.0, 0.0, 3.5),
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
