/// Shadow rendering pipeline for WebGPU
///
/// Renders box shadows as semi-transparent rectangles using instanced rendering.
/// Shadows are rendered BEFORE rectangles to ensure proper layering.

/// Instance data for a unified multi-layer shadow
/// Supports up to 2 shadow layers blended together
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShadowInstance {
    /// Position (x, y) in screen coordinates
    pub position: [f32; 2],
    /// Size (width, height) in pixels - includes blur expansion
    pub size: [f32; 2],
    /// Content rectangle size (without blur expansion)
    pub content_size: [f32; 2],

    // Layer 1 (usually the small sharp shadow)
    pub color1: [f32; 4],
    pub blur_radius1: f32,
    pub offset1: [f32; 2],

    // Layer 2 (usually the large diffuse shadow)
    pub color2: [f32; 4],
    pub blur_radius2: f32,
    pub offset2: [f32; 2],

    /// Padding for alignment
    pub _padding: [f32; 2],
}

impl ShadowInstance {
    /// Create a unified shadow with two layers blended together
    pub fn new_dual_layer(
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        content_width: f32,
        content_height: f32,
        color1: [f32; 4],
        blur1: f32,
        offset1: [f32; 2],
        color2: [f32; 4],
        blur2: f32,
        offset2: [f32; 2],
    ) -> Self {
        Self {
            position: [x, y],
            size: [width, height],
            content_size: [content_width, content_height],
            color1,
            blur_radius1: blur1,
            offset1,
            color2,
            blur_radius2: blur2,
            offset2,
            _padding: [0.0, 0.0],
        }
    }

    /// Vertex buffer layout descriptor
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ShadowInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // size
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // content_size
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color1
                wgpu::VertexAttribute {
                    offset: 24,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // blur_radius1
                wgpu::VertexAttribute {
                    offset: 40,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32,
                },
                // offset1
                wgpu::VertexAttribute {
                    offset: 44,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color2
                wgpu::VertexAttribute {
                    offset: 52,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // blur_radius2
                wgpu::VertexAttribute {
                    offset: 68,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32,
                },
                // offset2
                wgpu::VertexAttribute {
                    offset: 72,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// Shadow rendering pipeline
pub struct ShadowPipeline {
    render_pipeline: wgpu::RenderPipeline,
    instance_buffer: wgpu::Buffer,
    capacity: usize,
}

impl ShadowPipeline {
    /// Create a new shadow rendering pipeline
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        _viewport_width: u32,
        _viewport_height: u32,
        initial_capacity: usize,
    ) -> Self {
        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shadow Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shadow_shader.wgsl").into()),
        });

        // Create render pipeline
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Shadow Pipeline Layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[ShadowInstance::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
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
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        // Create instance buffer
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Instance Buffer"),
            size: (initial_capacity * std::mem::size_of::<ShadowInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            render_pipeline,
            instance_buffer,
            capacity: initial_capacity,
        }
    }

    /// Render shadows to the given texture view
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        shadows: &[ShadowInstance],
    ) {
        // Always create render pass to clear screen, even with 0 shadows

        // Resize buffer if needed (only if we have shadows to render)
        if !shadows.is_empty() && shadows.len() > self.capacity {
            self.capacity = shadows.len().next_power_of_two();
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Shadow Instance Buffer"),
                size: (self.capacity * std::mem::size_of::<ShadowInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        // Upload instance data (only if we have shadows)
        if !shadows.is_empty() {
            queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(shadows));
        }

        // Create command encoder
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Shadow Render Encoder"),
        });

        // Render pass (always create to clear screen)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shadow Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.96, // #f5f5f5 background
                            g: 0.96,
                            b: 0.96,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Only draw if we have shadows
            if !shadows.is_empty() {
                render_pass.set_pipeline(&self.render_pipeline);
                render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));

                // Draw 6 vertices per instance (2 triangles = 1 quad)
                render_pass.draw(0..6, 0..shadows.len() as u32);
            }
        }

        queue.submit(std::iter::once(encoder.finish()));
    }
}
