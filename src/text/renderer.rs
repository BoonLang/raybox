//! MSDF text renderer using wgpu
//!
//! Renders text using instanced quads with MSDF texture sampling.

use crate::text::atlas::MsdfAtlas;
use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// Vertex for text quad rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct TextVertex {
    /// Position (x, y)
    pub position: [f32; 2],
    /// UV coordinates
    pub uv: [f32; 2],
}

impl TextVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TextVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// Instance data for each glyph
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GlyphInstance {
    /// Position offset (x, y)
    pub offset: [f32; 2],
    /// Size (width, height)
    pub size: [f32; 2],
    /// UV min (u, v)
    pub uv_min: [f32; 2],
    /// UV max (u, v)
    pub uv_max: [f32; 2],
    /// Color (r, g, b, a)
    pub color: [f32; 4],
}

impl GlyphInstance {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GlyphInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // offset
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // size
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // uv_min
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // uv_max
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Text rendering uniforms
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct TextUniforms {
    /// Screen dimensions (width, height)
    pub screen_size: [f32; 2],
    /// SDF parameters: (px_range, _reserved)
    pub sdf_params: [f32; 2],
}

/// MSDF Text Renderer
#[allow(dead_code)]
pub struct TextRenderer {
    atlas: MsdfAtlas,
    atlas_texture: wgpu::Texture,
    atlas_view: wgpu::TextureView,
    atlas_sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    max_instances: u32,
    /// Distance field range in pixels (from atlas)
    distance_range: f32,
}

impl TextRenderer {
    /// Create a new text renderer
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        atlas: MsdfAtlas,
        atlas_data: &[u8],
    ) -> Result<Self> {
        // Determine if input is RGB or RGBA based on data length
        let expected_rgb = (atlas.width * atlas.height * 3) as usize;
        let expected_rgba = (atlas.width * atlas.height * 4) as usize;
        let is_rgba = atlas_data.len() == expected_rgba;

        // Create atlas texture
        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("MSDF Atlas Texture"),
            size: wgpu::Extent3d {
                width: atlas.width,
                height: atlas.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Convert to RGBA if needed
        let rgba_data: Vec<u8> = if is_rgba {
            atlas_data.to_vec()
        } else if atlas_data.len() == expected_rgb {
            let mut rgba = Vec::with_capacity(expected_rgba);
            for i in 0..(atlas.width * atlas.height) as usize {
                rgba.push(atlas_data[i * 3]);
                rgba.push(atlas_data[i * 3 + 1]);
                rgba.push(atlas_data[i * 3 + 2]);
                rgba.push(255);
            }
            rgba
        } else {
            anyhow::bail!(
                "Invalid atlas data length: {} (expected {} for RGB or {} for RGBA)",
                atlas_data.len(),
                expected_rgb,
                expected_rgba
            );
        };

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(atlas.width * 4),
                rows_per_image: Some(atlas.height),
            },
            wgpu::Extent3d {
                width: atlas.width,
                height: atlas.height,
                depth_or_array_layers: 1,
            },
        );

        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("MSDF Atlas Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Store distance range from atlas
        let distance_range = atlas.distance_range;

        // Create uniform buffer
        let uniforms = TextUniforms {
            screen_size: [800.0, 600.0],
            sdf_params: [distance_range, 0.0],
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Text Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Bind group layout
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
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

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
            label: Some("MSDF Text Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/text_msdf.wgsl").into()),
        });

        // Create pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Text Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Text Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[TextVertex::desc(), GlyphInstance::desc()],
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
            label: Some("Text Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Create instance buffer
        let max_instances = 10000;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Text Instance Buffer"),
            size: (max_instances as usize * std::mem::size_of::<GlyphInstance>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            atlas,
            atlas_texture,
            atlas_view,
            atlas_sampler,
            bind_group_layout,
            bind_group,
            pipeline,
            uniform_buffer,
            vertex_buffer,
            instance_buffer,
            max_instances,
            distance_range,
        })
    }

    /// Update screen size
    pub fn update_screen_size(&self, queue: &wgpu::Queue, width: f32, height: f32) {
        let uniforms = TextUniforms {
            screen_size: [width, height],
            sdf_params: [self.distance_range, 0.0],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Layout text and return glyph instances
    /// x, y: top-left position of the text block (baseline at y + ascender * font_size)
    /// font_size: height of the text in em units (pixels)
    pub fn layout_text(
        &self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        color: [f32; 4],
    ) -> Vec<GlyphInstance> {
        let mut instances = Vec::new();
        let mut cursor_x = x;

        // Scale factor: font_size is the em height
        let scale = font_size;

        // Get font metrics for baseline calculation
        let ascender = self.atlas.metrics.ascender;

        // Baseline position (y is top of line, baseline is y + ascender * scale)
        let baseline_y = y + ascender * scale;

        let chars: Vec<char> = text.chars().collect();
        for (i, &ch) in chars.iter().enumerate() {
            if ch == '\n' {
                continue;
            }

            if let Some(glyph) = self.atlas.get_glyph(ch) {
                // Apply kerning if there's a next character
                if i > 0 {
                    let kern = self.atlas.get_kerning(chars[i - 1], ch);
                    cursor_x += kern * scale;
                }

                if let Some((uv_min_x, uv_min_y, uv_max_x, uv_max_y)) = glyph.uvs {
                    if let Some((pb_left, pb_bottom, pb_right, pb_top)) = glyph.plane_bounds {
                        // Calculate glyph quad position using plane bounds
                        // plane_bounds are in em units relative to the baseline
                        let glyph_x = cursor_x + pb_left * scale;
                        let glyph_y = baseline_y - pb_top * scale; // Y is flipped for screen coords
                        let glyph_w = (pb_right - pb_left) * scale;
                        let glyph_h = (pb_top - pb_bottom) * scale;

                        instances.push(GlyphInstance {
                            offset: [glyph_x, glyph_y],
                            size: [glyph_w, glyph_h],
                            uv_min: [uv_min_x, uv_min_y],
                            uv_max: [uv_max_x, uv_max_y],
                            color,
                        });
                    }
                }

                // Advance cursor by glyph advance width
                cursor_x += glyph.advance * scale;
            }
        }

        instances
    }

    /// Layout multi-line text with automatic line wrapping
    pub fn layout_text_block(
        &self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        line_height: f32,
        color: [f32; 4],
    ) -> Vec<GlyphInstance> {
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
        instances: &[GlyphInstance],
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
    pub fn atlas(&self) -> &MsdfAtlas {
        &self.atlas
    }
}
