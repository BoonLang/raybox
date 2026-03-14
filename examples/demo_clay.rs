//! Demo 5: 3D Clay Tablet with Vector SDF Relief Text
//! Text carved into a clay slab using exact Bézier SDF
//!
//! Run with: cargo run --example demo_clay --features windowed
//! Screenshot: cargo run --example demo_clay

#[path = "../src/camera.rs"]
mod camera;
#[path = "../src/constants.rs"]
mod constants;
#[cfg(feature = "windowed")]
mod demo_core {
    pub use raybox::demo_core::*;
}
#[cfg(feature = "windowed")]
#[path = "../src/input.rs"]
mod input;
#[path = "../src/text/mod.rs"]
mod text;

#[allow(unused, non_snake_case, non_camel_case_types, non_upper_case_globals)]
mod shader_bindings {
    include!(concat!(env!("OUT_DIR"), "/shader_bindings.rs"));
}

use camera::FlyCamera;
use constants::{HEIGHT, WIDTH};
#[cfg(feature = "windowed")]
use input::{CameraConfig, InputAction, InputHandler};
use text::{build_char_grid, VectorFont, VectorFontAtlas};

use anyhow::{Context, Result};
use wgpu::util::DeviceExt;

// ~175 words per paragraph
const LOREM: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum. Curabitur pretium tincidunt lacus. Nulla gravida orci a odio. Nullam varius, turpis et commodo pharetra, est eros bibendum elit, nec luctus magna felis sollicitudin mauris. Integer in mauris eu nibh euismod gravida. Duis ac tellus et risus vulputate vehicula. Donec lobortis risus a elit. Etiam tempor. Ut ullamcorper, ligula eu tempor congue, eros est euismod turpis, id tincidunt sapien risus a quam. Maecenas fermentum consequat mi. Donec fermentum. Pellentesque malesuada nulla a mi. Duis sapien sem, aliquet sed, vulputate eget, feugiat non, orci. Sed neque. Sed eget lacus. Mauris non dui nec urna suscipit nonummy. Fusce fermentum fermentum arcu. Vestibulum ante ipsum primis in faucibus orci luctus et ultrices posuere cubilia curae.";
const TEXT_GRID_DIMS: [u32; 2] = [80, 48];

type Uniforms = shader_bindings::sdf_clay_vector::Uniforms_std140_0;
type GpuCharGridCell = shader_bindings::sdf_clay_vector::CharGridCellData_std430_0;
type GpuBezierCurve = shader_bindings::sdf_clay_vector::BezierCurve_std430_0;
type GpuGlyphData = shader_bindings::sdf_clay_vector::GlyphData_std430_0;
type GpuCharInstance = shader_bindings::sdf_clay_vector::CharInstance_std430_0;

const EMPTY_CHAR_GRID_CELLS: [GpuCharGridCell; 1] = [GpuCharGridCell::new(0, 0)];
const EMPTY_CURVES: [GpuBezierCurve; 1] = [GpuBezierCurve::new([0.0; 4], [0.0; 4], [0.0; 4])];
const EMPTY_GLYPH_DATA: [GpuGlyphData; 1] = [GpuGlyphData::new([0.0; 4], [0; 4], [0; 4])];
const EMPTY_CHAR_INSTANCES: [GpuCharInstance; 1] = [GpuCharInstance::new([0.0; 4])];
const EMPTY_U32: [u32; 1] = [0];

fn build_uniforms(
    camera: &FlyCamera,
    width: u32,
    height: u32,
    time: f32,
    char_count: u32,
    char_grid_params: [f32; 4],
    char_grid_bounds: [f32; 4],
) -> Uniforms {
    let aspect = width as f32 / height as f32;
    Uniforms::new(
        shader_bindings::sdf_clay_vector::_MatrixStorage_float4x4_ColMajorstd140_0::new(
            camera.inv_view_projection_matrix(aspect).to_cols_array_2d(),
        ),
        [
            camera.position().x,
            camera.position().y,
            camera.position().z,
            time,
        ],
        [0.5, 0.8, 0.3, 1.5],
        [width as f32, height as f32, 0.2, 1.0],
        [char_count as f32, 0.0, 0.4, 0.0],
        char_grid_params,
        char_grid_bounds,
    )
}

/// Build text layout for clay tablet (horizontal layout on XZ plane)
fn build_clay_text_layout(
    atlas: &VectorFontAtlas,
    tablet_width: f32,
    tablet_depth: f32,
) -> Vec<GpuCharInstance> {
    let mut instances = Vec::new();

    // Generate 1000+ words by repeating lorem ipsum
    let full_text = format!(
        "RAYBOX SDF TEXT ENGINE\n\n{}",
        format!(
            "{} {} {} {} {} {}",
            LOREM, LOREM, LOREM, LOREM, LOREM, LOREM
        )
    );

    let scale = 0.15; // Character scale
    let line_height = 0.22;
    let margin = 0.15;

    let start_x = -tablet_width + margin;
    let start_z = tablet_depth - margin; // Start at top (far from camera)
    let max_x = tablet_width - margin;

    let mut x = start_x;
    let mut z = start_z;
    let mut line_num = 0;
    let max_lines = 16; // Limit lines for performance

    for ch in full_text.chars() {
        if line_num >= max_lines {
            break;
        }

        if ch == '\n' {
            x = start_x;
            z -= line_height; // Move toward camera (down the tablet)
            line_num += 1;
            continue;
        }

        let codepoint = ch as u32;
        if let Some(entry) = atlas.glyphs.get(&codepoint) {
            let glyph_idx = atlas
                .glyph_list
                .iter()
                .position(|(cp, _)| *cp == codepoint)
                .unwrap_or(0) as u32;

            let advance = entry.advance * scale;

            // Word wrap
            if x + advance > max_x {
                x = start_x;
                z -= line_height;
                line_num += 1;
                if line_num >= max_lines {
                    break;
                }
            }

            instances.push(GpuCharInstance::new([x, z, scale, glyph_idx as f32]));

            x += advance;
        } else if ch == ' ' {
            x += 0.08 * scale;
            if x > max_x {
                x = start_x;
                z -= line_height;
                line_num += 1;
            }
        }
    }

    instances
}

fn main() -> Result<()> {
    env_logger::init();

    #[cfg(feature = "windowed")]
    {
        run_windowed()
    }

    #[cfg(not(feature = "windowed"))]
    {
        run_headless_screenshot()
    }
}

// ============================================================================
// Windowed mode (requires --features windowed)
// ============================================================================

#[cfg(feature = "windowed")]
const DEMO_TITLE: &str = "Demo 5: Clay Tablet";

#[cfg(feature = "windowed")]
fn run_windowed() -> Result<()> {
    use std::sync::Arc;
    use winit::{
        application::ApplicationHandler,
        event::{DeviceEvent, WindowEvent},
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        keyboard::PhysicalKey,
        window::{Window, WindowId},
    };

    struct Renderer {
        window: Arc<Window>,
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        config: wgpu::SurfaceConfiguration,
        pipeline: wgpu::RenderPipeline,
        uniform_buffer: wgpu::Buffer,
        bind_group: wgpu::BindGroup,
        camera: FlyCamera,
        input: InputHandler,
        start_time: std::time::Instant,
        last_frame_time: std::time::Instant,
        char_count: u32,
        char_grid_params: [f32; 4],
        char_grid_bounds: [f32; 4],
    }

    impl Renderer {
        fn new(window: Arc<Window>) -> Result<Self> {
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

            // Load vector font
            let font_data =
                std::fs::read("assets/fonts/DejaVuSans.ttf").context("Failed to load font file")?;
            let font = VectorFont::from_ttf(&font_data).map_err(|e| anyhow::anyhow!(e))?;
            let atlas = VectorFontAtlas::from_font(&font, 8);

            // Build text layout
            let char_instances = build_clay_text_layout(&atlas, 2.3, 1.6);
            let char_count = char_instances.len() as u32;
            let instance_data: Vec<[f32; 4]> =
                char_instances.iter().map(|c| c.posAndChar_0).collect();
            let char_grid = build_char_grid(&instance_data, &atlas, TEXT_GRID_DIMS);
            let char_grid_params = [
                char_grid.dims[0] as f32,
                char_grid.dims[1] as f32,
                char_grid.cell_size[0],
                char_grid.cell_size[1],
            ];
            let char_grid_bounds = char_grid.bounds;
            let gpu_char_grid_cells: Vec<GpuCharGridCell> = char_grid
                .cells
                .iter()
                .map(|c| GpuCharGridCell::new(c.offset, c.count))
                .collect();

            // Prepare GPU data
            let gpu_curves: Vec<GpuBezierCurve> = atlas
                .curves
                .iter()
                .map(|c| {
                    let p0 = c.p0();
                    let p1 = c.p1();
                    let p2 = c.p2();
                    GpuBezierCurve::new(
                        [p0.0, p0.1, p1.0, p1.1],
                        [p2.0, p2.1, c.bbox[0], c.bbox[1]],
                        [c.bbox[2], c.bbox[3], c.flags as f32, 0.0],
                    )
                })
                .collect();

            let gpu_glyph_data: Vec<GpuGlyphData> = atlas
                .glyph_list
                .iter()
                .map(|(_, entry)| {
                    GpuGlyphData::new(
                        entry.bounds,
                        [entry.grid_offset, entry.grid_size[0], entry.grid_size[1], 0],
                        [entry.curve_offset, entry.curve_count, 0, 0],
                    )
                })
                .collect();

            // Camera config: positioned above, looking down at tablet
            let camera_config = CameraConfig {
                initial_position: glam::Vec3::new(0.0, 4.5, 1.5),
                look_at_target: glam::Vec3::new(0.0, 0.0, 0.0),
            };
            let input = InputHandler::new(camera_config);

            let mut camera = FlyCamera::default();
            input.setup_camera(&mut camera);

            let uniforms = build_uniforms(
                &camera,
                WIDTH,
                HEIGHT,
                0.0,
                char_count,
                char_grid_params,
                char_grid_bounds,
            );

            let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Uniform Buffer"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

            let curves_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Curves Buffer"),
                contents: bytemuck::cast_slice(if gpu_curves.is_empty() {
                    &EMPTY_CURVES
                } else {
                    &gpu_curves
                }),
                usage: wgpu::BufferUsages::STORAGE,
            });

            let glyph_data_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Glyph Data Buffer"),
                contents: bytemuck::cast_slice(if gpu_glyph_data.is_empty() {
                    &EMPTY_GLYPH_DATA
                } else {
                    &gpu_glyph_data
                }),
                usage: wgpu::BufferUsages::STORAGE,
            });

            let char_instances_buffer =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Char Instances Buffer"),
                    contents: bytemuck::cast_slice(if char_instances.is_empty() {
                        &EMPTY_CHAR_INSTANCES
                    } else {
                        &char_instances
                    }),
                    usage: wgpu::BufferUsages::STORAGE,
                });

            let char_grid_cells_buffer =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Char Grid Cells Buffer"),
                    contents: bytemuck::cast_slice(if gpu_char_grid_cells.is_empty() {
                        &EMPTY_CHAR_GRID_CELLS
                    } else {
                        &gpu_char_grid_cells
                    }),
                    usage: wgpu::BufferUsages::STORAGE,
                });

            let char_grid_indices_buffer =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Char Grid Indices Buffer"),
                    contents: bytemuck::cast_slice(if char_grid.char_indices.is_empty() {
                        &EMPTY_U32
                    } else {
                        &char_grid.char_indices
                    }),
                    usage: wgpu::BufferUsages::STORAGE,
                });

            let bind_group_layout = device.create_bind_group_layout(
                &shader_bindings::sdf_clay_vector::WgpuBindGroup0::LAYOUT_DESCRIPTOR,
            );
            let bind_group_entries = shader_bindings::sdf_clay_vector::WgpuBindGroup0Entries::new(
                shader_bindings::sdf_clay_vector::WgpuBindGroup0EntriesParams {
                    uniforms_0: wgpu::BufferBinding {
                        buffer: &uniform_buffer,
                        offset: 0,
                        size: None,
                    },
                    charGridCells_0: wgpu::BufferBinding {
                        buffer: &char_grid_cells_buffer,
                        offset: 0,
                        size: None,
                    },
                    charGridIndices_0: wgpu::BufferBinding {
                        buffer: &char_grid_indices_buffer,
                        offset: 0,
                        size: None,
                    },
                    charInstances_0: wgpu::BufferBinding {
                        buffer: &char_instances_buffer,
                        offset: 0,
                        size: None,
                    },
                    glyphData_0: wgpu::BufferBinding {
                        buffer: &glyph_data_buffer,
                        offset: 0,
                        size: None,
                    },
                    curves_0: wgpu::BufferBinding {
                        buffer: &curves_buffer,
                        offset: 0,
                        size: None,
                    },
                },
            );

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Bind Group"),
                layout: &bind_group_layout,
                entries: &bind_group_entries.as_array(),
            });

            // Create pipeline
            let shader_module =
                shader_bindings::sdf_clay_vector::create_shader_module_embed_source(&device);

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Clay Tablet Pipeline"),
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
                bind_group,
                camera,
                input,
                start_time: std::time::Instant::now(),
                last_frame_time: std::time::Instant::now(),
                char_count,
                char_grid_params,
                char_grid_bounds,
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
            let uniforms = build_uniforms(
                &self.camera,
                self.config.width,
                self.config.height,
                time,
                self.char_count,
                self.char_grid_params,
                self.char_grid_bounds,
            );
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
                render_pass.set_bind_group(0, &self.bind_group, &[]);
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
                    .with_title(input::demo_title(5, "Clay Tablet"))
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

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App { renderer: None };
    event_loop.run_app(&mut app)?;
    Ok(())
}

// ============================================================================
// Headless screenshot mode (default, no windowed feature)
// ============================================================================

#[cfg(not(feature = "windowed"))]
fn run_headless_screenshot() -> Result<()> {
    println!("Capturing headless screenshot of Clay Tablet Vector SDF...");

    // Load vector font
    let font_data =
        std::fs::read("assets/fonts/DejaVuSans.ttf").context("Failed to load font file")?;
    let font = VectorFont::from_ttf(&font_data).map_err(|e| anyhow::anyhow!(e))?;
    let atlas = VectorFontAtlas::from_font(&font, 8);

    // Build text layout
    let char_instances = build_clay_text_layout(&atlas, 2.3, 1.6);
    let char_count = char_instances.len() as u32;

    println!("Text has {} characters", char_count);
    println!(
        "Atlas has {} glyphs, {} curves, {} grid cells",
        atlas.glyph_list.len(),
        atlas.curves.len(),
        atlas.grid_cells.len()
    );

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .context("Failed to find adapter")?;

    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("Headless Device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
    }))
    .context("Failed to create device")?;

    let width = WIDTH;
    let height = HEIGHT;
    let texture_format = wgpu::TextureFormat::Rgba8UnormSrgb;

    // Create render texture
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Render Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: texture_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    // Prepare GPU data
    let instance_data: Vec<[f32; 4]> = char_instances.iter().map(|c| c.posAndChar_0).collect();
    let char_grid = build_char_grid(&instance_data, &atlas, TEXT_GRID_DIMS);
    let char_grid_params = [
        char_grid.dims[0] as f32,
        char_grid.dims[1] as f32,
        char_grid.cell_size[0],
        char_grid.cell_size[1],
    ];
    let char_grid_bounds = char_grid.bounds;
    let gpu_char_grid_cells: Vec<GpuCharGridCell> = char_grid
        .cells
        .iter()
        .map(|c| GpuCharGridCell::new(c.offset, c.count))
        .collect();

    let gpu_curves: Vec<GpuBezierCurve> = atlas
        .curves
        .iter()
        .map(|c| {
            let p0 = c.p0();
            let p1 = c.p1();
            let p2 = c.p2();
            GpuBezierCurve::new(
                [p0.0, p0.1, p1.0, p1.1],
                [p2.0, p2.1, c.bbox[0], c.bbox[1]],
                [c.bbox[2], c.bbox[3], c.flags as f32, 0.0],
            )
        })
        .collect();

    let gpu_glyph_data: Vec<GpuGlyphData> = atlas
        .glyph_list
        .iter()
        .map(|(_, entry)| {
            GpuGlyphData::new(
                entry.bounds,
                [entry.grid_offset, entry.grid_size[0], entry.grid_size[1], 0],
                [entry.curve_offset, entry.curve_count, 0, 0],
            )
        })
        .collect();

    // Create buffers - fly camera positioned above, looking down at tablet
    let mut camera = FlyCamera::default();
    camera.position = glam::Vec3::new(0.0, 4.5, 1.5);
    camera.pitch = -1.2; // Looking down at the tablet

    let uniforms = build_uniforms(
        &camera,
        width,
        height,
        0.0,
        char_count,
        char_grid_params,
        char_grid_bounds,
    );

    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Uniform Buffer"),
        contents: bytemuck::cast_slice(&[uniforms]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let curves_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Curves Buffer"),
        contents: bytemuck::cast_slice(if gpu_curves.is_empty() {
            &EMPTY_CURVES
        } else {
            &gpu_curves
        }),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let glyph_data_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Glyph Data Buffer"),
        contents: bytemuck::cast_slice(if gpu_glyph_data.is_empty() {
            &EMPTY_GLYPH_DATA
        } else {
            &gpu_glyph_data
        }),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let char_instances_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Char Instances Buffer"),
        contents: bytemuck::cast_slice(if char_instances.is_empty() {
            &EMPTY_CHAR_INSTANCES
        } else {
            &char_instances
        }),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let char_grid_cells_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Char Grid Cells Buffer"),
        contents: bytemuck::cast_slice(if gpu_char_grid_cells.is_empty() {
            &EMPTY_CHAR_GRID_CELLS
        } else {
            &gpu_char_grid_cells
        }),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let char_grid_indices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Char Grid Indices Buffer"),
        contents: bytemuck::cast_slice(if char_grid.char_indices.is_empty() {
            &EMPTY_U32
        } else {
            &char_grid.char_indices
        }),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let bind_group_layout = device.create_bind_group_layout(
        &shader_bindings::sdf_clay_vector::WgpuBindGroup0::LAYOUT_DESCRIPTOR,
    );
    let bind_group_entries = shader_bindings::sdf_clay_vector::WgpuBindGroup0Entries::new(
        shader_bindings::sdf_clay_vector::WgpuBindGroup0EntriesParams {
            uniforms_0: wgpu::BufferBinding {
                buffer: &uniform_buffer,
                offset: 0,
                size: None,
            },
            charGridCells_0: wgpu::BufferBinding {
                buffer: &char_grid_cells_buffer,
                offset: 0,
                size: None,
            },
            charGridIndices_0: wgpu::BufferBinding {
                buffer: &char_grid_indices_buffer,
                offset: 0,
                size: None,
            },
            charInstances_0: wgpu::BufferBinding {
                buffer: &char_instances_buffer,
                offset: 0,
                size: None,
            },
            glyphData_0: wgpu::BufferBinding {
                buffer: &glyph_data_buffer,
                offset: 0,
                size: None,
            },
            curves_0: wgpu::BufferBinding {
                buffer: &curves_buffer,
                offset: 0,
                size: None,
            },
        },
    );

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Bind Group"),
        layout: &bind_group_layout,
        entries: &bind_group_entries.as_array(),
    });

    // Create pipeline
    let shader_module =
        shader_bindings::sdf_clay_vector::create_shader_module_embed_source(&device);

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Clay Tablet Pipeline"),
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
                format: texture_format,
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

    // Render
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Render Encoder"),
    });

    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &texture_view,
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

        render_pass.set_pipeline(&pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    queue.submit(std::iter::once(encoder.finish()));

    // Copy texture to buffer and save
    let bytes_per_pixel = 4u32;
    let unpadded_bytes_per_row = width * bytes_per_pixel;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;
    let buffer_size = (padded_bytes_per_row * height) as u64;

    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Staging Buffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Copy Encoder"),
    });

    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging_buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(std::iter::once(encoder.finish()));

    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result).unwrap();
    });
    pollster::block_on(async {
        device.poll(wgpu::PollType::Wait).unwrap();
    });
    rx.recv()?.context("Failed to map buffer")?;

    let padded_data = buffer_slice.get_mapped_range();
    let mut image_data = Vec::with_capacity((unpadded_bytes_per_row * height) as usize);
    for row in 0..height {
        let start = (row * padded_bytes_per_row) as usize;
        let end = start + unpadded_bytes_per_row as usize;
        image_data.extend_from_slice(&padded_data[start..end]);
    }
    drop(padded_data);
    staging_buffer.unmap();

    std::fs::create_dir_all("output")?;
    image::save_buffer(
        "output/demo_clay.png",
        &image_data,
        width,
        height,
        image::ColorType::Rgba8,
    )?;
    println!("Screenshot saved to output/demo_clay.png");

    Ok(())
}
