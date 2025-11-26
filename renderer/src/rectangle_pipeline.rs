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
    /// Border radius in pixels (uniform for all corners for now)
    pub border_radius: f32,
    /// Border width for outline rendering (0.0 = filled, >0 = outline)
    pub border_width: f32,
    /// Padding for alignment (2 floats to align to 16 bytes)
    pub _padding: [f32; 2],

    // Inset shadow (rendered inside the rectangle bounds)
    /// Inset shadow color (RGBA)
    pub inset_shadow_color: [f32; 4],
    /// Inset shadow blur radius
    pub inset_shadow_blur: f32,
    /// Inset shadow offset (x, y)
    pub inset_shadow_offset: [f32; 2],
    /// Whether inset shadow is enabled (0.0 = no, 1.0 = yes)
    pub has_inset_shadow: f32,
}

impl RectangleInstance {
    pub fn new(x: f32, y: f32, width: f32, height: f32, color: [f32; 4]) -> Self {
        Self::new_with_radius(x, y, width, height, color, 0.0)
    }

    pub fn new_with_radius(
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        border_radius: f32,
    ) -> Self {
        Self {
            position_size: [x, y, width, height],
            color,
            border_radius,
            border_width: 0.0,
            _padding: [0.0, 0.0],
            inset_shadow_color: [0.0, 0.0, 0.0, 0.0],
            inset_shadow_blur: 0.0,
            inset_shadow_offset: [0.0, 0.0],
            has_inset_shadow: 0.0,
        }
    }

    /// Create a rounded border outline (ring shape)
    pub fn new_border_outline(
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        border_radius: f32,
        border_width: f32,
    ) -> Self {
        Self {
            position_size: [x, y, width, height],
            color,
            border_radius,
            border_width,
            _padding: [0.0, 0.0],
            inset_shadow_color: [0.0, 0.0, 0.0, 0.0],
            inset_shadow_blur: 0.0,
            inset_shadow_offset: [0.0, 0.0],
            has_inset_shadow: 0.0,
        }
    }

    /// Create a rectangle with inset shadow
    pub fn new_with_inset_shadow(
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        border_radius: f32,
        shadow_color: [f32; 4],
        shadow_blur: f32,
        shadow_offset: [f32; 2],
    ) -> Self {
        Self {
            position_size: [x, y, width, height],
            color,
            border_radius,
            border_width: 0.0,
            _padding: [0.0, 0.0],
            inset_shadow_color: shadow_color,
            inset_shadow_blur: shadow_blur,
            inset_shadow_offset: shadow_offset,
            has_inset_shadow: 1.0,
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
                // border_radius (float) + border_width (float) + padding (vec2) = vec4
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 4]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // inset_shadow_color (vec4)
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 4]>() * 3) as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // inset_shadow_blur (float) + inset_shadow_offset (vec2) + has_inset_shadow (float) = vec4
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 4]>() * 4) as wgpu::BufferAddress,
                    shader_location: 4,
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
}

impl RectanglePipeline {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        viewport_width: u32,
        viewport_height: u32,
        initial_capacity: usize,
    ) -> Self {
        // WGSL shader for rectangle rendering with rounded corners (SDF)
        let shader_source = format!(
            r#"
// Viewport dimensions for coordinate transformation
const VIEWPORT_WIDTH: f32 = {};
const VIEWPORT_HEIGHT: f32 = {};

struct VertexInput {{
    @builtin(vertex_index) vertex_index: u32,
}}

struct InstanceInput {{
    @location(0) position_size: vec4<f32>,        // x, y, width, height (CSS pixels)
    @location(1) color: vec4<f32>,                // rgba
    @location(2) border_radius_pad: vec4<f32>,    // border_radius (x), border_width (y), padding (zw)
    @location(3) inset_shadow_color: vec4<f32>,   // Inset shadow RGBA
    @location(4) inset_shadow_params: vec4<f32>,  // blur (x), offset (yz), has_shadow (w)
}}

struct VertexOutput {{
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_pos: vec2<f32>,            // Position within rectangle (0,0 to w,h)
    @location(2) size: vec2<f32>,                 // Rectangle size (w, h)
    @location(3) border_radius: f32,              // Border radius in pixels
    @location(4) border_width: f32,               // Border width for outline rendering (0 = filled)
    @location(5) inset_shadow_color: vec4<f32>,   // Inset shadow RGBA
    @location(6) inset_shadow_blur: f32,          // Inset shadow blur radius
    @location(7) inset_shadow_offset: vec2<f32>,  // Inset shadow offset (x, y)
    @location(8) has_inset_shadow: f32,           // Whether inset shadow is enabled
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
    let border_radius = instance.border_radius_pad.x;
    let border_width = instance.border_radius_pad.y;

    // Generate quad vertices (two triangles)
    // 0: top-left, 1: top-right, 2: bottom-left
    // 3: bottom-left, 4: top-right, 5: bottom-right
    var pos: vec2<f32>;
    var local: vec2<f32>;
    let idx = vertex.vertex_index % 6u;

    if (idx == 0u) {{
        pos = css_to_ndc(x, y);  // top-left
        local = vec2<f32>(0.0, 0.0);
    }} else if (idx == 1u || idx == 4u) {{
        pos = css_to_ndc(x + w, y);  // top-right
        local = vec2<f32>(w, 0.0);
    }} else if (idx == 2u || idx == 3u) {{
        pos = css_to_ndc(x, y + h);  // bottom-left
        local = vec2<f32>(0.0, h);
    }} else {{
        pos = css_to_ndc(x + w, y + h);  // bottom-right
        local = vec2<f32>(w, h);
    }}

    out.position = vec4<f32>(pos, 0.0, 1.0);
    out.color = instance.color;
    out.local_pos = local;
    out.size = vec2<f32>(w, h);
    out.border_radius = border_radius;
    out.border_width = border_width;

    // Pass inset shadow data
    out.inset_shadow_color = instance.inset_shadow_color;
    out.inset_shadow_blur = instance.inset_shadow_params.x;
    out.inset_shadow_offset = instance.inset_shadow_params.yz;
    out.has_inset_shadow = instance.inset_shadow_params.w;

    return out;
}}

// SDF for rounded rectangle
fn sd_rounded_box(p: vec2<f32>, size: vec2<f32>, radius: f32) -> f32 {{
    let q = abs(p) - size + radius;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - radius;
}}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {{
    // Calculate center-relative position
    let center = in.size * 0.5;
    let p = in.local_pos - center;

    var final_color: vec4<f32>;

    // Check if rendering outline (border ring) or filled rectangle
    if (in.border_width > 0.5) {{
        // Render outline/ring (for rounded borders)
        // Outer rounded box
        let outer_dist = sd_rounded_box(p, center, in.border_radius);

        // Inner rounded box (shrunk by border_width)
        let inner_size = center - vec2<f32>(in.border_width, in.border_width);
        let inner_radius = max(in.border_radius - in.border_width, 0.0);
        let inner_dist = sd_rounded_box(p, inner_size, inner_radius);

        // Ring = outside inner box AND inside outer box
        // dist is positive outside shape, negative inside
        let ring_dist = max(outer_dist, -inner_dist);

        // Anti-aliasing: smooth edge over 1 pixel
        let alpha = 1.0 - smoothstep(-0.5, 0.5, ring_dist);

        final_color = vec4<f32>(in.color.rgb, in.color.a * alpha);
    }} else {{
        // Render filled rectangle
        if (in.border_radius < 0.5) {{
            // No rounded corners, render solid
            final_color = in.color;
        }} else {{
            // Rounded corners with SDF
            let dist = sd_rounded_box(p, center, in.border_radius);

            // Anti-aliasing: smooth edge over 1 pixel
            let alpha = 1.0 - smoothstep(-0.5, 0.5, dist);

            // Apply alpha to color
            final_color = vec4<f32>(in.color.rgb, in.color.a * alpha);
        }}
    }}

    // Render inset shadow if enabled
    if (in.has_inset_shadow > 0.5) {{
        // SDF distance from edges (accounting for offset)
        let shadow_center = center + in.inset_shadow_offset;
        let shadow_p = in.local_pos - shadow_center;
        let shadow_dist = sd_rounded_box(shadow_p, center, in.border_radius);

        // Inset shadow: only inside rectangle (dist < 0)
        // Alpha is HIGH at edges (dist ≈ 0) and fades toward center (dist < -blur)
        if (shadow_dist < 0.0) {{
            // smoothstep(-blur, 0.0, dist) gives:
            // - 0.0 when dist <= -blur (center, no shadow)
            // - 1.0 when dist >= 0.0 (edges, full shadow)
            // - gradient in between
            let shadow_alpha = smoothstep(-in.inset_shadow_blur, 0.0, shadow_dist) * in.inset_shadow_color.a;

            // Blend shadow on top using "over" compositing
            let shadow_rgb = in.inset_shadow_color.rgb * shadow_alpha;
            let blended_rgb = final_color.rgb * (1.0 - shadow_alpha) + shadow_rgb;
            final_color = vec4<f32>(blended_rgb, final_color.a);
        }}
    }}

    return final_color;
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
                cull_mode: None, // No culling for 2D rectangles
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
                        load: wgpu::LoadOp::Load, // Load existing content (shadows already rendered)
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
