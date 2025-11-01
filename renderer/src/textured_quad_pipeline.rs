use wgpu;

/// Instance data for a textured quad (for rendering text)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TexturedQuadInstance {
    /// Position and size in CSS pixels (x, y, width, height)
    pub position_size: [f32; 4],
}

impl TexturedQuadInstance {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            position_size: [x, y, width, height],
        }
    }

    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TexturedQuadInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x4,
            }],
        }
    }
}

pub struct TexturedQuadPipeline {
    render_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
}

impl TexturedQuadPipeline {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        viewport_width: u32,
        viewport_height: u32,
        initial_capacity: usize,
    ) -> Self {
        let viewport_width = viewport_width as f32;
        let viewport_height = viewport_height as f32;

        // Create bind group layout for texture + sampler
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Text Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
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

        // WGSL shader for textured quad rendering
        let shader_source = format!(
            r#"
const VIEWPORT_WIDTH: f32 = {};
const VIEWPORT_HEIGHT: f32 = {};

struct VertexInput {{
    @builtin(vertex_index) vertex_index: u32,
}}

struct InstanceInput {{
    @location(0) position_size: vec4<f32>,  // x, y, width, height (CSS pixels)
}}

struct VertexOutput {{
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
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
    var tex: vec2<f32>;
    let idx = vertex.vertex_index % 6u;

    if (idx == 0u) {{
        pos = css_to_ndc(x, y);
        tex = vec2<f32>(0.0, 0.0);
    }} else if (idx == 1u || idx == 4u) {{
        pos = css_to_ndc(x + w, y);
        tex = vec2<f32>(1.0, 0.0);
    }} else if (idx == 2u || idx == 3u) {{
        pos = css_to_ndc(x, y + h);
        tex = vec2<f32>(0.0, 1.0);
    }} else {{
        pos = css_to_ndc(x + w, y + h);
        tex = vec2<f32>(1.0, 1.0);
    }}

    out.position = vec4<f32>(pos, 0.0, 1.0);
    out.tex_coords = tex;

    return out;
}}

@group(0) @binding(0)
var t_texture: texture_2d<f32>;

@group(0) @binding(1)
var t_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {{
    return textureSample(t_texture, t_sampler, in.tex_coords);
}}
"#,
            viewport_width, viewport_height
        );

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Textured Quad Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Textured Quad Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Textured Quad Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[TexturedQuadInstance::desc()],
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

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Textured Quad Instance Buffer"),
            size: (initial_capacity * std::mem::size_of::<TexturedQuadInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            render_pipeline,
            bind_group_layout,
            instance_buffer,
            instance_capacity: initial_capacity,
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    /// Render textured quads with the given instances and bind groups
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        instances: &[TexturedQuadInstance],
        bind_groups: &[&wgpu::BindGroup],
    ) {
        if instances.is_empty() {
            return;
        }

        // Resize buffer if needed
        if instances.len() > self.instance_capacity {
            let new_capacity = (instances.len() * 2).max(64);
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Textured Quad Instance Buffer"),
                size: (new_capacity * std::mem::size_of::<TexturedQuadInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_capacity = new_capacity;
        }

        // Upload instance data
        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(instances));

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Textured Quad Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load, // Don't clear, we're drawing on top of rectangles
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

        // Draw each textured quad with its bind group
        for (i, bind_group) in bind_groups.iter().enumerate() {
            render_pass.set_bind_group(0, Some(*bind_group), &[]);
            render_pass.draw(0..6, i as u32..(i as u32 + 1));
        }
    }
}
