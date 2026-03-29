//! Demo runner for unified demo application
//!
//! Handles window management, demo switching, input, and overlay rendering.
//! Supports hot-reload with state preservation.

use super::{
    create_demo, CompiledFrameGraph, CompiledFramePass, Demo, DemoContext, DemoId, DemoType,
    EffectPacket, FramePacket, FrameTarget, OverlayPacket, PresentPacket, UiLayerPacket,
    WorldViewPacket,
};
use crate::camera::FlyCamera;
use crate::constants::{HEIGHT, WIDTH};
use crate::gpu_runtime_common::{PresentHost, TextureCompositeHost, PRESENT_INTERMEDIATE_FORMAT};
#[allow(unused_imports)]
use crate::input::{InputAction, InputHandler, OverlayMode};

use crate::simple_overlay::SimpleOverlay;

#[cfg(feature = "control")]
use crate::control::{
    AppStatus, Command, ErrorCode, Response, ResponseMessage, SharedControlState,
};

#[cfg(feature = "hot-reload")]
use crate::hot_reload::{OverlayModeState, ReloadableState, ShaderLoader};

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
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

const ACTIVE_FRAME_TIME: Duration = Duration::from_micros(16_667);

#[derive(Debug, Clone, Copy)]
#[cfg_attr(not(feature = "control"), allow(dead_code))]
enum DemoUserEvent {
    Wake,
}

#[cfg(feature = "control")]
fn parse_control_keycode(key: &str) -> Option<KeyCode> {
    match key {
        "0" => Some(KeyCode::Digit0),
        "1" => Some(KeyCode::Digit1),
        "2" => Some(KeyCode::Digit2),
        "3" => Some(KeyCode::Digit3),
        "4" => Some(KeyCode::Digit4),
        "5" => Some(KeyCode::Digit5),
        "6" => Some(KeyCode::Digit6),
        "7" => Some(KeyCode::Digit7),
        "8" => Some(KeyCode::Digit8),
        "9" => Some(KeyCode::Digit9),
        "-" => Some(KeyCode::Minus),
        "=" => Some(KeyCode::Equal),
        "a" | "A" => Some(KeyCode::KeyA),
        "b" | "B" => Some(KeyCode::KeyB),
        "c" | "C" => Some(KeyCode::KeyC),
        "d" | "D" => Some(KeyCode::KeyD),
        "e" | "E" => Some(KeyCode::KeyE),
        "f" | "F" => Some(KeyCode::KeyF),
        "g" | "G" => Some(KeyCode::KeyG),
        "h" | "H" => Some(KeyCode::KeyH),
        "i" | "I" => Some(KeyCode::KeyI),
        "j" | "J" => Some(KeyCode::KeyJ),
        "k" | "K" => Some(KeyCode::KeyK),
        "l" | "L" => Some(KeyCode::KeyL),
        "m" | "M" => Some(KeyCode::KeyM),
        "n" | "N" => Some(KeyCode::KeyN),
        "o" | "O" => Some(KeyCode::KeyO),
        "p" | "P" => Some(KeyCode::KeyP),
        "q" | "Q" => Some(KeyCode::KeyQ),
        "r" | "R" => Some(KeyCode::KeyR),
        "s" | "S" => Some(KeyCode::KeyS),
        "t" | "T" => Some(KeyCode::KeyT),
        "u" | "U" => Some(KeyCode::KeyU),
        "v" | "V" => Some(KeyCode::KeyV),
        "w" | "W" => Some(KeyCode::KeyW),
        "x" | "X" => Some(KeyCode::KeyX),
        "y" | "Y" => Some(KeyCode::KeyY),
        "z" | "Z" => Some(KeyCode::KeyZ),
        "up" | "Up" | "UP" => Some(KeyCode::ArrowUp),
        "down" | "Down" | "DOWN" => Some(KeyCode::ArrowDown),
        "left" | "Left" | "LEFT" => Some(KeyCode::ArrowLeft),
        "right" | "Right" | "RIGHT" => Some(KeyCode::ArrowRight),
        _ => None,
    }
}

#[cfg(feature = "control")]
fn apply_list_item_command(
    demo: &mut dyn Demo,
    index: usize,
    completed: Option<bool>,
    label: Option<&str>,
    toggle: bool,
) -> bool {
    let mut changed = false;

    if toggle {
        changed |= demo.toggle_list_item(index);
    }
    if let Some(completed) = completed {
        changed |= demo.set_list_item_completed(index, completed);
    }
    if let Some(label) = label {
        changed |= demo.set_list_item_label(index, label);
    }

    changed
}

/// Demo runner state
struct OffscreenTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

pub struct DemoRunner {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,

    // Current demo
    current_demo: Box<dyn Demo>,

    // Camera and input
    camera: FlyCamera,
    input: InputHandler,

    // Overlay
    overlay: SimpleOverlay,
    present_host: PresentHost,
    composite_host: TextureCompositeHost,
    offscreen_targets: HashMap<Cow<'static, str>, OffscreenTarget>,
    show_keybindings: bool,

    // When true, all keyboard input is ignored (Tab to pause, mouse click to resume)
    keyboard_paused: bool,

    // 2D demo controls
    pressed_keys: HashSet<KeyCode>,

    // Timing
    start_time: std::time::Instant,
    last_frame_time: std::time::Instant,
    needs_redraw: bool,
    was_continuous_redraw_active: bool,

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

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
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

        // Create demo context
        let ctx = DemoContext {
            device: &device,
            queue: &queue,
            surface_format: PRESENT_INTERMEDIATE_FORMAT,
            width: WIDTH,
            height: HEIGHT,
            scale_factor: window.scale_factor() as f32,
        };

        // Create initial demo
        let current_demo = create_demo(initial_demo, &ctx)?;

        // Setup camera with demo's config
        let camera_config = current_demo.camera_config();
        let input = InputHandler::new(camera_config.clone());
        let mut camera = FlyCamera::default();
        input.setup_camera(&mut camera);

        // Create overlay renderer
        let overlay =
            SimpleOverlay::new(&device, &queue, PRESENT_INTERMEDIATE_FORMAT, WIDTH, HEIGHT)?;
        let present_host = PresentHost::new(&device, WIDTH, HEIGHT, surface_format, "Native");
        let composite_host =
            TextureCompositeHost::new(&device, PRESENT_INTERMEDIATE_FORMAT, "Native");

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            current_demo,
            camera,
            input,
            overlay,
            present_host,
            composite_host,
            offscreen_targets: HashMap::new(),
            show_keybindings: false,
            keyboard_paused: true,
            pressed_keys: HashSet::new(),
            start_time: std::time::Instant::now(),
            last_frame_time: std::time::Instant::now(),
            needs_redraw: true,
            was_continuous_redraw_active: false,
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
            current_demo: self.current_demo.id() as u8,
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
            if demo_id != self.current_demo.id() {
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
        self.start_time =
            std::time::Instant::now() - std::time::Duration::from_secs_f32(state.time_offset);

        log::info!("State restored from hot-reload");

        // Delete the state file after successful restore
        let _ = std::fs::remove_file(&path);

        self.mark_needs_redraw();

        Ok(())
    }

    /// Reload shader for the current demo at runtime
    #[cfg(feature = "hot-reload")]
    pub fn reload_shader(&mut self, shader_name: &str) -> anyhow::Result<()> {
        log::info!("Reloading shader: {}", shader_name);

        // Check if this shader is used by the current demo
        let demo_shader = self.current_demo.shader_name();
        if demo_shader != Some(shader_name) {
            log::debug!(
                "Shader {} not used by current demo (uses {:?})",
                shader_name,
                demo_shader
            );
            return Ok(());
        }

        // Compile the shader at runtime
        let result = self.shader_loader.load_shader(&self.device, shader_name);

        match result {
            Ok(shader_module) => {
                // Recreate the pipeline for the current demo
                self.current_demo.on_shader_reload(
                    self.create_pipeline_for_shader(&shader_module, self.config.format),
                );
                self.mark_needs_redraw();
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
        let bind_group_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Hot-reload Bind Group Layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<
                                crate::shader_bindings::sdf_raymarch::Uniforms_std140_0,
                            >(
                            )
                                as u64),
                        },
                        count: None,
                    }],
                });

        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Hot-reload Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
        if new_id == self.current_demo.id() {
            return Ok(());
        }

        let ctx = DemoContext {
            device: &self.device,
            queue: &self.queue,
            surface_format: PRESENT_INTERMEDIATE_FORMAT,
            width: self.config.width,
            height: self.config.height,
            scale_factor: self.window.scale_factor() as f32,
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
        self.offscreen_targets.clear();
        self.reset_frame_timing();
        self.mark_needs_redraw();

        self.update_window_title();
        Ok(())
    }

    fn update(&mut self) {
        let now = std::time::Instant::now();
        let dt = (now - self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;

        self.input.update_frame_time(dt);
        self.sync_demo_scale_factor();

        // Process control commands
        #[cfg(feature = "control")]
        self.process_control_commands();

        let demo_type = self.current_demo.demo_type();
        if demo_type.uses_camera_controls() {
            self.input.update_camera(&mut self.camera, dt);
        } else if demo_type.uses_2d_view_controls() {
            self.update_2d_controls(dt);
        }

        self.current_demo.update(dt, &mut self.camera);
    }

    fn mark_needs_redraw(&mut self) {
        self.needs_redraw = true;
    }

    fn reset_frame_timing(&mut self) {
        self.last_frame_time = Instant::now();
    }

    fn wants_continuous_redraw(&self) -> bool {
        self.current_demo.wants_continuous_redraw()
    }

    fn demo_needs_redraw(&self) -> bool {
        self.current_demo.needs_redraw()
    }

    fn has_active_interaction(&self) -> bool {
        match self.current_demo.demo_type() {
            DemoType::Ui2D => self.has_active_2d_interaction(),
            DemoType::UiPhysical => self.has_active_ui_physical_interaction(),
            DemoType::World3D => self.has_active_world3d_interaction(),
        }
    }

    fn has_active_world3d_interaction(&self) -> bool {
        const CONTINUOUS_KEYS: &[KeyCode] = &[
            KeyCode::KeyW,
            KeyCode::KeyA,
            KeyCode::KeyS,
            KeyCode::KeyD,
            KeyCode::Space,
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
            KeyCode::KeyQ,
            KeyCode::KeyE,
        ];

        CONTINUOUS_KEYS
            .iter()
            .any(|key| self.input.pressed_keys.contains(key))
    }

    fn has_active_ui_physical_interaction(&self) -> bool {
        const CONTINUOUS_KEYS: &[KeyCode] = &[
            KeyCode::KeyW,
            KeyCode::KeyA,
            KeyCode::KeyS,
            KeyCode::KeyD,
            KeyCode::Space,
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
            KeyCode::KeyQ,
            KeyCode::KeyE,
        ];

        CONTINUOUS_KEYS
            .iter()
            .any(|key| self.input.pressed_keys.contains(key))
    }

    fn has_active_2d_interaction(&self) -> bool {
        const CONTINUOUS_KEYS: &[KeyCode] = &[
            KeyCode::KeyW,
            KeyCode::KeyA,
            KeyCode::KeyS,
            KeyCode::KeyD,
            KeyCode::ArrowUp,
            KeyCode::ArrowDown,
            KeyCode::KeyQ,
            KeyCode::KeyE,
        ];

        CONTINUOUS_KEYS
            .iter()
            .any(|key| self.pressed_keys.contains(key))
    }

    fn continuous_redraw_active(&self) -> bool {
        self.wants_continuous_redraw() || self.has_active_interaction()
    }

    fn handle_continuous_redraw_transition(&mut self) {
        let is_active = self.has_active_interaction();
        if is_active && !self.was_continuous_redraw_active {
            self.reset_frame_timing();
        }
        self.was_continuous_redraw_active = is_active;
    }

    #[cfg(feature = "control")]
    fn has_pending_control_commands(&self) -> bool {
        self.control_state
            .as_ref()
            .and_then(|state| state.read().ok())
            .map(|state| state.has_commands())
            .unwrap_or(false)
    }

    /// Process pending control commands
    #[cfg(feature = "control")]
    fn process_control_commands(&mut self) {
        let state = match &self.control_state {
            Some(s) => s.clone(),
            None => return,
        };

        loop {
            let pending = {
                let mut guard = match state.write() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                guard.pop_command()
            };

            let Some(cmd) = pending else {
                break;
            };

            let response = self.handle_control_command(cmd.id, cmd.command);

            if let Ok(mut guard) = state.write() {
                guard.push_response(response);
            }
        }
    }

    /// Handle a single control command
    #[cfg(feature = "control")]
    fn handle_control_command(&mut self, id: u64, command: Command) -> ResponseMessage {
        match command {
            Command::SwitchDemo { id: demo_id } => match DemoId::from_u8(demo_id) {
                Some(new_id) => {
                    if let Err(e) = self.switch_demo(new_id) {
                        ResponseMessage::error(id, ErrorCode::Internal, e.to_string())
                    } else {
                        ResponseMessage::success(
                            id,
                            Some(serde_json::json!({
                                "demo": demo_id,
                                "name": new_id.name()
                            })),
                        )
                    }
                }
                None => ResponseMessage::error(
                    id,
                    ErrorCode::InvalidDemoId,
                    format!("Invalid demo ID: {}", demo_id),
                ),
            },
            Command::SetCamera {
                position,
                yaw,
                pitch,
                roll,
            } => {
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
            Command::Screenshot { center_crop } => self.capture_screenshot(id, center_crop),
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
                    "n" | "N" => {
                        let _ = self.current_demo.handle_key_pressed(KeyCode::KeyN);
                    }
                    "m" | "M" => {
                        let _ = self.current_demo.handle_key_pressed(KeyCode::KeyM);
                    }
                    _ => {
                        if let Some(code) = parse_control_keycode(&key) {
                            let _ = self.current_demo.handle_key_pressed(code);
                        }
                    }
                }
                ResponseMessage::success(id, None)
            }
            Command::ReloadShaders => {
                #[cfg(feature = "hot-reload")]
                {
                    // Try to reload the current demo's shader
                    if let Some(shader_name) = self.current_demo.shader_name() {
                        match self.reload_shader(shader_name) {
                            Ok(()) => ResponseMessage::success(
                                id,
                                Some(serde_json::json!({
                                    "message": format!("Shader {} reloaded successfully", shader_name),
                                    "shader": shader_name
                                })),
                            ),
                            Err(e) => {
                                ResponseMessage::error(id, ErrorCode::Internal, e.to_string())
                            }
                        }
                    } else {
                        ResponseMessage::success(
                            id,
                            Some(serde_json::json!({
                                "message": "Current demo does not use a named shader"
                            })),
                        )
                    }
                }
                #[cfg(not(feature = "hot-reload"))]
                {
                    ResponseMessage::success(
                        id,
                        Some(serde_json::json!({
                            "message": "Shader reload requires hot-reload feature"
                        })),
                    )
                }
            }
            Command::SetTheme { theme, dark_mode } => {
                let options = self.current_demo.named_theme_options();
                if options.is_empty() {
                    ResponseMessage::error(
                        id,
                        ErrorCode::InvalidCommand,
                        "Current demo does not support named themes".to_string(),
                    )
                } else if let Some((theme_name, dark_mode)) =
                    self.current_demo.set_named_theme(&theme, dark_mode)
                {
                    ResponseMessage::success(
                        id,
                        Some(serde_json::json!({
                            "theme": theme_name,
                            "dark_mode": dark_mode,
                        })),
                    )
                } else {
                    let message = if options.is_empty() {
                        format!("Invalid theme: {}", theme)
                    } else {
                        format!("Invalid theme: {}. Valid: {}", theme, options.join(", "))
                    };
                    ResponseMessage::error(id, ErrorCode::InvalidTheme, message)
                }
            }
            Command::SetListItem {
                index,
                completed,
                label,
                toggle,
            } => {
                if !self.current_demo.has_list_command_target() {
                    ResponseMessage::error(
                        id,
                        ErrorCode::InvalidCommand,
                        "Current demo does not support list item commands".to_string(),
                    )
                } else {
                    let index = index as usize;
                    let changed = apply_list_item_command(
                        self.current_demo.as_mut(),
                        index,
                        completed,
                        label.as_deref(),
                        toggle,
                    );

                    ResponseMessage::success(
                        id,
                        Some(serde_json::json!({
                            "index": index,
                            "changed": changed,
                        })),
                    )
                }
            }
            Command::SetListFilter { filter } => {
                if !self.current_demo.has_list_command_target() {
                    ResponseMessage::error(
                        id,
                        ErrorCode::InvalidCommand,
                        "Current demo does not support list filter commands".to_string(),
                    )
                } else if let Some(filter_kind) =
                    crate::demo_core::ListFilter::from_str(&filter.to_ascii_lowercase())
                {
                    let changed = self.current_demo.set_list_filter(filter_kind);

                    ResponseMessage::success(
                        id,
                        Some(serde_json::json!({
                            "filter": filter_kind.name(),
                            "changed": changed,
                        })),
                    )
                } else {
                    ResponseMessage::error(
                        id,
                        ErrorCode::InvalidCommand,
                        format!(
                            "Invalid list filter: {}. Valid: all, active, completed",
                            filter
                        ),
                    )
                }
            }
            Command::SetListScroll { offset_y } => {
                if !self.current_demo.has_list_command_target() {
                    ResponseMessage::error(
                        id,
                        ErrorCode::InvalidCommand,
                        "Current demo does not support list scroll commands".to_string(),
                    )
                } else {
                    self.current_demo.set_list_scroll_offset(offset_y);

                    ResponseMessage::success(
                        id,
                        Some(serde_json::json!({
                            "offset_y": offset_y,
                        })),
                    )
                }
            }
            Command::SetNamedScroll { name, offset_y } => {
                if !self.current_demo.has_named_scroll_target() {
                    ResponseMessage::error(
                        id,
                        ErrorCode::InvalidCommand,
                        "Current demo does not support named scroll commands".to_string(),
                    )
                } else {
                    let changed = self.current_demo.set_named_scroll_offset(&name, offset_y);
                    ResponseMessage::success(
                        id,
                        Some(serde_json::json!({
                            "name": name,
                            "offset_y": offset_y,
                            "changed": changed,
                        })),
                    )
                }
            }
            Command::Ping => ResponseMessage::new(id, Response::Pong),
        }
    }

    /// Capture a screenshot and return as base64, optionally cropping to a centered region
    #[cfg(feature = "control")]
    fn capture_screenshot(&mut self, id: u64, center_crop: Option<[u32; 2]>) -> ResponseMessage {
        let time = self.start_time.elapsed().as_secs_f32();

        self.prepare_frame(time);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Screenshot Encoder"),
            });

        let mut frame_packet = self.current_demo.build_frame_packet(time);
        frame_packet.push_overlay(OverlayPacket::new("Overlay"));
        let frame_graph = frame_packet.compile();
        self.execute_frame_graph(&frame_graph, &mut encoder, None, time);

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
                texture: self.present_host.scene_texture(),
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
                let mut pixels =
                    Vec::with_capacity((self.config.width * self.config.height * 4) as usize);
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
                let (final_img, final_w, final_h): (
                    Box<dyn std::ops::Deref<Target = [u8]>>,
                    u32,
                    u32,
                ) = if let Some([crop_w, crop_h]) = center_crop {
                    let cw = crop_w.min(self.config.width);
                    let ch = crop_h.min(self.config.height);
                    let cx = (self.config.width.saturating_sub(cw)) / 2;
                    let cy = (self.config.height.saturating_sub(ch)) / 2;
                    let cropped = image::imageops::crop_imm(&img, cx, cy, cw, ch).to_image();
                    let w = cropped.width();
                    let h = cropped.height();
                    (Box::new(cropped.into_raw()), w, h)
                } else {
                    (
                        Box::new(img.into_raw()),
                        self.config.width,
                        self.config.height,
                    )
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

                ResponseMessage::new(
                    id,
                    Response::Screenshot {
                        base64: base64_str,
                        width: final_w,
                        height: final_h,
                    },
                )
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
            current_demo: self.current_demo.id() as u8,
            demo_name: self.current_demo.name().to_string(),
            demo_family: self.current_demo.demo_type().family_name().to_string(),
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
        if !self.current_demo.demo_type().uses_2d_view_controls() {
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

        self.current_demo
            .apply_2d_view_controls(offset_delta, scale_factor, rotation_delta);
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let time = self.start_time.elapsed().as_secs_f32();
        self.prepare_frame(time);

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        let frame_packet = self.build_frame_packet(time);
        let frame_graph = frame_packet.compile();
        self.execute_frame_graph(&frame_graph, &mut encoder, Some(&view), time);

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        let _ = self.device.poll(wgpu::PollType::Poll);
        self.needs_redraw = false;

        Ok(())
    }

    fn prepare_frame(&mut self, time: f32) {
        self.current_demo.prepare_frame(&self.queue);
        self.update_demo_uniforms(time);
        self.update_overlay();
    }

    fn build_frame_packet(&self, time: f32) -> FramePacket {
        let mut packet = self.current_demo.build_frame_packet(time);
        packet.push_overlay(OverlayPacket::new("Overlay"));
        packet.push_present(PresentPacket::new());
        packet
    }

    fn collect_frame_targets(graph: &CompiledFrameGraph) -> Vec<Cow<'static, str>> {
        let mut names = Vec::new();
        for pass in graph.passes() {
            let target = match pass {
                CompiledFramePass::WorldView(packet) => Some(&packet.target),
                CompiledFramePass::UiLayer(packet) => Some(&packet.target),
                CompiledFramePass::Effect(packet) => Some(&packet.target),
                CompiledFramePass::Overlay(packet) => Some(&packet.target),
                CompiledFramePass::Present(_) => None,
            };
            if let Some(FrameTarget::Offscreen(name)) = target {
                if !names.iter().any(|existing| existing == name) {
                    names.push(name.clone());
                }
            }
            if let CompiledFramePass::Effect(packet) = pass {
                if let FrameTarget::Offscreen(name) = &packet.source {
                    if !names.iter().any(|existing| existing == name) {
                        names.push(name.clone());
                    }
                }
            }
        }
        names
    }

    fn create_frame_target_texture(&self, label: &str) -> OffscreenTarget {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: self.config.width.max(1),
                height: self.config.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: PRESENT_INTERMEDIATE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        OffscreenTarget {
            _texture: texture,
            view,
        }
    }

    fn ensure_frame_targets(&mut self, graph: &CompiledFrameGraph) {
        let required_targets = Self::collect_frame_targets(graph);
        self.offscreen_targets
            .retain(|name, _| required_targets.iter().any(|required| required == name));
        for name in required_targets {
            if !self.offscreen_targets.contains_key(&name) {
                let target =
                    self.create_frame_target_texture(&format!("Frame Offscreen Target {name}"));
                self.offscreen_targets.insert(name, target);
            }
        }
    }

    fn target_view(&self, target: &FrameTarget) -> Option<&wgpu::TextureView> {
        match target {
            FrameTarget::SceneColor => Some(self.present_host.scene_view()),
            FrameTarget::Offscreen(name) => {
                self.offscreen_targets.get(name).map(|target| &target.view)
            }
        }
    }

    fn source_view(&self, source: &FrameTarget) -> Option<&wgpu::TextureView> {
        self.target_view(source)
    }

    fn target_size(&self, target: &FrameTarget) -> [u32; 2] {
        match target {
            FrameTarget::SceneColor => self.present_host.size(),
            FrameTarget::Offscreen(_) => [self.config.width.max(1), self.config.height.max(1)],
        }
    }

    fn execute_frame_graph(
        &mut self,
        graph: &CompiledFrameGraph,
        encoder: &mut wgpu::CommandEncoder,
        output_view: Option<&wgpu::TextureView>,
        time: f32,
    ) {
        self.ensure_frame_targets(graph);
        for pass in graph.passes() {
            match pass {
                CompiledFramePass::WorldView(packet) => {
                    self.encode_world_view_pass(encoder, packet, time)
                }
                CompiledFramePass::UiLayer(packet) => {
                    self.encode_ui_layer_pass(encoder, packet, time)
                }
                CompiledFramePass::Effect(packet) => self.encode_effect_pass(encoder, packet),
                CompiledFramePass::Overlay(packet) => self.encode_overlay_packet(encoder, packet),
                CompiledFramePass::Present(_packet) => {
                    if let Some(output_view) = output_view {
                        self.present_host.encode_present_pass(encoder, output_view);
                    }
                }
            }
        }
    }

    fn encode_world_view_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        packet: &WorldViewPacket,
        time: f32,
    ) {
        let Some(view) = self.target_view(&packet.target) else {
            return;
        };
        self.encode_demo_scene_pass(
            encoder,
            view,
            packet.label.as_ref(),
            packet.composite_mode.load_op(packet.clear_color),
            time,
            |demo, render_pass, queue, time| {
                demo.render_world_view(packet, render_pass, queue, time);
            },
        );
    }

    fn encode_ui_layer_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        packet: &UiLayerPacket,
        time: f32,
    ) {
        let Some(view) = self.target_view(&packet.target) else {
            return;
        };
        self.encode_demo_scene_pass(
            encoder,
            view,
            packet.label.as_ref(),
            packet.composite_mode.load_op(packet.clear_color),
            time,
            |demo, render_pass, queue, time| {
                demo.render_ui_layer(packet, render_pass, queue, time);
            },
        );
    }

    fn encode_effect_pass(&self, encoder: &mut wgpu::CommandEncoder, packet: &EffectPacket) {
        let Some(source_view) = self.source_view(&packet.source) else {
            return;
        };
        let Some(target_view) = self.target_view(&packet.target) else {
            return;
        };
        self.composite_host.encode_pass(
            &self.device,
            &self.queue,
            encoder,
            packet.label.as_ref(),
            source_view,
            target_view,
            packet.composite_mode.load_op(packet.clear_color),
            self.target_size(&packet.target),
            packet.source_rect.as_vec4(),
            packet.target_rect.as_vec4(),
        );
    }

    fn encode_demo_scene_pass<F>(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        label: &str,
        load: wgpu::LoadOp<wgpu::Color>,
        time: f32,
        render_fn: F,
    ) where
        F: for<'a> FnOnce(&'a dyn Demo, &mut wgpu::RenderPass<'a>, &wgpu::Queue, f32),
    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_fn(
            self.current_demo.as_ref(),
            &mut render_pass,
            &self.queue,
            time,
        );
    }

    fn encode_overlay_packet(&self, encoder: &mut wgpu::CommandEncoder, packet: &OverlayPacket) {
        let Some(view) = self.target_view(&packet.target) else {
            return;
        };
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(packet.label.as_ref()),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
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

    fn update_demo_uniforms(&self, time: f32) {
        self.current_demo
            .update_camera_uniforms(&self.queue, &self.camera, time);
    }

    fn update_overlay(&mut self) {
        let mut stats = self.input.format_stats();
        if !stats.is_empty() {
            stats = format!("{} | {}x{}", stats, self.config.width, self.config.height);
        }
        let keybindings = if self.show_keybindings {
            let demo_bindings = self.current_demo.keybindings();
            let mut all_bindings =
                Vec::with_capacity(demo_bindings.len() + super::KEYBINDINGS_COMMON.len());
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
            self.sync_demo_scale_factor();
            self.overlay.resize(new_size.width, new_size.height);
            self.present_host
                .resize(&self.device, new_size.width, new_size.height, "Native");
            self.present_host.update_surface_format(
                &self.device,
                &self.queue,
                self.config.format,
                "Native",
            );
            self.offscreen_targets.clear();
            self.mark_needs_redraw();
        }
    }

    fn sync_demo_scale_factor(&mut self) {
        self.current_demo
            .set_window_scale_factor(self.window.scale_factor() as f32);
    }

    fn update_window_title(&self) {
        let title = format!(
            "Demo {}: {} | K: show keys",
            self.current_demo.id() as u8,
            self.current_demo.name()
        );
        self.window.set_title(&title);
    }

    fn handle_key_pressed(&mut self, code: KeyCode, _event_loop: &ActiveEventLoop) {
        // Number keys for demo switching
        let switched = match code {
            KeyCode::Digit0 => {
                let _ = self.switch_demo(DemoId::Empty);
                true
            }
            KeyCode::Digit1 => {
                let _ = self.switch_demo(DemoId::Objects);
                true
            }
            KeyCode::Digit2 => {
                let _ = self.switch_demo(DemoId::Spheres);
                true
            }
            KeyCode::Digit3 => {
                let _ = self.switch_demo(DemoId::Towers);
                true
            }
            KeyCode::Digit4 => {
                let _ = self.switch_demo(DemoId::Text2D);
                true
            }
            KeyCode::Digit5 => {
                let _ = self.switch_demo(DemoId::Clay);
                true
            }
            KeyCode::Digit6 => {
                let _ = self.switch_demo(DemoId::TextShadow);
                true
            }
            KeyCode::Digit7 => {
                let _ = self.switch_demo(DemoId::TodoMvc);
                true
            }
            KeyCode::Digit8 => {
                let _ = self.switch_demo(DemoId::TodoMvc3D);
                true
            }
            KeyCode::Digit9 => {
                let _ = self.switch_demo(DemoId::RetainedUi);
                true
            }
            KeyCode::Minus => {
                let _ = self.switch_demo(DemoId::RetainedUiPhysical);
                true
            }
            KeyCode::Equal => {
                let _ = self.switch_demo(DemoId::TextPhysical);
                true
            }
            KeyCode::KeyV => {
                let _ = self.switch_demo(DemoId::MixedUiWorld);
                true
            }
            // Escape handled at top of window_event before keyboard_paused check
            _ => false,
        };

        if switched {
            return;
        }

        let _ = self.current_demo.handle_key_pressed(code);

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
    #[cfg(feature = "control")]
    control_state: Option<SharedControlState>,
}

impl DemoApp {
    pub fn new(initial_demo: DemoId) -> Self {
        Self {
            runner: None,
            initial_demo,
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

impl ApplicationHandler<DemoUserEvent> for DemoApp {
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
                    runner.mark_needs_redraw();
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
        if let Some(runner) = self.runner.as_mut() {
            runner.handle_continuous_redraw_transition();

            #[cfg(feature = "control")]
            let has_pending_control = runner.has_pending_control_commands();
            #[cfg(not(feature = "control"))]
            let has_pending_control = false;

            let should_draw = runner.needs_redraw
                || has_pending_control
                || runner.continuous_redraw_active()
                || runner.demo_needs_redraw();

            if should_draw {
                runner.window.request_redraw();
            }

            let control_flow = if runner.continuous_redraw_active() {
                ControlFlow::WaitUntil(Instant::now() + ACTIVE_FRAME_TIME)
            } else {
                ControlFlow::Wait
            };

            event_loop.set_control_flow(control_flow);
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

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: DemoUserEvent) {
        let Some(runner) = self.runner.as_mut() else {
            return;
        };

        match event {
            DemoUserEvent::Wake => {
                runner.mark_needs_redraw();
                runner.window.request_redraw();
            }
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if let Some(runner) = self.runner.as_mut() {
            if runner.current_demo.demo_type().uses_camera_controls() {
                if let DeviceEvent::MouseMotion { delta } = event {
                    runner.input.handle_mouse_motion(&mut runner.camera, delta);
                    if runner.input.mouse_captured {
                        runner.mark_needs_redraw();
                    }
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(runner) = self.runner.as_mut() else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    // Tab pauses keyboard input (always active, even when paused)
                    if code == KeyCode::Tab && event.state == ElementState::Pressed {
                        runner.keyboard_paused = !runner.keyboard_paused;
                        if runner.keyboard_paused {
                            runner.input.release_capture(&runner.window);
                            runner.pressed_keys.clear();
                            runner.input.pressed_keys.clear();
                        }
                        runner.mark_needs_redraw();
                        return;
                    }

                    // Escape exits when paused (capture already released)
                    if runner.keyboard_paused {
                        if code == KeyCode::Escape && event.state == ElementState::Pressed {
                            event_loop.exit();
                        }
                        return;
                    }

                    match event.state {
                        ElementState::Pressed => {
                            // Handle input actions for 3D demos
                            if runner.current_demo.demo_type().uses_camera_controls() {
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
                                        runner.current_demo.reset_2d_rotation();
                                    }
                                    KeyCode::KeyT => {
                                        runner.current_demo.reset_2d_all();
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
                            runner.mark_needs_redraw();
                        }
                        ElementState::Released => {
                            if runner.current_demo.demo_type().uses_camera_controls() {
                                runner.input.handle_key(event);
                            }
                            runner.handle_key_released(code);
                            runner.mark_needs_redraw();
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if runner.current_demo.demo_type().uses_camera_controls() {
                    runner.input.handle_scroll(&mut runner.camera, delta);
                    runner.mark_needs_redraw();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                runner.keyboard_paused = false;
                if runner.current_demo.demo_type().uses_camera_controls() {
                    runner.input.capture(&runner.window);
                }
                runner.mark_needs_redraw();
            }
            WindowEvent::Resized(size) => runner.resize(size),
            WindowEvent::ScaleFactorChanged { .. } => {
                runner.sync_demo_scale_factor();
                runner.mark_needs_redraw();
            }
            WindowEvent::RedrawRequested => {
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
    let event_loop = EventLoop::<DemoUserEvent>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = DemoApp::new(initial_demo);
    event_loop.run_app(&mut app)?;
    Ok(())
}

/// Run the demo application with control server enabled
#[cfg(feature = "control")]
pub fn run_with_control(initial_demo: DemoId, port: Option<u16>) -> Result<()> {
    use crate::control::{WsServer, DEFAULT_WS_PORT};

    let port = port.unwrap_or(DEFAULT_WS_PORT);

    let event_loop = EventLoop::<DemoUserEvent>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    let command_waker: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        let _ = proxy.send_event(DemoUserEvent::Wake);
    });

    // Create WebSocket server
    let ws_server = WsServer::with_command_waker(command_waker);
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

    let mut app = DemoApp::new(initial_demo).with_control(control_state);
    event_loop.run_app(&mut app)?;
    Ok(())
}
