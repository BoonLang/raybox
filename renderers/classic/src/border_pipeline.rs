use wgpu;

/// Instance data for a border edge (thin colored rectangle)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BorderInstance {
    /// Position and size in CSS pixels (x, y, width, height)
    pub position_size: [f32; 4],
    /// RGBA color (normalized 0.0-1.0)
    pub color: [f32; 4],
}

impl BorderInstance {
    pub fn new(x: f32, y: f32, width: f32, height: f32, color: [f32; 4]) -> Self {
        Self {
            position_size: [x, y, width, height],
            color,
        }
    }

    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<BorderInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Create 4 border edge instances for a box
pub fn create_border_edges(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    border_width: f32,
    color: [f32; 4],
) -> [BorderInstance; 4] {
    [
        // Top edge
        BorderInstance::new(x, y, width, border_width, color),
        // Right edge
        BorderInstance::new(x + width - border_width, y, border_width, height, color),
        // Bottom edge
        BorderInstance::new(x, y + height - border_width, width, border_width, color),
        // Left edge
        BorderInstance::new(x, y, border_width, height, color),
    ]
}

pub struct BorderPipeline {
    render_pipeline: wgpu::RenderPipeline,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
}

impl BorderPipeline {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        viewport_width: u32,
        viewport_height: u32,
        initial_capacity: usize,
    ) -> Self {
        let viewport_width = viewport_width as f32;
        let viewport_height = viewport_height as f32;

        // WGSL shader for border rendering (same as rectangle but with per-instance colors)
        let shader_source = format!(
            r#"
const VIEWPORT_WIDTH: f32 = {};
const VIEWPORT_HEIGHT: f32 = {};

struct VertexInput {{
    @builtin(vertex_index) vertex_index: u32,
}}

struct InstanceInput {{
    @location(0) position_size: vec4<f32>,  // x, y, width, height (CSS pixels)
    @location(1) color: vec4<f32>,          // RGBA color
}}

struct VertexOutput {{
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}}

fn css_to_ndc(x: f32, y: f32) -> vec2<f32> {{
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

    var pos: vec2<f32>;
    let idx = vertex.vertex_index % 6u;

    if (idx == 0u) {{
        pos = css_to_ndc(x, y);
    }} else if (idx == 1u || idx == 4u) {{
        pos = css_to_ndc(x + w, y);
    }} else if (idx == 2u || idx == 3u) {{
        pos = css_to_ndc(x, y + h);
    }} else {{
        pos = css_to_ndc(x + w, y + h);
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

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Border Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Border Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Border Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[BorderInstance::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
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

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Border Instance Buffer"),
            size: (initial_capacity * std::mem::size_of::<BorderInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            render_pipeline,
            instance_buffer,
            instance_capacity: initial_capacity,
        }
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        instances: &[BorderInstance],
    ) {
        if instances.is_empty() {
            return;
        }

        // Resize buffer if needed
        if instances.len() > self.instance_capacity {
            self.instance_capacity = instances.len() * 2;
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Border Instance Buffer"),
                size: (self.instance_capacity * std::mem::size_of::<BorderInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        // Upload instance data
        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(instances));

        // Create command encoder and render pass
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Border Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Border Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Don't clear, draw on top
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
            render_pass.draw(0..6, 0..instances.len() as u32);
        }

        queue.submit(std::iter::once(encoder.finish()));
    }
}
