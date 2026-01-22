//! Vector SDF text renderer using wgpu
//!
//! Renders text using exact Bézier curve SDF computation on the GPU
//! with grid-based acceleration for O(1-3) curve lookups per pixel.

use super::glyph_atlas::{GlyphAtlasEntry, GridCell, VectorFontAtlas};
use super::vector_font::{BezierCurve, VectorFont};
use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// Vertex for text quad rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct TextVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
}

impl TextVertex {
    #[allow(dead_code)]
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TextVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position -> location 0 (POSITION)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // uv -> location 6 (TEXCOORD0 maps to location 6 in Slang)
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// Instance data for each glyph (passed as vertex attributes)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VectorGlyphInstance {
    /// Screen position (top-left)
    pub offset: [f32; 2],
    /// Glyph size in pixels
    pub size: [f32; 2],
    /// Glyph bounds in em units [left, bottom, right, top]
    pub glyph_bounds: [f32; 4],
    /// Grid info [gridOffset, gridSizeX, gridSizeY, curveCount]
    pub grid_info: [u32; 4],
    /// Color RGBA
    pub color: [f32; 4],
}

impl VectorGlyphInstance {
    #[allow(dead_code)]
    pub fn new(
        offset: [f32; 2],
        size: [f32; 2],
        entry: &GlyphAtlasEntry,
        color: [f32; 4],
    ) -> Self {
        Self {
            offset,
            size,
            glyph_bounds: entry.bounds,
            grid_info: [
                entry.grid_offset,
                entry.grid_size[0],
                entry.grid_size[1],
                entry.curve_count,
            ],
            color,
        }
    }

    #[allow(dead_code)]
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<VectorGlyphInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // offset -> location 1 (TEXCOORD1)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // size -> location 2 (TEXCOORD2)
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // glyph_bounds -> location 3 (TEXCOORD3)
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // grid_info -> location 4 (TEXCOORD4, Uint32x4)
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Uint32x4,
                },
                // color -> location 5 (TEXCOORD5)
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 8]>() + std::mem::size_of::<[u32; 4]>())
                        as wgpu::BufferAddress,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Text rendering uniforms
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VectorTextUniforms {
    pub screen_size: [f32; 2],
    pub sdf_params: [f32; 2],
}

/// Packed grid cell for GPU
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuGridCell {
    /// Lower 16 bits = curve_start, upper 8 = count, top 8 = flags
    pub curve_start_and_count: u32,
}

impl From<&GridCell> for GpuGridCell {
    fn from(cell: &GridCell) -> Self {
        Self {
            curve_start_and_count: (cell.curve_start as u32)
                | ((cell.curve_count as u32) << 16)
                | ((cell.flags as u32) << 24),
        }
    }
}

/// Packed Bézier curve for GPU
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuBezierCurve {
    pub points01: [f32; 4],    // p0.xy, p1.xy
    pub points2bbox: [f32; 4], // p2.xy, bbox.xy (min)
    pub bbox_flags: [f32; 4],  // bbox.zw (max), flags, padding
}

impl From<&BezierCurve> for GpuBezierCurve {
    fn from(curve: &BezierCurve) -> Self {
        Self {
            points01: [
                curve.points[0],
                curve.points[1],
                curve.points[2],
                curve.points[3],
            ],
            points2bbox: [
                curve.points[4],
                curve.points[5],
                curve.bbox[0],
                curve.bbox[1],
            ],
            bbox_flags: [curve.bbox[2], curve.bbox[3], curve.flags as f32, 0.0],
        }
    }
}

/// Vector SDF Text Renderer
#[allow(dead_code)]
pub struct VectorTextRenderer {
    atlas: VectorFontAtlas,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    grid_cell_buffer: wgpu::Buffer,
    curve_index_buffer: wgpu::Buffer,
    curve_buffer: wgpu::Buffer,
    max_instances: u32,
}

#[allow(dead_code)]
impl VectorTextRenderer {
    /// Create a new vector text renderer
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        font: VectorFont,
        grid_resolution: u32,
    ) -> Result<Self> {
        // Build the atlas with grid subdivision
        let atlas = VectorFontAtlas::from_font(&font, grid_resolution);

        // Create uniform buffer
        let uniforms = VectorTextUniforms {
            screen_size: [800.0, 600.0],
            sdf_params: [grid_resolution as f32, 0.0],
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vector Text Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create instance buffer
        let max_instances = 10000u32;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vector Text Instance Buffer"),
            size: (max_instances as usize * std::mem::size_of::<VectorGlyphInstance>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create grid cell buffer
        let gpu_grid_cells: Vec<GpuGridCell> =
            atlas.grid_cells.iter().map(GpuGridCell::from).collect();

        // Ensure we have at least one element for the buffer
        let grid_cell_data = if gpu_grid_cells.is_empty() {
            vec![GpuGridCell {
                curve_start_and_count: 0,
            }]
        } else {
            gpu_grid_cells
        };

        let grid_cell_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vector Text Grid Cell Buffer"),
            contents: bytemuck::cast_slice(&grid_cell_data),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // Create curve index buffer
        let curve_index_data = if atlas.curve_indices.is_empty() {
            vec![0u32]
        } else {
            atlas.curve_indices.iter().map(|&x| x as u32).collect()
        };

        let curve_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vector Text Curve Index Buffer"),
            contents: bytemuck::cast_slice(&curve_index_data),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // Create curve buffer
        let gpu_curves: Vec<GpuBezierCurve> =
            atlas.curves.iter().map(GpuBezierCurve::from).collect();

        let curve_data = if gpu_curves.is_empty() {
            vec![GpuBezierCurve {
                points01: [0.0; 4],
                points2bbox: [0.0; 4],
                bbox_flags: [0.0; 4],
            }]
        } else {
            gpu_curves
        };

        let curve_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vector Text Curve Buffer"),
            contents: bytemuck::cast_slice(&curve_data),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Vector Text Bind Group Layout"),
            entries: &[
                // Uniforms
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
                // Grid cells
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
                // Curve indices
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
                // Curves
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
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Vector Text Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: grid_cell_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: curve_index_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: curve_buffer.as_entire_binding(),
                },
            ],
        });

        // Create shader module from generated bindings
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Vector SDF Text Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(env!("OUT_DIR"), "/sdf_text_vector.wgsl")).into(),
            ),
        });

        // Create pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Vector Text Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Vector Text Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[TextVertex::desc(), VectorGlyphInstance::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create vertex buffer (unit quad)
        let vertices = [
            TextVertex {
                position: [0.0, 0.0],
                uv: [0.0, 0.0],
            },
            TextVertex {
                position: [1.0, 0.0],
                uv: [1.0, 0.0],
            },
            TextVertex {
                position: [1.0, 1.0],
                uv: [1.0, 1.0],
            },
            TextVertex {
                position: [0.0, 0.0],
                uv: [0.0, 0.0],
            },
            TextVertex {
                position: [1.0, 1.0],
                uv: [1.0, 1.0],
            },
            TextVertex {
                position: [0.0, 1.0],
                uv: [0.0, 1.0],
            },
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vector Text Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Ok(Self {
            atlas,
            bind_group_layout,
            bind_group,
            pipeline,
            uniform_buffer,
            vertex_buffer,
            instance_buffer,
            grid_cell_buffer,
            curve_index_buffer,
            curve_buffer,
            max_instances,
        })
    }

    /// Update screen size
    pub fn update_screen_size(&self, queue: &wgpu::Queue, width: f32, height: f32) {
        let uniforms = VectorTextUniforms {
            screen_size: [width, height],
            sdf_params: [self.atlas.grid_resolution as f32, 0.0],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Layout text and return glyph instances
    pub fn layout_text(
        &self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        color: [f32; 4],
    ) -> Vec<VectorGlyphInstance> {
        let mut instances = Vec::new();
        let mut cursor_x = x;

        let scale = font_size;
        let baseline_y = y + self.atlas.ascender * scale;

        for ch in text.chars() {
            if ch == '\n' {
                continue;
            }

            if let Some(entry) = self.atlas.get_glyph(ch) {
                // Calculate glyph quad position
                let glyph_width = (entry.bounds[2] - entry.bounds[0]) * scale;
                let glyph_height = (entry.bounds[3] - entry.bounds[1]) * scale;
                let glyph_x = cursor_x + entry.bounds[0] * scale;
                let glyph_y = baseline_y - entry.bounds[3] * scale;

                instances.push(VectorGlyphInstance::new(
                    [glyph_x, glyph_y],
                    [glyph_width, glyph_height],
                    entry,
                    color,
                ));

                cursor_x += entry.advance * scale;
            }
        }

        instances
    }

    /// Layout multi-line text
    pub fn layout_text_block(
        &self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        line_height: f32,
        color: [f32; 4],
    ) -> Vec<VectorGlyphInstance> {
        let mut instances = Vec::new();
        let mut current_y = y;

        for line in text.lines() {
            let line_instances = self.layout_text(line, x, current_y, font_size, color);
            instances.extend(line_instances);
            current_y += line_height;
        }

        instances
    }

    /// Render text instances
    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        instances: &[VectorGlyphInstance],
    ) {
        if instances.is_empty() {
            return;
        }

        let instance_count = instances.len().min(self.max_instances as usize);
        queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&instances[..instance_count]),
        );

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        render_pass.draw(0..6, 0..instance_count as u32);
    }

    /// Get the atlas reference
    pub fn atlas(&self) -> &VectorFontAtlas {
        &self.atlas
    }
}
