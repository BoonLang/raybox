//! Demo runner for unified demo application
//!
//! Handles window management, demo switching, input, and overlay rendering.
//! Supports hot-reload with state preservation.

use super::{create_demo, Demo, DemoContext, DemoId, DemoType};
use crate::camera::FlyCamera;
use crate::constants::{HEIGHT, WIDTH};
#[allow(unused_imports)]
use crate::input::{InputAction, InputHandler, OverlayMode};

use crate::simple_overlay::SimpleOverlay;

#[cfg(feature = "control")]
use crate::control::{
    AppStatus, Command, ErrorCode, Response, ResponseMessage, SharedControlState,
};

#[cfg(feature = "hot-reload")]
use crate::hot_reload::{ReloadableState, OverlayModeState, ShaderLoader};

// Convert input::OverlayMode to OverlayModeState for hot-reload state
#[cfg(feature = "hot-reload")]
impl From<OverlayMode> for OverlayModeState {
    fn from(mode: OverlayMode) -> Self {
        match mode {
            OverlayMode::Off => Self::Off,
            OverlayMode::App => Self::App,
            OverlayMode::Full => Self::Full,
        }
    }
}

#[cfg(feature = "hot-reload")]
impl From<OverlayModeState> for OverlayMode {
    fn from(state: OverlayModeState) -> Self {
        match state {
            OverlayModeState::Off => Self::Off,
            OverlayModeState::App => Self::App,
            OverlayModeState::Full => Self::Full,
        }
    }
}

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::event::{DeviceEvent, ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};
use winit::application::ApplicationHandler;

/// Demo runner state
pub struct DemoRunner {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,

    // Current demo
    current_demo: Box<dyn Demo>,
    current_demo_id: DemoId,

    // Camera and input
    camera: FlyCamera,
    input: InputHandler,

    // Overlay
    overlay: SimpleOverlay,
    show_keybindings: bool,

    // 2D demo controls
    pressed_keys: HashSet<KeyCode>,

    // Timing
    start_time: std::time::Instant,
    last_frame_time: std::time::Instant,

    // Control server integration
    #[cfg(feature = "control")]
    control_state: Option<SharedControlState>,

    // Hot-reload shader loader
    #[cfg(feature = "hot-reload")]
    shader_loader: ShaderLoader,
}

impl DemoRunner {
    pub fn new(window: Arc<Window>, initial_demo: DemoId) -> Result<Self> {
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

        // Create demo context
        let ctx = DemoContext {
            device: &device,
            queue: &queue,
            surface_format,
            width: WIDTH,
            height: HEIGHT,
        };

        // Create initial demo
        let current_demo = create_demo(initial_demo, &ctx)?;

        // Setup camera with demo's config
        let camera_config = current_demo.camera_config();
        let input = InputHandler::new(camera_config.clone());
        let mut camera = FlyCamera::default();
        input.setup_camera(&mut camera);

        // Create overlay renderer
        let overlay = SimpleOverlay::new(&device, &queue, surface_format, WIDTH, HEIGHT)?;

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            current_demo,
            current_demo_id: initial_demo,
            camera,
            input,
            overlay,
            show_keybindings: false,
            pressed_keys: HashSet::new(),
            start_time: std::time::Instant::now(),
            last_frame_time: std::time::Instant::now(),
            #[cfg(feature = "control")]
            control_state: None,
            #[cfg(feature = "hot-reload")]
            shader_loader: ShaderLoader::default(),
        })
    }

    /// Set the control state for external control
    #[cfg(feature = "control")]
    pub fn set_control_state(&mut self, state: SharedControlState) {
        self.control_state = Some(state);
    }

    /// Save current state to file for hot-reload
    #[cfg(feature = "hot-reload")]
    pub fn save_state(&self) -> anyhow::Result<()> {
        let state = ReloadableState {
            current_demo: self.current_demo_id as u8,
            camera_position: self.camera.position.to_array(),
            camera_yaw: self.camera.yaw,
            camera_pitch: self.camera.pitch,
            camera_roll: self.camera.roll,
            move_speed: self.camera.move_speed,
            overlay_mode: self.input.overlay_mode.into(),
            show_keybindings: self.show_keybindings,
            time_offset: self.start_time.elapsed().as_secs_f32(),
            window_size: [self.config.width, self.config.height],
        };
        state.save_default()?;
        log::info!("State saved for hot-reload");
        Ok(())
    }

    /// Restore state from file after hot-reload
    #[cfg(feature = "hot-reload")]
    pub fn restore_state(&mut self) -> anyhow::Result<()> {
        let path = ReloadableState::default_path();
        if !std::path::Path::new(&path).exists() {
            return Ok(());
        }

        let state = ReloadableState::load_from_file(&path)?;

        // Restore demo
        if let Some(demo_id) = DemoId::from_u8(state.current_demo) {
            if demo_id != self.current_demo_id {
                let _ = self.switch_demo(demo_id);
            }
        }

        // Restore camera
        self.camera.position = glam::Vec3::from_array(state.camera_position);
        self.camera.yaw = state.camera_yaw;
        self.camera.pitch = state.camera_pitch;
        self.camera.roll = state.camera_roll;
        self.camera.move_speed = state.move_speed;

        // Restore overlay mode
        self.input.overlay_mode = state.overlay_mode.into();
        self.show_keybindings = state.show_keybindings;

        // Adjust start time to maintain animation continuity
        self.start_time = std::time::Instant::now() - std::time::Duration::from_secs_f32(state.time_offset);

        log::info!("State restored from hot-reload");

        // Delete the state file after successful restore
        let _ = std::fs::remove_file(&path);

        Ok(())
    }

    /// Reload shader for the current demo at runtime
    #[cfg(feature = "hot-reload")]
    pub fn reload_shader(&mut self, shader_name: &str) -> anyhow::Result<()> {
        log::info!("Reloading shader: {}", shader_name);

        // Check if this shader is used by the current demo
        let demo_shader = self.current_demo.shader_name();
        if demo_shader != Some(shader_name) {
            log::debug!("Shader {} not used by current demo (uses {:?})", shader_name, demo_shader);
            return Ok(());
        }

        // Compile the shader at runtime
        let result = self.shader_loader.load_shader(&self.device, shader_name);

        match result {
            Ok(shader_module) => {
                // Recreate the pipeline for the current demo
                self.current_demo.on_shader_reload(self.create_pipeline_for_shader(
                    &shader_module,
                    self.config.format,
                ));
                log::info!("Shader {} reloaded successfully", shader_name);
                Ok(())
            }
            Err(e) => {
                log::error!("Failed to reload shader {}: {}", shader_name, e);
                anyhow::bail!("Shader reload failed: {}", e)
            }
        }
    }

    /// Create a render pipeline from a shader module
    #[cfg(feature = "hot-reload")]
    fn create_pipeline_for_shader(
        &self,
        shader_module: &wgpu::ShaderModule,
        surface_format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        // Create a generic pipeline layout (demos that need specific layouts will override)
        let bind_group_layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Hot-reload Bind Group Layout"),
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

        let pipeline_layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Hot-reload Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Hot-reload Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader_module,
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
        })
    }

    fn switch_demo(&mut self, new_id: DemoId) -> Result<()> {
        if new_id == self.current_demo_id {
            return Ok(());
        }

        let ctx = DemoContext {
            device: &self.device,
            queue: &self.queue,
            surface_format: self.config.format,
            width: self.config.width,
            height: self.config.height,
        };

        let new_demo = create_demo(new_id, &ctx)?;

        // Setup camera with new demo's config
        let camera_config = new_demo.camera_config();
        self.input = InputHandler::new(camera_config);
        self.camera = FlyCamera::default();
        self.input.setup_camera(&mut self.camera);

        // Reset 2D controls if switching to 2D demo
        self.pressed_keys.clear();

        self.current_demo = new_demo;
        self.current_demo_id = new_id;

        self.update_window_title();
        Ok(())
    }

    fn update(&mut self) {
        let now = std::time::Instant::now();
        let dt = (now - self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;

        self.input.update_frame_time(dt);

        // Process control commands
        #[cfg(feature = "control")]
        self.process_control_commands();

        match self.current_demo.demo_type() {
            DemoType::Scene3D => {
                self.input.update_camera(&mut self.camera, dt);
            }
            DemoType::Scene2D => {
                self.update_2d_controls(dt);
            }
        }

        self.current_demo.update(dt, &mut self.camera);
    }

    /// Process pending control commands
    #[cfg(feature = "control")]
    fn process_control_commands(&mut self) {
        let state = match &self.control_state {
            Some(s) => s.clone(),
            None => return,
        };

        // Get pending command
        let pending = {
            let mut guard = match state.write() {
                Ok(g) => g,
                Err(_) => return,
            };
            guard.pop_command()
        };

        if let Some(cmd) = pending {
            let response = self.handle_control_command(cmd.id, cmd.command);

            // Send response
            if let Ok(mut guard) = state.write() {
                guard.push_response(response);
            }
        }
    }

    /// Handle a single control command
    #[cfg(feature = "control")]
    fn handle_control_command(&mut self, id: u64, command: Command) -> ResponseMessage {
        match command {
            Command::SwitchDemo { id: demo_id } => {
                match DemoId::from_u8(demo_id) {
                    Some(new_id) => {
                        if let Err(e) = self.switch_demo(new_id) {
                            ResponseMessage::error(id, ErrorCode::Internal, e.to_string())
                        } else {
                            ResponseMessage::success(id, Some(serde_json::json!({
                                "demo": demo_id,
                                "name": new_id.name()
                            })))
                        }
                    }
                    None => ResponseMessage::error(
                        id,
                        ErrorCode::InvalidDemoId,
                        format!("Invalid demo ID: {}", demo_id),
                    ),
                }
            }
            Command::SetCamera { position, yaw, pitch, roll } => {
                if let Some(pos) = position {
                    self.camera.position = glam::Vec3::from_array(pos);
                }
                if let Some(y) = yaw {
                    self.camera.yaw = y;
                }
                if let Some(p) = pitch {
                    self.camera.pitch = p;
                }
                if let Some(r) = roll {
                    self.camera.roll = r;
                }
                ResponseMessage::success(id, None)
            }
            Command::Screenshot { center_crop } => {
                self.capture_screenshot(id, center_crop)
            }
            Command::GetStatus => {
                let status = self.get_status();
                ResponseMessage::new(id, status.to_response())
            }
            Command::ToggleOverlay { mode } => {
                match mode.as_str() {
                    "off" => self.input.overlay_mode = OverlayMode::Off,
                    "app" => self.input.overlay_mode = OverlayMode::App,
                    "full" => self.input.overlay_mode = OverlayMode::Full,
                    _ => {}
                }
                ResponseMessage::success(id, None)
            }
            Command::PressKey { key } => {
                // Simulate key press (limited support)
                match key.as_str() {
                    "k" | "K" => self.show_keybindings = !self.show_keybindings,
                    "f" | "F" => self.input.toggle_overlay_app(),
                    "g" | "G" => self.input.toggle_overlay_full(),
                    "r" | "R" => self.input.reset_roll(&mut self.camera),
                    "t" | "T" => self.input.reset_camera(&mut self.camera),
                    _ => {}
                }
                ResponseMessage::success(id, None)
            }
            Command::ReloadShaders => {
                #[cfg(feature = "hot-reload")]
                {
                    // Try to reload the current demo's shader
                    if let Some(shader_name) = self.current_demo.shader_name() {
                        match self.reload_shader(shader_name) {
                            Ok(()) => ResponseMessage::success(id, Some(serde_json::json!({
                                "message": format!("Shader {} reloaded successfully", shader_name),
                                "shader": shader_name
                            }))),
                            Err(e) => ResponseMessage::error(id, ErrorCode::Internal, e.to_string()),
                        }
                    } else {
                        ResponseMessage::success(id, Some(serde_json::json!({
                            "message": "Current demo does not use a named shader"
                        })))
                    }
                }
                #[cfg(not(feature = "hot-reload"))]
                {
                    ResponseMessage::success(id, Some(serde_json::json!({
                        "message": "Shader reload requires hot-reload feature"
                    })))
                }
            }
            Command::Ping => {
                ResponseMessage::new(id, Response::Pong)
            }
        }
    }

    /// Capture a screenshot and return as base64, optionally cropping to a centered region
    #[cfg(feature = "control")]
    fn capture_screenshot(&self, id: u64, center_crop: Option<[u32; 2]>) -> ResponseMessage {
        // Create a texture to render to
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Screenshot Texture"),
            size: wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let time = self.start_time.elapsed().as_secs_f32();

        // Render to the texture
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Screenshot Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Screenshot Render Pass"),
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

            self.current_demo.render(&mut render_pass, &self.queue, time);
        }

        // Create buffer for reading back
        let bytes_per_row = (self.config.width * 4 + 255) & !255; // Align to 256
        let buffer_size = bytes_per_row * self.config.height;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Screenshot Buffer"),
            size: buffer_size as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Copy texture to buffer
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(self.config.height),
                },
            },
            wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        // Map and read buffer
        let buffer_slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });

        let _ = self.device.poll(wgpu::PollType::Wait);

        match rx.recv() {
            Ok(Ok(())) => {
                let data = buffer_slice.get_mapped_range();

                // Remove padding from rows
                let mut pixels = Vec::with_capacity((self.config.width * self.config.height * 4) as usize);
                for y in 0..self.config.height {
                    let start = (y * bytes_per_row) as usize;
                    let end = start + (self.config.width * 4) as usize;
                    pixels.extend_from_slice(&data[start..end]);
                }
                drop(data);
                buffer.unmap();

                // Encode as PNG using image crate
                let img: image::ImageBuffer<image::Rgba<u8>, _> = match image::ImageBuffer::from_raw(
                    self.config.width,
                    self.config.height,
                    pixels,
                ) {
                    Some(img) => img,
                    None => {
                        return ResponseMessage::error(
                            id,
                            ErrorCode::ScreenshotFailed,
                            "Failed to create image buffer".to_string(),
                        );
                    }
                };

                // Apply center crop if requested
                let (final_img, final_w, final_h): (Box<dyn std::ops::Deref<Target = [u8]>>, u32, u32) =
                    if let Some([crop_w, crop_h]) = center_crop {
                        let cw = crop_w.min(self.config.width);
                        let ch = crop_h.min(self.config.height);
                        let cx = (self.config.width.saturating_sub(cw)) / 2;
                        let cy = (self.config.height.saturating_sub(ch)) / 2;
                        let cropped = image::imageops::crop_imm(&img, cx, cy, cw, ch).to_image();
                        let w = cropped.width();
                        let h = cropped.height();
                        (Box::new(cropped.into_raw()), w, h)
                    } else {
                        (Box::new(img.into_raw()), self.config.width, self.config.height)
                    };

                let final_img_buf: image::ImageBuffer<image::Rgba<u8>, _> =
                    image::ImageBuffer::from_raw(final_w, final_h, final_img.to_vec()).unwrap();

                let mut png_data = Vec::new();
                let mut cursor = std::io::Cursor::new(&mut png_data);
                if let Err(e) = final_img_buf.write_to(&mut cursor, image::ImageFormat::Png) {
                    return ResponseMessage::error(
                        id,
                        ErrorCode::ScreenshotFailed,
                        format!("PNG encoding failed: {}", e),
                    );
                }

                // Base64 encode
                use base64::Engine;
                let base64_str = base64::engine::general_purpose::STANDARD.encode(&png_data);

                ResponseMessage::new(id, Response::Screenshot {
                    base64: base64_str,
                    width: final_w,
                    height: final_h,
                })
            }
            _ => ResponseMessage::error(
                id,
                ErrorCode::ScreenshotFailed,
                "Failed to read screenshot buffer".to_string(),
            ),
        }
    }

    /// Get current status
    #[cfg(feature = "control")]
    fn get_status(&self) -> AppStatus {
        let overlay_mode = match self.input.overlay_mode {
            OverlayMode::Off => "off",
            OverlayMode::App => "app",
            OverlayMode::Full => "full",
        };

        AppStatus {
            current_demo: self.current_demo_id as u8,
            demo_name: self.current_demo.name().to_string(),
            camera_position: self.camera.position.to_array(),
            camera_yaw: self.camera.yaw,
            camera_pitch: self.camera.pitch,
            camera_roll: self.camera.roll,
            fps: self.input.fps(),
            overlay_mode: overlay_mode.to_string(),
            show_keybindings: self.show_keybindings,
        }
    }

    fn update_2d_controls(&mut self, dt: f32) {
        if self.current_demo.demo_type() != DemoType::Scene2D {
            return;
        }

        let pan_speed = 200.0 * dt;
        let zoom_speed = 1.5 * dt;
        let rot_speed = 2.0 * dt;

        let mut offset_delta = [0.0f32, 0.0f32];
        let mut scale_factor = 1.0f32;
        let mut rotation_delta = 0.0f32;

        if self.pressed_keys.contains(&KeyCode::KeyA) {
            offset_delta[0] -= pan_speed;
        }
        if self.pressed_keys.contains(&KeyCode::KeyD) {
            offset_delta[0] += pan_speed;
        }
        if self.pressed_keys.contains(&KeyCode::KeyW) {
            offset_delta[1] += pan_speed;
        }
        if self.pressed_keys.contains(&KeyCode::KeyS) {
            offset_delta[1] -= pan_speed;
        }

        if self.pressed_keys.contains(&KeyCode::ArrowUp) {
            scale_factor *= 1.0 + zoom_speed;
        }
        if self.pressed_keys.contains(&KeyCode::ArrowDown) {
            scale_factor *= 1.0 - zoom_speed;
        }

        if self.pressed_keys.contains(&KeyCode::KeyQ) {
            rotation_delta += rot_speed;
        }
        if self.pressed_keys.contains(&KeyCode::KeyE) {
            rotation_delta -= rot_speed;
        }

        // Apply to demo (requires downcasting to each 2D demo type)
        if let Some(text2d) = self.current_demo.as_any_mut().downcast_mut::<super::text2d::Text2DDemo>() {
            text2d.offset[0] += offset_delta[0] / text2d.scale;
            text2d.offset[1] += offset_delta[1] / text2d.scale;
            text2d.scale *= scale_factor;
            text2d.scale = text2d.scale.clamp(0.1, 10.0);
            text2d.rotation += rotation_delta;
        } else if let Some(todomvc) = self.current_demo.as_any_mut().downcast_mut::<super::todomvc::TodoMvcDemo>() {
            todomvc.offset[0] += offset_delta[0] / todomvc.scale;
            todomvc.offset[1] += offset_delta[1] / todomvc.scale;
            todomvc.scale *= scale_factor;
            todomvc.scale = todomvc.scale.clamp(0.1, 10.0);
            todomvc.rotation += rotation_delta;
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let time = self.start_time.elapsed().as_secs_f32();

        // Update demo-specific uniforms
        self.update_demo_uniforms(time);

        // Update overlay
        self.update_overlay();

        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // Pass 1: Main scene
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Scene Render Pass"),
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

            self.current_demo.render(&mut render_pass, &self.queue, time);
        }

        // Pass 2: Overlay (preserves content, blends on top)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Overlay Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.overlay.render(&mut render_pass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        let _ = self.device.poll(wgpu::PollType::Poll);

        Ok(())
    }

    fn update_demo_uniforms(&self, time: f32) {
        // Update uniforms for demos that need camera data
        match self.current_demo_id {
            DemoId::Objects => {
                if let Some(demo) = self.current_demo.as_any().downcast_ref::<super::objects::ObjectsDemo>() {
                    demo.update_uniforms(&self.queue, &self.camera, time);
                }
            }
            DemoId::Spheres => {
                if let Some(demo) = self.current_demo.as_any().downcast_ref::<super::spheres::SpheresDemo>() {
                    demo.update_uniforms(&self.queue, &self.camera, time);
                }
            }
            DemoId::Towers => {
                if let Some(demo) = self.current_demo.as_any().downcast_ref::<super::towers::TowersDemo>() {
                    demo.update_uniforms(&self.queue, &self.camera, time);
                }
            }
            DemoId::Clay => {
                if let Some(demo) = self.current_demo.as_any().downcast_ref::<super::clay::ClayDemo>() {
                    demo.update_uniforms(&self.queue, &self.camera, time);
                }
            }
            DemoId::TextShadow => {
                if let Some(demo) = self.current_demo.as_any().downcast_ref::<super::text_shadow::TextShadowDemo>() {
                    demo.update_uniforms(&self.queue, &self.camera, time);
                }
            }
            _ => {}
        }
    }

    fn update_overlay(&mut self) {
        let stats = self.input.format_stats();
        let keybindings = if self.show_keybindings {
            let demo_bindings = self.current_demo.keybindings();
            let mut all_bindings = Vec::with_capacity(demo_bindings.len() + super::KEYBINDINGS_COMMON.len());
            all_bindings.extend_from_slice(demo_bindings);
            all_bindings.extend_from_slice(super::KEYBINDINGS_COMMON);
            Some(all_bindings)
        } else {
            None
        };

        self.overlay.update(
            &self.queue,
            &self.device,
            &stats,
            keybindings.as_deref(),
            self.config.width,
            self.config.height,
        );
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.current_demo.resize(new_size.width, new_size.height);
            self.overlay.resize(new_size.width, new_size.height);
        }
    }

    fn update_window_title(&self) {
        let title = format!(
            "Demo {}: {} | K: show keys",
            self.current_demo_id as u8,
            self.current_demo.name()
        );
        self.window.set_title(&title);
    }

    fn handle_key_pressed(&mut self, code: KeyCode, event_loop: &ActiveEventLoop) {
        // Number keys for demo switching
        match code {
            KeyCode::Digit0 => { let _ = self.switch_demo(DemoId::Empty); }
            KeyCode::Digit1 => { let _ = self.switch_demo(DemoId::Objects); }
            KeyCode::Digit2 => { let _ = self.switch_demo(DemoId::Spheres); }
            KeyCode::Digit3 => { let _ = self.switch_demo(DemoId::Towers); }
            KeyCode::Digit4 => { let _ = self.switch_demo(DemoId::Text2D); }
            KeyCode::Digit5 => { let _ = self.switch_demo(DemoId::Clay); }
            KeyCode::Digit6 => { let _ = self.switch_demo(DemoId::TextShadow); }
            KeyCode::Digit7 => { let _ = self.switch_demo(DemoId::TodoMvc); }
            KeyCode::Escape => {
                event_loop.exit();
            }
            _ => {}
        }

        // Track pressed keys for 2D controls
        self.pressed_keys.insert(code);
    }

    fn handle_key_released(&mut self, code: KeyCode) {
        self.pressed_keys.remove(&code);
    }
}

/// Demo application handler
pub struct DemoApp {
    runner: Option<DemoRunner>,
    initial_demo: DemoId,
    last_render: Instant,
    #[cfg(feature = "control")]
    control_state: Option<SharedControlState>,
}

impl DemoApp {
    pub fn new(initial_demo: DemoId) -> Self {
        Self {
            runner: None,
            initial_demo,
            last_render: Instant::now(),
            #[cfg(feature = "control")]
            control_state: None,
        }
    }

    /// Enable control mode with the given shared state
    #[cfg(feature = "control")]
    pub fn with_control(mut self, state: SharedControlState) -> Self {
        self.control_state = Some(state);
        self
    }
}

impl ApplicationHandler for DemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.runner.is_none() {
            let window_attrs = Window::default_attributes()
                .with_title("RayBox Demos")
                .with_inner_size(winit::dpi::PhysicalSize::new(WIDTH, HEIGHT));

            let window = Arc::new(event_loop.create_window(window_attrs).unwrap());

            match DemoRunner::new(window, self.initial_demo) {
                #[allow(unused_mut)]
                Ok(mut runner) => {
                    // Set control state if enabled
                    #[cfg(feature = "control")]
                    if let Some(ref state) = self.control_state {
                        runner.set_control_state(state.clone());
                    }

                    // Restore state from previous hot-reload if available
                    #[cfg(feature = "hot-reload")]
                    if let Err(e) = runner.restore_state() {
                        log::warn!("Failed to restore state: {}", e);
                    }

                    runner.update_window_title();
                    self.runner = Some(runner);
                }
                Err(e) => {
                    eprintln!("Failed to create demo runner: {}", e);
                    event_loop.exit();
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.runner.is_some() {
            let target_frametime = Duration::from_secs_f64(1.0 / 60.0);
            let next_frame = self.last_render + target_frametime;
            let now = Instant::now();
            if now >= next_frame {
                self.runner.as_ref().unwrap().window.request_redraw();
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(next_frame));
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        // Save state for hot-reload before exiting
        #[cfg(feature = "hot-reload")]
        if let Some(ref runner) = self.runner {
            if let Err(e) = runner.save_state() {
                log::warn!("Failed to save state: {}", e);
            }
        }

        self.runner.take();
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if let Some(runner) = self.runner.as_mut() {
            if runner.current_demo.demo_type() == DemoType::Scene3D {
                if let DeviceEvent::MouseMotion { delta } = event {
                    runner.input.handle_mouse_motion(&mut runner.camera, delta);
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(runner) = self.runner.as_mut() else { return };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            // Handle input actions for 3D demos
                            if runner.current_demo.demo_type() == DemoType::Scene3D {
                                if let Some(action) = runner.input.handle_key(event.clone()) {
                                    match action {
                                        InputAction::Exit => event_loop.exit(),
                                        InputAction::ToggleCapture => {
                                            runner.input.toggle_capture(&runner.window);
                                        }
                                        InputAction::ToggleOverlayApp => {
                                            runner.input.toggle_overlay_app();
                                        }
                                        InputAction::ToggleOverlayFull => {
                                            runner.input.toggle_overlay_full();
                                        }
                                        InputAction::ResetRoll => {
                                            runner.input.reset_roll(&mut runner.camera);
                                        }
                                        InputAction::ResetCamera => {
                                            runner.input.reset_camera(&mut runner.camera);
                                        }
                                        InputAction::ToggleKeybindings => {
                                            runner.show_keybindings = !runner.show_keybindings;
                                        }
                                    }
                                }
                            } else {
                                // Handle 2D demo specific keys
                                match code {
                                    KeyCode::KeyR => {
                                        if let Some(text2d) = runner.current_demo.as_any_mut().downcast_mut::<super::text2d::Text2DDemo>() {
                                            text2d.reset_rotation();
                                        } else if let Some(todomvc) = runner.current_demo.as_any_mut().downcast_mut::<super::todomvc::TodoMvcDemo>() {
                                            todomvc.reset_rotation();
                                        }
                                    }
                                    KeyCode::KeyT => {
                                        if let Some(text2d) = runner.current_demo.as_any_mut().downcast_mut::<super::text2d::Text2DDemo>() {
                                            text2d.reset_all();
                                        } else if let Some(todomvc) = runner.current_demo.as_any_mut().downcast_mut::<super::todomvc::TodoMvcDemo>() {
                                            todomvc.reset_all();
                                        }
                                    }
                                    KeyCode::KeyF => {
                                        runner.input.toggle_overlay_app();
                                    }
                                    KeyCode::KeyG => {
                                        runner.input.toggle_overlay_full();
                                    }
                                    KeyCode::KeyK => {
                                        runner.show_keybindings = !runner.show_keybindings;
                                    }
                                    _ => {}
                                }
                            }

                            runner.handle_key_pressed(code, event_loop);
                        }
                        ElementState::Released => {
                            if runner.current_demo.demo_type() == DemoType::Scene3D {
                                runner.input.handle_key(event);
                            }
                            runner.handle_key_released(code);
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if runner.current_demo.demo_type() == DemoType::Scene3D {
                    runner.input.handle_scroll(&mut runner.camera, delta);
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                if runner.current_demo.demo_type() == DemoType::Scene3D {
                    runner.input.capture(&runner.window);
                }
            }
            WindowEvent::Resized(size) => runner.resize(size),
            WindowEvent::RedrawRequested => {
                self.last_render = Instant::now();
                runner.update();
                if let Err(e) = runner.render() {
                    eprintln!("Render error: {:?}", e);
                }
            }
            _ => {}
        }
    }
}

/// Run the demo application
pub fn run(initial_demo: DemoId) -> Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = DemoApp::new(initial_demo);
    event_loop.run_app(&mut app)?;
    Ok(())
}

/// Run the demo application with control server enabled
#[cfg(feature = "control")]
pub fn run_with_control(initial_demo: DemoId, port: Option<u16>) -> Result<()> {
    use crate::control::{new_shared_state, WsServer, DEFAULT_WS_PORT};

    let port = port.unwrap_or(DEFAULT_WS_PORT);

    // Create shared state
    let _state = new_shared_state();

    // Create WebSocket server
    let ws_server = WsServer::new();
    let ws_state = ws_server.state();

    // We need to copy our state to the WS server's state
    // Actually, let's use the WS server's state directly
    let control_state = ws_state;

    // Start WebSocket server in a background thread
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async {
            if let Err(e) = ws_server.run(port).await {
                log::error!("WebSocket server error: {}", e);
            }
        });
    });

    log::info!("Control server started on ws://127.0.0.1:{}", port);

    // Run the event loop
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = DemoApp::new(initial_demo).with_control(control_state);
    event_loop.run_app(&mut app)?;
    Ok(())
}
