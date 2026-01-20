#[cfg(feature = "windowed")]
pub mod windowed {
    use crate::constants::{HEIGHT, WIDTH};
    use crate::shader_bindings::rectangle;
    use anyhow::{Context, Result};
    use std::sync::Arc;
    use wgpu::util::DeviceExt;
    use winit::{
        application::ApplicationHandler,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        window::{Window, WindowId},
    };

    struct WindowedRenderer {
        window: Arc<Window>,
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        config: wgpu::SurfaceConfiguration,
        pipeline: wgpu::RenderPipeline,
        vertex_buffer: wgpu::Buffer,
        index_buffer: wgpu::Buffer,
        num_indices: u32,
    }

    impl WindowedRenderer {
        fn new(window: Arc<Window>) -> Result<Self> {
            // Create wgpu instance
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });

            // Create surface
            let surface = instance.create_surface(window.clone())?;

            // Request adapter
            let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            }))
            .context("Failed to find a suitable GPU adapter")?;

            // Create device and queue
            let (device, queue) = pollster::block_on(adapter.request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("RayBox Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                    trace: wgpu::Trace::Off,
                },
            ))
            .context("Failed to create device")?;

            // Configure surface
            let surface_caps = surface.get_capabilities(&adapter);
            let surface_format = surface_caps
                .formats
                .iter()
                .find(|f| f.is_srgb())
                .copied()
                .unwrap_or(surface_caps.formats[0]);

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: WIDTH,
                height: HEIGHT,
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode: surface_caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);

            // Create shader module
            let shader_module = rectangle::create_shader_module_embed_source(&device);

            // Create pipeline layout
            let pipeline_layout = rectangle::create_pipeline_layout(&device);

            // Create render pipeline
            let vertex_entry = rectangle::vs_main_entry(wgpu::VertexStepMode::Vertex);
            let fragment_entry = rectangle::fs_main_entry([Some(wgpu::ColorTargetState {
                format: surface_format,
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

            // Create rectangle vertices
            let vertices: [rectangle::vertexInput_0; 4] = [
                rectangle::vertexInput_0::new([-0.5, 0.5], [1.0, 0.0, 0.0, 1.0]),
                rectangle::vertexInput_0::new([0.5, 0.5], [0.0, 1.0, 0.0, 1.0]),
                rectangle::vertexInput_0::new([0.5, -0.5], [0.0, 0.0, 1.0, 1.0]),
                rectangle::vertexInput_0::new([-0.5, -0.5], [1.0, 1.0, 0.0, 1.0]),
            ];

            let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

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
                window,
                surface,
                device,
                queue,
                config,
                pipeline,
                vertex_buffer,
                index_buffer,
                num_indices: indices.len() as u32,
            })
        }

        fn render(&self) -> Result<(), wgpu::SurfaceError> {
            let output = self.surface.get_current_texture()?;
            let view = output
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

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

            self.queue.submit(std::iter::once(encoder.finish()));
            output.present();

            Ok(())
        }

        fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
            if new_size.width > 0 && new_size.height > 0 {
                self.config.width = new_size.width;
                self.config.height = new_size.height;
                self.surface.configure(&self.device, &self.config);
            }
        }
    }

    struct App {
        renderer: Option<WindowedRenderer>,
    }

    impl ApplicationHandler for App {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.renderer.is_none() {
                let window_attrs = Window::default_attributes()
                    .with_title("RayBox - Rectangle Renderer")
                    .with_inner_size(winit::dpi::PhysicalSize::new(WIDTH, HEIGHT));

                let window = Arc::new(event_loop.create_window(window_attrs).unwrap());

                match WindowedRenderer::new(window) {
                    Ok(renderer) => {
                        self.renderer = Some(renderer);
                    }
                    Err(e) => {
                        eprintln!("Failed to create renderer: {}", e);
                        event_loop.exit();
                    }
                }
            }
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _window_id: WindowId,
            event: WindowEvent,
        ) {
            let Some(renderer) = self.renderer.as_mut() else {
                return;
            };

            match event {
                WindowEvent::CloseRequested => {
                    event_loop.exit();
                }
                WindowEvent::Resized(physical_size) => {
                    renderer.resize(physical_size);
                }
                WindowEvent::RedrawRequested => {
                    match renderer.render() {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => {
                            renderer.resize(winit::dpi::PhysicalSize::new(
                                renderer.config.width,
                                renderer.config.height,
                            ));
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            event_loop.exit();
                        }
                        Err(e) => eprintln!("Render error: {:?}", e),
                    }
                    renderer.window.request_redraw();
                }
                _ => {}
            }
        }
    }

    pub fn run() -> Result<()> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut app = App { renderer: None };
        event_loop.run_app(&mut app)?;

        Ok(())
    }
}
