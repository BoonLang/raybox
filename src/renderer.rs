use crate::constants::{HEIGHT, TEXTURE_FORMAT, WIDTH};
use crate::shader_bindings::rectangle;
use anyhow::{Context, Result};
use wgpu::util::DeviceExt;

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
}

impl Renderer {
    pub async fn new() -> Result<Self> {
        // Initialize wgpu instance
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // Request adapter (no surface needed for headless)
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .context("Failed to find a suitable GPU adapter")?;

        // Create device and queue
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("RayBox Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .context("Failed to create device")?;

        // Create shader module using generated bindings
        let shader_module = rectangle::create_shader_module_embed_source(&device);

        // Create pipeline layout
        let pipeline_layout = rectangle::create_pipeline_layout(&device);

        // Create render pipeline
        let vertex_entry = rectangle::vs_main_entry(wgpu::VertexStepMode::Vertex);
        let fragment_entry = rectangle::fs_main_entry([Some(wgpu::ColorTargetState {
            format: TEXTURE_FORMAT,
            blend: Some(wgpu::BlendState::REPLACE),
            write_mask: wgpu::ColorWrites::ALL,
        })]);

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Rectangle Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: rectangle::vertex_state(&shader_module, &vertex_entry),
            fragment: Some(rectangle::fragment_state(&shader_module, &fragment_entry)),
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

        // Create rectangle vertices (centered, 0.5 x 0.5 in clip space)
        // Using vertexInput_0 struct from generated bindings
        let vertices: [rectangle::vertexInput_0; 4] = [
            // Top-left (red)
            rectangle::vertexInput_0::new([-0.5, 0.5], [1.0, 0.0, 0.0, 1.0]),
            // Top-right (green)
            rectangle::vertexInput_0::new([0.5, 0.5], [0.0, 1.0, 0.0, 1.0]),
            // Bottom-right (blue)
            rectangle::vertexInput_0::new([0.5, -0.5], [0.0, 0.0, 1.0, 1.0]),
            // Bottom-left (yellow)
            rectangle::vertexInput_0::new([-0.5, -0.5], [1.0, 1.0, 0.0, 1.0]),
        ];

        let indices: [u16; 6] = [
            0, 1, 2, // First triangle
            0, 2, 3, // Second triangle
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Ok(Self {
            device,
            queue,
            pipeline,
            vertex_buffer,
            index_buffer,
            num_indices: indices.len() as u32,
        })
    }

    /// Render to an offscreen texture and return it
    pub fn render(&self) -> wgpu::Texture {
        // Create render target texture
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Render Target"),
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TEXTURE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create command encoder
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Begin render pass
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
        }

        // Submit commands
        self.queue.submit(std::iter::once(encoder.finish()));

        texture
    }
}
