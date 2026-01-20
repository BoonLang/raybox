use crate::camera::{OrbitalCamera, Uniforms};
use crate::constants::{HEIGHT, WIDTH};
use crate::shader_bindings::sdf_raymarch;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wgpu::util::DeviceExt;

struct WebRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    camera: OrbitalCamera,
    pressed_keys: HashSet<String>,
    start_time: f64,
}

impl WebRenderer {
    async fn new(canvas: web_sys::HtmlCanvasElement) -> Result<Self, JsValue> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| JsValue::from_str(&format!("Failed to create surface: {}", e)))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("No suitable GPU adapter found: {:?}", e)))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("RayBox SDF Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("Failed to create device: {}", e)))?;

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
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        // Get current time
        let start_time = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            uniform_buffer,
            uniform_bind_group,
            camera,
            pressed_keys: HashSet::new(),
            start_time,
        })
    }

    fn update(&mut self) {
        const ROTATION_SPEED: f32 = 0.03;
        const ZOOM_SPEED: f32 = 0.1;

        // A/D for horizontal rotation
        if self.pressed_keys.contains("KeyA") || self.pressed_keys.contains("a") {
            self.camera.rotate_horizontal(-ROTATION_SPEED);
        }
        if self.pressed_keys.contains("KeyD") || self.pressed_keys.contains("d") {
            self.camera.rotate_horizontal(ROTATION_SPEED);
        }

        // W/S for zoom
        if self.pressed_keys.contains("KeyW") || self.pressed_keys.contains("w") {
            self.camera.zoom(ZOOM_SPEED);
        }
        if self.pressed_keys.contains("KeyS") || self.pressed_keys.contains("s") {
            self.camera.zoom(-ZOOM_SPEED);
        }

        // Q/E for vertical rotation
        if self.pressed_keys.contains("KeyQ") || self.pressed_keys.contains("q") {
            self.camera.rotate_vertical(ROTATION_SPEED);
        }
        if self.pressed_keys.contains("KeyE") || self.pressed_keys.contains("e") {
            self.camera.rotate_vertical(-ROTATION_SPEED);
        }
    }

    fn update_uniforms(&self) {
        let current_time = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);
        let time = ((current_time - self.start_time) / 1000.0) as f32;

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

    fn key_down(&mut self, code: String) {
        self.pressed_keys.insert(code);
    }

    fn key_up(&mut self, code: String) {
        self.pressed_keys.remove(&code);
    }
}

fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    web_sys::window()
        .unwrap()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .unwrap();
}

#[wasm_bindgen(start)]
pub async fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).unwrap();

    log::info!("Initializing raybox SDF WebGPU renderer...");

    let window = web_sys::window().ok_or("No window found")?;
    let document = window.document().ok_or("No document found")?;
    let canvas = document
        .get_element_by_id("canvas")
        .ok_or("No canvas element found")?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    let renderer = WebRenderer::new(canvas).await?;
    log::info!("SDF Renderer initialized successfully");
    log::info!("Controls: A/D = rotate, W/S = zoom, Q/E = tilt");

    let renderer = Rc::new(RefCell::new(renderer));

    // Set up keyboard event listeners
    let renderer_keydown = renderer.clone();
    let keydown_closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
        renderer_keydown.borrow_mut().key_down(event.code());
    }) as Box<dyn FnMut(_)>);

    let renderer_keyup = renderer.clone();
    let keyup_closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
        renderer_keyup.borrow_mut().key_up(event.code());
    }) as Box<dyn FnMut(_)>);

    window.add_event_listener_with_callback("keydown", keydown_closure.as_ref().unchecked_ref())?;
    window.add_event_listener_with_callback("keyup", keyup_closure.as_ref().unchecked_ref())?;

    // Keep closures alive
    keydown_closure.forget();
    keyup_closure.forget();

    // Animation loop
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();

    let renderer_clone = renderer.clone();
    *g.borrow_mut() = Some(Closure::new(move || {
        renderer_clone.borrow_mut().update();
        if let Err(e) = renderer_clone.borrow().render() {
            log::error!("Render error: {:?}", e);
        }
        request_animation_frame(f.borrow().as_ref().unwrap());
    }));

    request_animation_frame(g.borrow().as_ref().unwrap());

    Ok(())
}
