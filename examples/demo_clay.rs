//! Demo 5: 3D Clay Tablet with Vector SDF Relief Text
//! Text carved into a clay slab using exact Bézier SDF
//!
//! Run with: cargo run --example demo_clay --features windowed
//! Screenshot: cargo run --example demo_clay

#[path = "../src/camera.rs"]
mod camera;
#[path = "../src/constants.rs"]
mod constants;
#[path = "../src/text/mod.rs"]
mod text;

#[allow(unused, non_snake_case, non_camel_case_types, non_upper_case_globals)]
mod shader_bindings {
    include!(concat!(env!("OUT_DIR"), "/shader_bindings.rs"));
}

use camera::OrbitalCamera;
use constants::{HEIGHT, WIDTH};
use text::{VectorFont, VectorFontAtlas};

use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

// ~175 words per paragraph
const LOREM: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum. Curabitur pretium tincidunt lacus. Nulla gravida orci a odio. Nullam varius, turpis et commodo pharetra, est eros bibendum elit, nec luctus magna felis sollicitudin mauris. Integer in mauris eu nibh euismod gravida. Duis ac tellus et risus vulputate vehicula. Donec lobortis risus a elit. Etiam tempor. Ut ullamcorper, ligula eu tempor congue, eros est euismod turpis, id tincidunt sapien risus a quam. Maecenas fermentum consequat mi. Donec fermentum. Pellentesque malesuada nulla a mi. Duis sapien sem, aliquet sed, vulputate eget, feugiat non, orci. Sed neque. Sed eget lacus. Mauris non dui nec urna suscipit nonummy. Fusce fermentum fermentum arcu. Vestibulum ante ipsum primis in faucibus orci luctus et ultrices posuere cubilia curae.";

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Uniforms {
    inv_view_proj: [[f32; 4]; 4],
    camera_pos_time: [f32; 4],
    light_dir_intensity: [f32; 4],
    render_params: [f32; 4], // xy = resolution, z = textDepth, w = textScale
    text_params: [f32; 4],   // x = charCount, y = numLines, z = lineHeight, w = reserved
}

impl Default for Uniforms {
    fn default() -> Self {
        Self {
            inv_view_proj: [[0.0; 4]; 4],
            camera_pos_time: [0.0, 2.0, 4.5, 0.0],
            light_dir_intensity: [0.5, 0.8, 0.3, 1.5],
            render_params: [WIDTH as f32, HEIGHT as f32, 0.2, 1.0], // depth, scale
            text_params: [0.0, 0.0, 0.4, 0.0],
        }
    }
}

impl Uniforms {
    fn update_from_camera(&mut self, camera: &OrbitalCamera, width: u32, height: u32, time: f32) {
        let aspect = width as f32 / height as f32;
        let view = camera.view_matrix();
        let proj = glam::Mat4::perspective_rh(45.0_f32.to_radians(), aspect, 0.1, 100.0);
        let view_proj = proj * view;
        let inv_view_proj = view_proj.inverse();

        self.inv_view_proj = inv_view_proj.to_cols_array_2d();
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

/// GPU grid cell (packed)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GpuGridCell {
    curve_start_and_count: u32,
}

/// GPU Bézier curve
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GpuBezierCurve {
    points01: [f32; 4],
    points2bbox: [f32; 4],
    bbox_flags: [f32; 4],
}

/// Glyph metadata for GPU
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GpuGlyphData {
    bounds: [f32; 4],
    grid_info: [u32; 4],  // gridOffset, gridSizeX, gridSizeY, unused
    curve_info: [u32; 4], // curveOffset, curveCount, unused, unused
}

/// Character instance for text layout
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GpuCharInstance {
    pos_and_char: [f32; 4], // xy = position, z = scale, w = glyph index
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
        format!("{} {} {} {} {} {}", LOREM, LOREM, LOREM, LOREM, LOREM, LOREM)
    );

    let scale = 0.15; // Character scale
    let line_height = 0.22;
    let margin = 0.15;

    let start_x = -tablet_width + margin;
    let start_z = -tablet_depth + margin; // Start at bottom (closest to camera)
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
            z += line_height; // Move away from camera (up the tablet)
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
                z += line_height;
                line_num += 1;
                if line_num >= max_lines {
                    break;
                }
            }

            instances.push(GpuCharInstance {
                pos_and_char: [x, z, scale, glyph_idx as f32],
            });

            x += advance;
        } else if ch == ' ' {
            x += 0.08 * scale;
            if x > max_x {
                x = start_x;
                z += line_height;
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
fn run_windowed() -> Result<()> {
    use std::collections::HashSet;
    use std::sync::Arc;
    use winit::{
        application::ApplicationHandler,
        event::{ElementState, KeyEvent, WindowEvent},
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        keyboard::{KeyCode, PhysicalKey},
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
        camera: OrbitalCamera,
        pressed_keys: HashSet<KeyCode>,
        start_time: std::time::Instant,
        char_count: u32,
    }

    impl Renderer {
        fn new(window: Arc<Window>) -> Result<Self> {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });

            let surface = instance.create_surface(window.clone())?;

            let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            }))
            .context("Failed to find a suitable GPU adapter")?;

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
            let font_data = std::fs::read("assets/fonts/DejaVuSans.ttf")
                .context("Failed to load font file")?;
            let font = VectorFont::from_ttf(&font_data).map_err(|e| anyhow::anyhow!(e))?;
            let atlas = VectorFontAtlas::from_font(&font, 8);

            // Build text layout
            let char_instances = build_clay_text_layout(&atlas, 2.3, 1.6);
            let char_count = char_instances.len() as u32;

            // Prepare GPU data
            let gpu_grid_cells: Vec<GpuGridCell> = atlas
                .grid_cells
                .iter()
                .map(|c| GpuGridCell {
                    curve_start_and_count: (c.curve_start as u32)
                        | ((c.curve_count as u32) << 16)
                        | ((c.flags as u32) << 24),
                })
                .collect();

            let gpu_curve_indices: Vec<u32> = atlas.curve_indices.iter().map(|&i| i as u32).collect();

            let gpu_curves: Vec<GpuBezierCurve> = atlas
                .curves
                .iter()
                .map(|c| {
                    let p0 = c.p0();
                    let p1 = c.p1();
                    let p2 = c.p2();
                    GpuBezierCurve {
                        points01: [p0.0, p0.1, p1.0, p1.1],
                        points2bbox: [p2.0, p2.1, c.bbox[0], c.bbox[1]],
                        bbox_flags: [c.bbox[2], c.bbox[3], c.flags as f32, 0.0],
                    }
                })
                .collect();

            let gpu_glyph_data: Vec<GpuGlyphData> = atlas
                .glyph_list
                .iter()
                .map(|(_, entry)| GpuGlyphData {
                    bounds: entry.bounds,
                    grid_info: [
                        entry.grid_offset,
                        entry.grid_size[0],
                        entry.grid_size[1],
                        0,
                    ],
                    curve_info: [
                        entry.curve_offset,
                        entry.curve_count,
                        0,
                        0,
                    ],
                })
                .collect();

            // Create buffers
            let mut camera = OrbitalCamera::default();
            camera.distance = 5.0;
            camera.elevation = 1.4; // Look almost straight down at the tablet
            camera.azimuth = 0.0;

            let mut uniforms = Uniforms::default();
            uniforms.update_from_camera(&camera, WIDTH, HEIGHT, 0.0);
            uniforms.text_params[0] = char_count as f32;

            let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Uniform Buffer"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

            let grid_cells_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Grid Cells Buffer"),
                contents: bytemuck::cast_slice(if gpu_grid_cells.is_empty() {
                    &[GpuGridCell { curve_start_and_count: 0 }]
                } else {
                    &gpu_grid_cells
                }),
                usage: wgpu::BufferUsages::STORAGE,
            });

            let curve_indices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Curve Indices Buffer"),
                contents: bytemuck::cast_slice(if gpu_curve_indices.is_empty() {
                    &[0u32]
                } else {
                    &gpu_curve_indices
                }),
                usage: wgpu::BufferUsages::STORAGE,
            });

            let curves_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Curves Buffer"),
                contents: bytemuck::cast_slice(if gpu_curves.is_empty() {
                    &[GpuBezierCurve {
                        points01: [0.0; 4],
                        points2bbox: [0.0; 4],
                        bbox_flags: [0.0; 4],
                    }]
                } else {
                    &gpu_curves
                }),
                usage: wgpu::BufferUsages::STORAGE,
            });

            let glyph_data_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Glyph Data Buffer"),
                contents: bytemuck::cast_slice(if gpu_glyph_data.is_empty() {
                    &[GpuGlyphData {
                        bounds: [0.0; 4],
                        grid_info: [0; 4],
                        curve_info: [0; 4],
                    }]
                } else {
                    &gpu_glyph_data
                }),
                usage: wgpu::BufferUsages::STORAGE,
            });

            let char_instances_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Char Instances Buffer"),
                contents: bytemuck::cast_slice(if char_instances.is_empty() {
                    &[GpuCharInstance { pos_and_char: [0.0; 4] }]
                } else {
                    &char_instances
                }),
                usage: wgpu::BufferUsages::STORAGE,
            });

            // Create bind group layout
            let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Bind Group"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: grid_cells_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: curve_indices_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: curves_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: glyph_data_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: char_instances_buffer.as_entire_binding(),
                    },
                ],
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
                pressed_keys: HashSet::new(),
                start_time: std::time::Instant::now(),
                char_count,
            })
        }

        fn update(&mut self) {
            const ROTATION_SPEED: f32 = 0.03;
            const ZOOM_SPEED: f32 = 0.1;

            if self.pressed_keys.contains(&KeyCode::KeyA) {
                self.camera.rotate_horizontal(-ROTATION_SPEED);
            }
            if self.pressed_keys.contains(&KeyCode::KeyD) {
                self.camera.rotate_horizontal(ROTATION_SPEED);
            }
            if self.pressed_keys.contains(&KeyCode::KeyW) {
                self.camera.zoom(ZOOM_SPEED);
            }
            if self.pressed_keys.contains(&KeyCode::KeyS) {
                self.camera.zoom(-ZOOM_SPEED);
            }
            if self.pressed_keys.contains(&KeyCode::KeyQ) {
                self.camera.rotate_vertical(ROTATION_SPEED);
            }
            if self.pressed_keys.contains(&KeyCode::KeyE) {
                self.camera.rotate_vertical(-ROTATION_SPEED);
            }
        }

        fn render(&self) -> Result<(), wgpu::SurfaceError> {
            let time = self.start_time.elapsed().as_secs_f32();
            let mut uniforms = Uniforms::default();
            uniforms.update_from_camera(&self.camera, self.config.width, self.config.height, time);
            uniforms.text_params[0] = self.char_count as f32;
            self.queue
                .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

            let output = self.surface.get_current_texture()?;
            let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

            let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
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

        fn handle_key(&mut self, event: KeyEvent) {
            if let PhysicalKey::Code(key_code) = event.physical_key {
                match event.state {
                    ElementState::Pressed => { self.pressed_keys.insert(key_code); }
                    ElementState::Released => { self.pressed_keys.remove(&key_code); }
                }
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
                    .with_title("Demo 5: Clay Tablet Vector SDF (A/D: rotate, W/S: zoom, Q/E: tilt)")
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

        fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
            let Some(renderer) = self.renderer.as_mut() else { return };

            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::KeyboardInput { event, .. } => {
                    if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
                        if event.state == ElementState::Pressed {
                            event_loop.exit();
                            return;
                        }
                    }
                    renderer.handle_key(event);
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
    let font_data = std::fs::read("assets/fonts/DejaVuSans.ttf")
        .context("Failed to load font file")?;
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

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("Headless Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
        },
    ))
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
    let gpu_grid_cells: Vec<GpuGridCell> = atlas
        .grid_cells
        .iter()
        .map(|c| GpuGridCell {
            curve_start_and_count: (c.curve_start as u32)
                | ((c.curve_count as u32) << 16)
                | ((c.flags as u32) << 24),
        })
        .collect();

    let gpu_curve_indices: Vec<u32> = atlas.curve_indices.iter().map(|&i| i as u32).collect();

    let gpu_curves: Vec<GpuBezierCurve> = atlas
        .curves
        .iter()
        .map(|c| {
            let p0 = c.p0();
            let p1 = c.p1();
            let p2 = c.p2();
            GpuBezierCurve {
                points01: [p0.0, p0.1, p1.0, p1.1],
                points2bbox: [p2.0, p2.1, c.bbox[0], c.bbox[1]],
                bbox_flags: [c.bbox[2], c.bbox[3], c.flags as f32, 0.0],
            }
        })
        .collect();

    let gpu_glyph_data: Vec<GpuGlyphData> = atlas
        .glyph_list
        .iter()
        .map(|(_, entry)| GpuGlyphData {
            bounds: entry.bounds,
            grid_info: [
                entry.grid_offset,
                entry.grid_size[0],
                entry.grid_size[1],
                0,
            ],
            curve_info: [
                entry.curve_offset,
                entry.curve_count,
                0,
                0,
            ],
        })
        .collect();

    // Create buffers
    let mut camera = OrbitalCamera::default();
    camera.distance = 5.0;
    camera.elevation = 1.4; // Look almost straight down at the tablet
    camera.azimuth = 0.0;

    let mut uniforms = Uniforms::default();
    uniforms.update_from_camera(&camera, width, height, 0.0);
    uniforms.text_params[0] = char_count as f32;

    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Uniform Buffer"),
        contents: bytemuck::cast_slice(&[uniforms]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let grid_cells_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Grid Cells Buffer"),
        contents: bytemuck::cast_slice(if gpu_grid_cells.is_empty() {
            &[GpuGridCell { curve_start_and_count: 0 }]
        } else {
            &gpu_grid_cells
        }),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let curve_indices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Curve Indices Buffer"),
        contents: bytemuck::cast_slice(if gpu_curve_indices.is_empty() {
            &[0u32]
        } else {
            &gpu_curve_indices
        }),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let curves_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Curves Buffer"),
        contents: bytemuck::cast_slice(if gpu_curves.is_empty() {
            &[GpuBezierCurve {
                points01: [0.0; 4],
                points2bbox: [0.0; 4],
                bbox_flags: [0.0; 4],
            }]
        } else {
            &gpu_curves
        }),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let glyph_data_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Glyph Data Buffer"),
        contents: bytemuck::cast_slice(if gpu_glyph_data.is_empty() {
            &[GpuGlyphData {
                bounds: [0.0; 4],
                grid_info: [0; 4],
                curve_info: [0; 4],
            }]
        } else {
            &gpu_glyph_data
        }),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let char_instances_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Char Instances Buffer"),
        contents: bytemuck::cast_slice(if char_instances.is_empty() {
            &[GpuCharInstance { pos_and_char: [0.0; 4] }]
        } else {
            &char_instances
        }),
        usage: wgpu::BufferUsages::STORAGE,
    });

    // Create bind group layout
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Bind Group Layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Bind Group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: grid_cells_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: curve_indices_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: curves_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: glyph_data_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: char_instances_buffer.as_entire_binding(),
            },
        ],
    });

    // Create pipeline
    let shader_module = shader_bindings::sdf_clay_vector::create_shader_module_embed_source(&device);

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
    pollster::block_on(async { device.poll(wgpu::PollType::Wait).unwrap(); });
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
