//! Demo: MSDF Text Rendering
//! High-quality text using Multi-channel Signed Distance Fields

#[path = "../src/constants.rs"]
mod constants;

use constants::{HEIGHT, WIDTH};
use raybox::text::{GlyphInstance, MsdfAtlas, TextRenderer};
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

// ~200 words per paragraph, need 5+ paragraphs for 1000+ words
const LOREM: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum. Curabitur pretium tincidunt lacus. Nulla gravida orci a odio. Nullam varius, turpis et commodo pharetra, est eros bibendum elit, nec luctus magna felis sollicitudin mauris. Integer in mauris eu nibh euismod gravida. Duis ac tellus et risus vulputate vehicula. Donec lobortis risus a elit. Etiam tempor. Ut ullamcorper, ligula eu tempor congue, eros est euismod turpis, id tincidunt sapien risus a quam. Maecenas fermentum consequat mi. Donec fermentum. Pellentesque malesuada nulla a mi. Duis sapien sem, aliquet sed, vulputate eget, feugiat non, orci. Sed neque. Sed eget lacus. Mauris non dui nec urna suscipit nonummy. Fusce fermentum fermentum arcu. Vestibulum ante ipsum primis in faucibus orci luctus et ultrices posuere cubilia curae. Proin ut est. Aliquam odio. Pellentesque massa turpis, cursus eu, euismod nec, tempor congue, nulla. Duis viverra gravida mauris. Cras tincidunt. Curabitur eros ligula, varius sit amet, nunc. Vivamus aliquet, orci nec malesuada elementum, justo mi convallis eros, eu commodo diam mi a enim.";

// Window height for 1000+ words at 16px with proper line height
const TEXT_WINDOW_HEIGHT: u32 = 1200;

struct Renderer {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    text_renderer: TextRenderer,
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

        // Load MSDF atlas
        let atlas_json = Path::new("assets/fonts/atlas.json");
        let atlas = MsdfAtlas::load(atlas_json).context("Failed to load MSDF atlas")?;

        let atlas_png = Path::new("assets/fonts/atlas.png");
        let atlas_image = image::open(atlas_png).context("Failed to load atlas image")?;
        let atlas_rgb = atlas_image.to_rgb8();
        let atlas_data = atlas_rgb.as_raw();

        let text_renderer =
            TextRenderer::new(&device, &queue, surface_format, atlas, atlas_data)?;

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            text_renderer,
        })
    }

    fn render(&self) -> Result<(), wgpu::SurfaceError> {
        self.text_renderer.update_screen_size(
            &self.queue,
            self.config.width as f32,
            self.config.height as f32,
        );

        let mut instances: Vec<GlyphInstance> = Vec::new();

        // Title - large
        instances.extend(self.text_renderer.layout_text(
            "RAYBOX SDF TEXT ENGINE",
            20.0,
            20.0,
            48.0,
            [0.1, 0.2, 0.5, 1.0],
        ));

        // Subtitle
        instances.extend(self.text_renderer.layout_text(
            "Multi-channel Signed Distance Field Rendering",
            20.0,
            75.0,
            20.0,
            [0.4, 0.4, 0.4, 1.0],
        ));

        // Body text - 16px, wrapped manually
        let wrap_width = 90; // characters per line
        let mut y = 110.0;
        let font_size = 16.0;
        let line_height = 22.0;

        // Generate 1000+ words by repeating lorem ipsum 6 times (~1050 words)
        let full_text = format!("{} {} {} {} {} {}", LOREM, LOREM, LOREM, LOREM, LOREM, LOREM);

        let words: Vec<&str> = full_text.split_whitespace().collect();
        let mut line = String::new();

        for word in words {
            if line.len() + word.len() + 1 > wrap_width {
                instances.extend(self.text_renderer.layout_text(
                    &line,
                    20.0,
                    y,
                    font_size,
                    [0.15, 0.15, 0.15, 1.0],
                ));
                y += line_height;
                line.clear();

                if y > self.config.height as f32 - 30.0 {
                    break;
                }
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }

        if !line.is_empty() && y <= self.config.height as f32 - 30.0 {
            instances.extend(self.text_renderer.layout_text(
                &line,
                20.0,
                y,
                font_size,
                [0.15, 0.15, 0.15, 1.0],
            ));
        }

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
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.98,
                            g: 0.98,
                            b: 0.96,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.text_renderer.render(&mut render_pass, &self.queue, &instances);
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
                .with_title("MSDF Text Demo (ESC to quit)")
                .with_inner_size(winit::dpi::PhysicalSize::new(WIDTH, TEXT_WINDOW_HEIGHT));

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
                    }
                }
            }
            WindowEvent::Resized(size) => renderer.resize(size),
            WindowEvent::RedrawRequested => {
                if let Err(e) = renderer.render() {
                    eprintln!("Render error: {:?}", e);
                }
                renderer.window.request_redraw();
            }
            _ => {}
        }
    }
}

fn main() -> Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App { renderer: None };
    event_loop.run_app(&mut app)?;
    Ok(())
}
