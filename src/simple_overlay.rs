//! Simple 2D text overlay using cosmic-text for CPU rasterization
//!
//! This module provides a straightforward text overlay system that:
//! - Uses CPU-rasterized text (cosmic-text) for reliable, crisp rendering
//! - Uploads text to a GPU texture for efficient display
//! - Works identically on native and web platforms
//! - Has minimal impact on demo resource measurements

use anyhow::{Context, Result};
use cosmic_text::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache,
};
use std::time::Instant;
use wgpu::util::DeviceExt;

/// Simple 2D text overlay renderer
pub struct SimpleOverlay {
    font_system: FontSystem,
    swash_cache: SwashCache,
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    // Current text content
    stats_text: String,
    keybindings_text: String,
    // Texture dimensions
    tex_width: u32,
    tex_height: u32,
    // Screen dimensions
    screen_width: u32,
    screen_height: u32,
    // Whether content changed and needs re-rasterization
    dirty: bool,
    // Throttle stats updates to avoid re-rasterizing every frame
    last_stats_update: Instant,
}

// Vertex for textured quad
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct OverlayVertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

impl OverlayVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<OverlayVertex>() as wgpu::BufferAddress,
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

// Shader for overlay rendering
const OVERLAY_SHADER: &str = r#"
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(in.position, 0.0, 1.0);
    out.tex_coords = in.tex_coords;
    return out;
}

@group(0) @binding(0)
var t_text: texture_2d<f32>;
@group(0) @binding(1)
var s_text: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_text, s_text, in.tex_coords);
    // Pre-multiplied alpha blending
    return tex_color;
}
"#;

impl SimpleOverlay {
    /// Create a new simple overlay renderer
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        // Initialize cosmic-text font system
        let mut font_system = FontSystem::new();

        // Load the DejaVu Sans font
        let font_data = std::fs::read("assets/fonts/DejaVuSans.ttf")
            .context("Failed to load font file")?;
        font_system.db_mut().load_font_data(font_data);

        let swash_cache = SwashCache::new();

        // Create texture for text rendering (start with screen size)
        let tex_width = width;
        let tex_height = height;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Overlay Text Texture"),
            size: wgpu::Extent3d {
                width: tex_width,
                height: tex_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Overlay Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Overlay Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Overlay Shader"),
            source: wgpu::ShaderSource::Wgsl(OVERLAY_SHADER.into()),
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Overlay Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Overlay Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[OverlayVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
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

        // Create vertex buffer for fullscreen quad
        let vertices: [OverlayVertex; 6] = [
            OverlayVertex { position: [-1.0, -1.0], tex_coords: [0.0, 1.0] },
            OverlayVertex { position: [1.0, -1.0], tex_coords: [1.0, 1.0] },
            OverlayVertex { position: [1.0, 1.0], tex_coords: [1.0, 0.0] },
            OverlayVertex { position: [-1.0, -1.0], tex_coords: [0.0, 1.0] },
            OverlayVertex { position: [1.0, 1.0], tex_coords: [1.0, 0.0] },
            OverlayVertex { position: [-1.0, 1.0], tex_coords: [0.0, 0.0] },
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Overlay Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Ok(Self {
            font_system,
            swash_cache,
            texture,
            texture_view,
            sampler,
            bind_group,
            bind_group_layout,
            pipeline,
            vertex_buffer,
            stats_text: String::new(),
            keybindings_text: String::new(),
            tex_width,
            tex_height,
            screen_width: width,
            screen_height: height,
            dirty: true,
            last_stats_update: Instant::now(),
        })
    }

    /// Update the overlay content
    pub fn update(
        &mut self,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
        stats: &str,
        keybindings: Option<&[(&str, &str)]>,
        width: u32,
        height: u32,
    ) {
        // Check if resize needed
        if width != self.screen_width || height != self.screen_height {
            self.screen_width = width;
            self.screen_height = height;
            self.resize_texture(device, width, height);
            self.dirty = true;
        }

        // Check if content changed (throttle stats to ~4/sec to avoid
        // re-rasterizing + uploading 1.92MB texture every frame)
        let now = Instant::now();
        let stats_interval_elapsed = now.duration_since(self.last_stats_update).as_millis() >= 250;

        let new_keybindings_text = if let Some(bindings) = keybindings {
            bindings
                .iter()
                .map(|(key, desc)| format!("{}: {}", key, desc))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            String::new()
        };

        // Keybindings changes are rare, always check. Stats change every frame, throttle.
        if new_keybindings_text != self.keybindings_text {
            self.keybindings_text = new_keybindings_text;
            self.dirty = true;
        }

        if stats_interval_elapsed && stats != self.stats_text {
            self.stats_text = stats.to_string();
            self.last_stats_update = now;
            self.dirty = true;
        }

        // Re-rasterize if needed
        if self.dirty {
            self.rasterize_text(queue);
            self.dirty = false;
        }
    }

    fn resize_texture(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.tex_width = width;
        self.tex_height = height;

        self.texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Overlay Text Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.texture_view = self.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Recreate bind group with new texture view
        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Overlay Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
    }

    fn rasterize_text(&mut self, queue: &wgpu::Queue) {
        // Create pixel buffer (RGBA)
        let mut pixels = vec![0u8; (self.tex_width * self.tex_height * 4) as usize];

        let font_size = 14.0;
        let line_height = font_size * 1.4;
        let margin = 10.0;

        // Rasterize stats (top-left)
        if !self.stats_text.is_empty() {
            self.render_text_block(
                &mut pixels,
                &self.stats_text.clone(),
                margin,
                margin,
                font_size,
                line_height,
                TextAlign::Left,
            );
        }

        // Rasterize keybindings (top-right)
        if !self.keybindings_text.is_empty() {
            self.render_text_block(
                &mut pixels,
                &self.keybindings_text.clone(),
                self.screen_width as f32 - margin,
                margin,
                font_size,
                line_height,
                TextAlign::Right,
            );
        }

        // Upload to GPU
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.tex_width * 4),
                rows_per_image: Some(self.tex_height),
            },
            wgpu::Extent3d {
                width: self.tex_width,
                height: self.tex_height,
                depth_or_array_layers: 1,
            },
        );
    }

    fn render_text_block(
        &mut self,
        pixels: &mut [u8],
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        line_height: f32,
        align: TextAlign,
    ) {
        let metrics = Metrics::new(font_size, line_height);
        let attrs = Attrs::new()
            .family(Family::SansSerif);

        let mut buffer = Buffer::new(&mut self.font_system, metrics);

        // Set buffer size
        let max_width = match align {
            TextAlign::Left => (self.screen_width as f32 - x - 10.0) as i32,
            TextAlign::Right => (x - 10.0) as i32,
        };
        buffer.set_size(&mut self.font_system, Some(max_width as f32), None);
        buffer.set_text(&mut self.font_system, text, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, false);

        // Calculate text bounds for right alignment
        let text_width = if align == TextAlign::Right {
            let mut max_line_width = 0.0f32;
            for run in buffer.layout_runs() {
                max_line_width = max_line_width.max(run.line_w);
            }
            max_line_width
        } else {
            0.0
        };

        let start_x = match align {
            TextAlign::Left => x,
            TextAlign::Right => x - text_width,
        };

        // Extract dimensions for use in closure
        let tex_width = self.tex_width;
        let tex_height = self.tex_height;

        // Render glyphs
        buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            Color::rgba(255, 255, 255, 230), // White with slight transparency
            |glyph_x, glyph_y, w, h, color| {
                let px = (start_x + glyph_x as f32) as i32;
                let py = (y + glyph_y as f32) as i32;

                // Draw each pixel of the glyph with outline effect
                for dy in 0..h as i32 {
                    for dx in 0..w as i32 {
                        let sx = px + dx;
                        let sy = py + dy;

                        if sx >= 0 && sx < tex_width as i32
                           && sy >= 0 && sy < tex_height as i32
                        {
                            let idx = ((sy as u32 * tex_width + sx as u32) * 4) as usize;

                            // Get glyph alpha
                            let glyph_a = color.a();
                            if glyph_a > 0 {
                                // Draw dark outline/shadow for readability
                                draw_outline(pixels, tex_width, tex_height, sx, sy, glyph_a);

                                // Draw the actual glyph (white text)
                                let dst_a = pixels[idx + 3] as f32 / 255.0;
                                let src_a = glyph_a as f32 / 255.0;
                                let out_a = src_a + dst_a * (1.0 - src_a);

                                if out_a > 0.0 {
                                    let blend = |src: u8, dst: u8| -> u8 {
                                        let src_c = src as f32 / 255.0 * src_a;
                                        let dst_c = dst as f32 / 255.0 * dst_a;
                                        let out_c = (src_c + dst_c * (1.0 - src_a)) / out_a;
                                        (out_c * 255.0).min(255.0) as u8
                                    };

                                    pixels[idx] = blend(color.r(), pixels[idx]);
                                    pixels[idx + 1] = blend(color.g(), pixels[idx + 1]);
                                    pixels[idx + 2] = blend(color.b(), pixels[idx + 2]);
                                    pixels[idx + 3] = (out_a * 255.0).min(255.0) as u8;
                                }
                            }
                        }
                    }
                }
            },
        );
    }

    /// Render the overlay
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        // Only render if there's content
        if self.stats_text.is_empty() && self.keybindings_text.is_empty() {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..6, 0..1);
    }

    /// Handle resize
    pub fn resize(&mut self, _width: u32, _height: u32) {
        // Resize is handled in update() when dimensions change
    }
}

#[derive(Clone, Copy, PartialEq)]
enum TextAlign {
    Left,
    Right,
}

/// Draw a dark outline around a glyph pixel for readability
fn draw_outline(pixels: &mut [u8], tex_width: u32, tex_height: u32, cx: i32, cy: i32, alpha: u8) {
    let outline_color = [0u8, 0, 0, (alpha as f32 * 0.8) as u8]; // Dark with reduced alpha

    for dy in -1..=1i32 {
        for dx in -1..=1i32 {
            if dx == 0 && dy == 0 {
                continue;
            }

            let ox = cx + dx;
            let oy = cy + dy;

            if ox >= 0 && ox < tex_width as i32
               && oy >= 0 && oy < tex_height as i32
            {
                let idx = ((oy as u32 * tex_width + ox as u32) * 4) as usize;

                // Only draw outline if there's no existing content
                if pixels[idx + 3] < outline_color[3] {
                    // Pre-multiplied alpha
                    let a = outline_color[3] as f32 / 255.0;
                    pixels[idx] = (outline_color[0] as f32 * a) as u8;
                    pixels[idx + 1] = (outline_color[1] as f32 * a) as u8;
                    pixels[idx + 2] = (outline_color[2] as f32 * a) as u8;
                    pixels[idx + 3] = outline_color[3];
                }
            }
        }
    }
}
