//! Demo 2: Animated Spheres Scene
//! Grid of bouncing colorful spheres
//!
//! Run with: cargo run --example demo_spheres --features windowed

#[cfg(not(feature = "windowed"))]
fn main() {
    eprintln!("This example requires the 'windowed' feature.");
    eprintln!("Run with: cargo run --example demo_spheres --features windowed");
}

#[cfg(feature = "windowed")]
fn main() -> anyhow::Result<()> {
    run_windowed()
}

#[cfg(feature = "windowed")]
#[path = "../src/camera.rs"]
mod camera;
#[cfg(feature = "windowed")]
#[path = "../src/constants.rs"]
mod constants;
#[cfg(feature = "windowed")]
#[path = "../src/input.rs"]
mod input;

#[cfg(feature = "windowed")]
#[allow(unused, non_snake_case, non_camel_case_types, non_upper_case_globals)]
mod shader_bindings {
    include!(concat!(env!("OUT_DIR"), "/shader_bindings.rs"));
}

#[cfg(feature = "windowed")]
const DEMO_TITLE: &str = "Demo 2: Spheres";

#[cfg(feature = "windowed")]
fn run_windowed() -> anyhow::Result<()> {
    use camera::FlyCamera;
    use constants::{HEIGHT, WIDTH};
    use input::{CameraConfig, InputAction, InputHandler};
    use shader_bindings::sdf_spheres;
    use std::sync::Arc;

    use anyhow::Context;
    use bytemuck::{Pod, Zeroable};
    use wgpu::util::DeviceExt;
    use winit::{
        application::ApplicationHandler,
        event::{DeviceEvent, WindowEvent},
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        keyboard::PhysicalKey,
        window::{Window, WindowId},
    };

    #[repr(C)]
    #[derive(Copy, Clone, Debug, Pod, Zeroable)]
    struct Uniforms {
        inv_view_proj: [[f32; 4]; 4],
        camera_pos_time: [f32; 4],
        light_dir_intensity: [f32; 4],
        render_params: [f32; 4],
    }

    impl Default for Uniforms {
        fn default() -> Self {
            Self {
                inv_view_proj: [[0.0; 4]; 4],
                camera_pos_time: [0.0, 2.0, 8.0, 0.0],
                light_dir_intensity: [0.577, 0.577, 0.577, 1.0],
                render_params: [WIDTH as f32, HEIGHT as f32, 0.5, 16.0],
            }
        }
    }

    impl Uniforms {
        fn update_from_camera(&mut self, camera: &FlyCamera, width: u32, height: u32, time: f32) {
            let aspect = width as f32 / height as f32;
            self.inv_view_proj = camera.inv_view_projection_matrix(aspect).to_cols_array_2d();
            self.camera_pos_time = [
                camera.position().x,
                camera.position().y,
                camera.position().z,
                time,
            ];
            self.render_params[0] = width as f32;
            self.render_params[1] = height as f32;
        }
    }

    struct Renderer {
        window: Arc<Window>,
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        config: wgpu::SurfaceConfiguration,
        pipeline: wgpu::RenderPipeline,
        uniform_buffer: wgpu::Buffer,
        uniform_bind_group: wgpu::BindGroup,
        camera: FlyCamera,
        input: InputHandler,
        start_time: std::time::Instant,
        last_frame_time: std::time::Instant,
    }

    impl Renderer {
        fn new(window: Arc<Window>) -> anyhow::Result<Self> {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });

            let surface = instance.create_surface(window.clone())?;

            let adapter =
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                }))
                .context("Failed to find a suitable GPU adapter")?;

            let (device, queue) =
                pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                    label: Some("RayBox Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                    trace: wgpu::Trace::Off,
                }))
                .context("Failed to create device")?;

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

            let shader_module = sdf_spheres::create_shader_module_embed_source(&device);

            // Camera config: positioned higher and further back for this scene
            let camera_config = CameraConfig {
                initial_position: glam::Vec3::new(0.0, 2.0, 8.0),
                look_at_target: glam::Vec3::new(0.0, 0.5, 0.0),
            };
            let input = InputHandler::new(camera_config);

            let mut camera = FlyCamera::default();
            input.setup_camera(&mut camera);

            let mut uniforms = Uniforms::default();
            uniforms.update_from_camera(&camera, config.width, config.height, 0.0);

            let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Uniform Buffer"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

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

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("SDF Pipeline"),
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
                primitive: wgpu::PrimitiveState::default(),
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
                input,
                start_time: std::time::Instant::now(),
                last_frame_time: std::time::Instant::now(),
            })
        }

        fn update(&mut self) {
            let now = std::time::Instant::now();
            let dt = (now - self.last_frame_time).as_secs_f32();
            self.last_frame_time = now;

            self.input.update_frame_time(dt);
            self.input.update_camera(&mut self.camera, dt);
            self.input
                .update_window_title(&self.window, DEMO_TITLE, &self.camera);
        }

        fn render(&self) -> Result<(), wgpu::SurfaceError> {
            let time = self.start_time.elapsed().as_secs_f32();
            let mut uniforms = Uniforms::default();
            uniforms.update_from_camera(&self.camera, self.config.width, self.config.height, time);
            self.queue
                .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

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
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
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
    }

    struct App {
        renderer: Option<Renderer>,
    }

    impl ApplicationHandler for App {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.renderer.is_none() {
                let window_attrs = Window::default_attributes()
                    .with_title(input::demo_title(2, "Spheres"))
                    .with_inner_size(winit::dpi::PhysicalSize::new(WIDTH, HEIGHT));

                let window = Arc::new(event_loop.create_window(window_attrs).unwrap());

                match Renderer::new(window) {
                    Ok(renderer) => self.renderer = Some(renderer),
                    Err(e) => {
                        eprintln!("Failed to create renderer: {}", e);
                        event_loop.exit();
                    }
                }
            }
        }

        fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
            self.renderer.take();
        }

        fn device_event(
            &mut self,
            _event_loop: &ActiveEventLoop,
            _device_id: winit::event::DeviceId,
            event: DeviceEvent,
        ) {
            if let Some(renderer) = self.renderer.as_mut() {
                if let DeviceEvent::MouseMotion { delta } = event {
                    renderer
                        .input
                        .handle_mouse_motion(&mut renderer.camera, delta);
                }
            }
        }

        fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
            let Some(renderer) = self.renderer.as_mut() else {
                return;
            };

            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::KeyboardInput { event, .. } => {
                    if let PhysicalKey::Code(_) = event.physical_key {
                        if let Some(action) = renderer.input.handle_key(event) {
                            match action {
                                InputAction::Exit => event_loop.exit(),
                                InputAction::ToggleCapture => {
                                    renderer.input.toggle_capture(&renderer.window);
                                }
                                InputAction::ToggleOverlayApp => {
                                    renderer.input.toggle_overlay_app();
                                }
                                InputAction::ToggleOverlayFull => {
                                    renderer.input.toggle_overlay_full();
                                }
                                InputAction::ResetRoll => {
                                    renderer.input.reset_roll(&mut renderer.camera);
                                }
                                InputAction::ResetCamera => {
                                    renderer.input.reset_camera(&mut renderer.camera);
                                }
                                InputAction::ToggleKeybindings => {
                                    // Not used in standalone examples
                                }
                            }
                        }
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    renderer.input.handle_scroll(&mut renderer.camera, delta);
                }
                WindowEvent::MouseInput {
                    state: winit::event::ElementState::Pressed,
                    button: winit::event::MouseButton::Left,
                    ..
                } => {
                    renderer.input.capture(&renderer.window);
                }
                WindowEvent::Resized(size) => renderer.resize(size),
                WindowEvent::RedrawRequested => {
                    renderer.update();
                    if let Err(e) = renderer.render() {
                        eprintln!("Render error: {:?}", e);
                    }
                    renderer.window.request_redraw();
                }
                _ => {}
            }
        }
    }

    env_logger::init();
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App { renderer: None };
    event_loop.run_app(&mut app)?;
    Ok(())
}
