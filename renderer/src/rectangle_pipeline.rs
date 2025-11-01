use wgpu;

/// Instance data for a single rectangle
/// Sent to GPU for instanced rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RectangleInstance {
    /// Position and size in CSS pixels (x, y, width, height)
    pub position_size: [f32; 4],
    /// Color as RGBA (0.0 - 1.0)
    pub color: [f32; 4],
}

impl RectangleInstance {
    pub fn new(x: f32, y: f32, width: f32, height: f32, color: [f32; 4]) -> Self {
        Self {
            position_size: [x, y, width, height],
            color,
        }
    }

    /// Vertex buffer layout for instanced rendering
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<RectangleInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // position_size (vec4)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // color (vec4)
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

pub struct RectanglePipeline {
    render_pipeline: wgpu::RenderPipeline,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    viewport_width: f32,
    viewport_height: f32,
}

impl RectanglePipeline {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        viewport_width: u32,
        viewport_height: u32,
        initial_capacity: usize,
    ) -> Self {
        let viewport_width = viewport_width as f32;
        let viewport_height = viewport_height as f32;

        // WGSL shader for rectangle rendering
        let shader_source = format!(
            r#"
// Viewport dimensions for coordinate transformation
const VIEWPORT_WIDTH: f32 = {};
const VIEWPORT_HEIGHT: f32 = {};

struct VertexInput {{
    @builtin(vertex_index) vertex_index: u32,
}}

struct InstanceInput {{
    @location(0) position_size: vec4<f32>,  // x, y, width, height (CSS pixels)
    @location(1) color: vec4<f32>,          // rgba
}}

struct VertexOutput {{
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}}

// Transform CSS pixel coordinates to NDC (Normalized Device Coordinates)
fn css_to_ndc(x: f32, y: f32) -> vec2<f32> {{
    // CSS: origin top-left, y goes down
    // NDC: origin center, y goes up, range [-1, 1]
    let x_ndc = (x / VIEWPORT_WIDTH) * 2.0 - 1.0;
    let y_ndc = 1.0 - (y / VIEWPORT_HEIGHT) * 2.0;
    return vec2<f32>(x_ndc, y_ndc);
}}

@vertex
fn vs_main(
    vertex: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {{
    var out: VertexOutput;

    let x = instance.position_size.x;
    let y = instance.position_size.y;
    let w = instance.position_size.z;
    let h = instance.position_size.w;

    // Generate quad vertices (two triangles)
    // 0: top-left, 1: top-right, 2: bottom-left
    // 3: bottom-left, 4: top-right, 5: bottom-right
    var pos: vec2<f32>;
    let idx = vertex.vertex_index % 6u;

    if (idx == 0u) {{
        pos = css_to_ndc(x, y);  // top-left
    }} else if (idx == 1u || idx == 4u) {{
        pos = css_to_ndc(x + w, y);  // top-right
    }} else if (idx == 2u || idx == 3u) {{
        pos = css_to_ndc(x, y + h);  // bottom-left
    }} else {{
        pos = css_to_ndc(x + w, y + h);  // bottom-right
    }}

    out.position = vec4<f32>(pos, 0.0, 1.0);
    out.color = instance.color;

    return out;
}}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {{
    return in.color;
}}
"#,
            viewport_width, viewport_height
        );

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Rectangle Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Rectangle Pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[RectangleInstance::desc()],
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
                cull_mode: None,  // No culling for 2D rectangles
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
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

        // Create instance buffer with initial capacity
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Rectangle Instance Buffer"),
            size: (initial_capacity * std::mem::size_of::<RectangleInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            render_pipeline,
            instance_buffer,
            instance_capacity: initial_capacity,
            viewport_width,
            viewport_height,
        }
    }

    /// Render rectangles with the given instances
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        instances: &[RectangleInstance],
    ) {
        // Resize buffer if needed
        if instances.len() > self.instance_capacity {
            let new_capacity = (instances.len() * 2).max(64);
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Rectangle Instance Buffer"),
                size: (new_capacity * std::mem::size_of::<RectangleInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_capacity = new_capacity;
        }

        // Upload instance data
        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(instances));

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Rectangle Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Rectangle Render Pass"),
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

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));

            // Draw 6 vertices per instance (2 triangles = 1 quad)
            render_pass.draw(0..6, 0..instances.len() as u32);
        }

        queue.submit(std::iter::once(encoder.finish()));
    }
}
