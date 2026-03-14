#[cfg(feature = "windowed")]
pub mod windowed {
    use crate::camera::{OrbitalCamera, Uniforms};
    use crate::constants::{HEIGHT, WIDTH};
    use crate::shader_bindings::sdf_raymarch;
    use anyhow::{Context, Result};
    use std::collections::HashSet;
    use std::sync::Arc;
    use wgpu::util::DeviceExt;
    use winit::{
        application::ApplicationHandler,
        event::{ElementState, KeyEvent, WindowEvent},
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        keyboard::{KeyCode, PhysicalKey},
        window::{Window, WindowId},
    };

    struct WindowedRenderer {
        window: Arc<Window>,
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        config: wgpu::SurfaceConfiguration,
        pipeline: wgpu::RenderPipeline,
        uniform_buffer: wgpu::Buffer,
        uniform_bind_group: wgpu::BindGroup,
        camera: OrbitalCamera,
        pressed_keys: HashSet<KeyCode>,
        start_time: std::time::Instant,
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
            let adapter =
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                }))
                .context("Failed to find a suitable GPU adapter")?;

            // Create device and queue
            let (device, queue) =
                pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                    label: Some("RayBox SDF Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                    trace: wgpu::Trace::Off,
                }))
                .context("Failed to create device")?;

            // Configure surface
            let surface_caps = surface.get_capabilities(&adapter);
            let surface_format = surface_caps
                .formats
                .iter()
                .find(|f| f.is_srgb())
                .copied()
                .unwrap_or(surface_caps.formats[0]);
            let alpha_mode = surface_caps
                .alpha_modes
                .iter()
                .copied()
                .find(|mode| *mode == wgpu::CompositeAlphaMode::Opaque)
                .unwrap_or(surface_caps.alpha_modes[0]);

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: WIDTH,
                height: HEIGHT,
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode,
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);

            // Create shader module
            let shader_module = sdf_raymarch::create_shader_module_embed_source(&device);

            // Create uniform buffer
            let camera = OrbitalCamera::default();
            let mut uniforms = Uniforms::default();
            uniforms.update_from_camera(&camera, config.width, config.height, 0.0);

            let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Uniform Buffer"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

            // Create bind group layout
            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Uniform Bind Group Layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

            let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Uniform Bind Group"),
                layout: &bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                }],
            });

            // Create pipeline layout
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("SDF Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            // Create render pipeline
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("SDF Raymarch Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader_module,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader_module,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
                        blend: Some(wgpu::BlendState::REPLACE),
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

            Ok(Self {
                window,
                surface,
                device,
                queue,
                config,
                pipeline,
                uniform_buffer,
                uniform_bind_group,
                camera,
                pressed_keys: HashSet::new(),
                start_time: std::time::Instant::now(),
            })
        }

        fn update(&mut self) {
            const ROTATION_SPEED: f32 = 0.03;
            const ZOOM_SPEED: f32 = 0.1;

            // A/D for horizontal rotation
            if self.pressed_keys.contains(&KeyCode::KeyA) {
                self.camera.rotate_horizontal(-ROTATION_SPEED);
            }
            if self.pressed_keys.contains(&KeyCode::KeyD) {
                self.camera.rotate_horizontal(ROTATION_SPEED);
            }

            // W/S for zoom
            if self.pressed_keys.contains(&KeyCode::KeyW) {
                self.camera.zoom(ZOOM_SPEED);
            }
            if self.pressed_keys.contains(&KeyCode::KeyS) {
                self.camera.zoom(-ZOOM_SPEED);
            }

            // Q/E for vertical rotation
            if self.pressed_keys.contains(&KeyCode::KeyQ) {
                self.camera.rotate_vertical(ROTATION_SPEED);
            }
            if self.pressed_keys.contains(&KeyCode::KeyE) {
                self.camera.rotate_vertical(-ROTATION_SPEED);
            }
        }

        fn update_uniforms(&self) {
            let time = self.start_time.elapsed().as_secs_f32();
            let mut uniforms = Uniforms::default();
            uniforms.update_from_camera(&self.camera, self.config.width, self.config.height, time);
            self.queue
                .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
        }

        fn render(&self) -> Result<(), wgpu::SurfaceError> {
            self.update_uniforms();

            let output = self.surface.get_current_texture()?;
            let view = output
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("SDF Render Encoder"),
                });

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("SDF Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
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
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                render_pass.draw(0..3, 0..1);
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

        fn handle_key(&mut self, event: KeyEvent) {
            if let PhysicalKey::Code(key_code) = event.physical_key {
                match event.state {
                    ElementState::Pressed => {
                        self.pressed_keys.insert(key_code);
                    }
                    ElementState::Released => {
                        self.pressed_keys.remove(&key_code);
                    }
                }
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
                    .with_title("RayBox - SDF Renderer (A/D: rotate, W/S: zoom, Q/E: tilt)")
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
                WindowEvent::KeyboardInput { event, .. } => {
                    // Escape to close
                    if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
                        if event.state == ElementState::Pressed {
                            event_loop.exit();
                            return;
                        }
                    }
                    renderer.handle_key(event);
                }
                WindowEvent::Resized(physical_size) => {
                    renderer.resize(physical_size);
                }
                WindowEvent::RedrawRequested => {
                    renderer.update();

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
