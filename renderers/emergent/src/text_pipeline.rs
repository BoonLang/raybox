//! MTSDF Text Rendering Pipeline
//!
//! Renders text using Multi-channel True Signed Distance Field atlas.

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::font::{FontAtlas, PositionedGlyph};

/// Embed the atlas PNG at compile time
const INTER_ATLAS_PNG: &[u8] = include_bytes!("../assets/fonts/inter_sdf_atlas.png");

/// Vertex data for a text quad
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct TextVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

impl TextVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x2,  // position
        1 => Float32x2,  // uv
        2 => Float32x4,  // color
    ];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TextVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Uniform data for text rendering
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct TextUniforms {
    resolution: [f32; 2],
    sdf_range: f32,
    _padding: f32,
}

/// Maximum number of glyphs we can render in a single draw call
const MAX_GLYPHS: usize = 1024;

#[allow(dead_code)] // Fields kept to maintain wgpu resource ownership
pub struct TextPipeline {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    atlas_texture: wgpu::Texture,
    font_atlas: FontAtlas,
    uniforms: TextUniforms,
    glyph_count: u32,
    // Accumulated vertices for batch rendering
    pending_vertices: Vec<TextVertex>,
}

impl TextPipeline {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        width: f32,
        height: f32,
    ) -> Self {
        // Load font atlas metadata
        let font_atlas = crate::font::load_inter_atlas()
            .expect("Failed to load Inter font atlas");

        // Load atlas texture from embedded PNG
        let atlas_image = image::load_from_memory(INTER_ATLAS_PNG)
            .expect("Failed to decode atlas PNG")
            .into_rgba8();

        let atlas_dimensions = atlas_image.dimensions();

        // Create texture
        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("MTSDF Atlas Texture"),
            size: wgpu::Extent3d {
                width: atlas_dimensions.0,
                height: atlas_dimensions.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload texture data
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas_image,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * atlas_dimensions.0),
                rows_per_image: Some(atlas_dimensions.1),
            },
            wgpu::Extent3d {
                width: atlas_dimensions.0,
                height: atlas_dimensions.1,
                depth_or_array_layers: 1,
            },
        );

        // Create texture view and sampler
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("MTSDF Atlas Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create uniform buffer
        let uniforms = TextUniforms {
            resolution: [width, height],
            sdf_range: font_atlas.sdf_range,
            _padding: 0.0,
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Text Uniforms Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create vertex buffer (empty, will be filled per-frame)
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Text Vertex Buffer"),
            size: (MAX_GLYPHS * 4 * std::mem::size_of::<TextVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create index buffer (6 indices per quad: 0,1,2, 2,3,0)
        let mut indices: Vec<u16> = Vec::with_capacity(MAX_GLYPHS * 6);
        for i in 0..MAX_GLYPHS {
            let base = (i * 4) as u16;
            indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
        }

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Text Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Text Bind Group Layout"),
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
                // Atlas texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Atlas sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Text Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("MTSDF Text Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/mtsdf_text.wgsl").into()),
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Text Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Text Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[TextVertex::desc()],
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
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            uniform_buffer,
            vertex_buffer,
            index_buffer,
            bind_group_layout,
            bind_group,
            atlas_texture,
            font_atlas,
            uniforms,
            glyph_count: 0,
            pending_vertices: Vec::new(),
        }
    }

    /// Clear all pending text (call before starting a new frame)
    pub fn clear_batch(&mut self) {
        self.pending_vertices.clear();
        self.glyph_count = 0;
    }

    /// Add text at a specific position (left-aligned)
    pub fn add_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: [f32; 4]) {
        let layout = self.font_atlas.layout_text(text, x, y, font_size);
        self.add_glyphs(&layout.glyphs, color);
    }

    /// Add centered text at a specific position
    pub fn add_centered_text(&mut self, text: &str, center_x: f32, center_y: f32, font_size: f32, color: [f32; 4]) {
        let layout = self.font_atlas.layout_text_centered(text, center_x, center_y, font_size);
        self.add_glyphs(&layout.glyphs, color);
    }

    /// Add glyphs to the pending batch
    fn add_glyphs(&mut self, glyphs: &[PositionedGlyph], color: [f32; 4]) {
        for glyph in glyphs {
            if glyph.char == ' ' {
                continue;
            }

            let x0 = glyph.x;
            let y0 = glyph.y;
            let x1 = glyph.x + glyph.width;
            let y1 = glyph.y + glyph.height;

            let u0 = glyph.uv_x;
            let v0 = glyph.uv_y;
            let u1 = glyph.uv_x + glyph.uv_width;
            let v1 = glyph.uv_y + glyph.uv_height;

            self.pending_vertices.push(TextVertex { position: [x0, y0], uv: [u0, v0], color });
            self.pending_vertices.push(TextVertex { position: [x1, y0], uv: [u1, v0], color });
            self.pending_vertices.push(TextVertex { position: [x1, y1], uv: [u1, v1], color });
            self.pending_vertices.push(TextVertex { position: [x0, y1], uv: [u0, v1], color });
        }
    }

    /// Upload all pending text to GPU (call after adding all text)
    pub fn flush_batch(&mut self, queue: &wgpu::Queue) {
        self.glyph_count = (self.pending_vertices.len() / 4) as u32;
        if !self.pending_vertices.is_empty() {
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&self.pending_vertices));
        }
    }

    /// Render the prepared text
    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        if self.glyph_count == 0 {
            return;
        }

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Text Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load, // Don't clear - render on top
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..(self.glyph_count * 6), 0, 0..1);
    }
}
