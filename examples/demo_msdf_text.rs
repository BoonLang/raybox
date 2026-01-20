//! Demo: MSDF Text Rendering
//! High-quality text using Multi-channel Signed Distance Fields

#[path = "../src/constants.rs"]
mod constants;

use constants::{HEIGHT, WIDTH};
use raybox::text::{MsdfAtlas, TextRenderer, GlyphInstance};
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

struct Renderer {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    text_renderer: TextRenderer,
    start_time: std::time::Instant,
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

        // Load atlas image
        let atlas_png = Path::new("assets/fonts/atlas.png");
        let atlas_image = image::open(atlas_png).context("Failed to load atlas image")?;
        let atlas_rgb = atlas_image.to_rgb8();
        let atlas_data = atlas_rgb.as_raw();

        // Create text renderer
        let text_renderer =
            TextRenderer::new(&device, &queue, surface_format, atlas, atlas_data)?;

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            text_renderer,
            start_time: std::time::Instant::now(),
        })
    }

    fn render(&self) -> Result<(), wgpu::SurfaceError> {
        let _time = self.start_time.elapsed().as_secs_f32();

        // Update screen size
        self.text_renderer.update_screen_size(
            &self.queue,
            self.config.width as f32,
            self.config.height as f32,
        );

        // Layout text
        let mut instances: Vec<GlyphInstance> = Vec::new();

        // Title
        let title_instances = self.text_renderer.layout_text(
            "RAYBOX",
            50.0,
            80.0,
            64.0,
            [0.2, 0.3, 0.8, 1.0], // Blue
        );
        instances.extend(title_instances);

        // Subtitle
        let subtitle_instances = self.text_renderer.layout_text(
            "Multi-channel Signed Distance Field Text",
            50.0,
            140.0,
            24.0,
            [0.3, 0.3, 0.3, 1.0], // Gray
        );
        instances.extend(subtitle_instances);

        // Body text
        let body_text = "MSDF text rendering provides crisp, resolution-independent
text at any size. Unlike traditional bitmap fonts, MSDF
preserves sharp corners and fine details even when scaled.

The quick brown fox jumps over the lazy dog.
ABCDEFGHIJKLMNOPQRSTUVWXYZ
abcdefghijklmnopqrstuvwxyz
0123456789 !@#$%^&*()";

        let mut y = 200.0;
        for line in body_text.lines() {
            let line_instances = self.text_renderer.layout_text(
                line,
                50.0,
                y,
                20.0,
                [0.1, 0.1, 0.1, 1.0], // Dark gray
            );
            instances.extend(line_instances);
            y += 28.0;
        }

        // Different sizes demo
        let sizes = [(12.0, "12px"), (16.0, "16px"), (24.0, "24px"), (32.0, "32px"), (48.0, "48px")];
        let mut x = 50.0;
        let y = 450.0;
        for (size, label) in sizes {
            let size_instances = self.text_renderer.layout_text(
                label,
                x,
                y,
                size,
                [0.6, 0.2, 0.2, 1.0], // Red
            );
            instances.extend(size_instances);
            x += size * 4.0;
        }

        // Render
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
                            r: 0.95,
                            g: 0.95,
                            b: 0.95,
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
                .with_title("MSDF Text Rendering Demo (ESC to quit)")
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
