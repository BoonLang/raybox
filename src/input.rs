//! Shared input handling for windowed examples
//!
//! Standardized controls:
//! - WASD: Movement (forward/back/strafe)
//! - Space: Move up (camera-relative)
//! - Ctrl: Move down (camera-relative)
//! - Q/E: Roll camera
//! - R: Reset roll to horizontal
//! - T: Reset camera to initial position
//! - Tab: Toggle mouse capture
//! - Scroll: Adjust movement speed
//! - F: Toggle app stats overlay (FPS, CPU, RAM, GPU, VRAM - app only)
//! - G: Toggle full stats overlay (app + system values)
//! - Esc: Release capture (if captured), else exit

use crate::camera::FlyCamera;
use glam::Vec3;
use std::collections::HashSet;
use std::collections::VecDeque;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use winit::{
    event::{ElementState, KeyEvent, MouseScrollDelta},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window},
};

/// Camera configuration for initial position and orientation
#[derive(Clone, Debug)]
pub struct CameraConfig {
    pub initial_position: Vec3,
    pub look_at_target: Vec3,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            initial_position: Vec3::new(0.0, 0.0, 4.0),
            look_at_target: Vec3::ZERO,
        }
    }
}

/// Debug overlay display mode
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverlayMode {
    #[default]
    Off,
    App,  // App-only stats (F key)
    Full, // App + system stats (G key)
}

/// Actions that require special handling from the application
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputAction {
    Exit,
    ToggleCapture,
    ToggleOverlayApp,
    ToggleOverlayFull,
    ResetRoll,
    ResetCamera,
}

/// System resource monitoring
pub struct SystemMonitor {
    system: System,
    pid: Pid,
    os_pid: u32,
    cpu_usage_app: f32,                 // This app's CPU usage %
    cpu_usage_system: f32,              // System-wide CPU usage %
    ram_usage_app_mb: f32,              // This app's RAM usage
    ram_usage_system_mb: u64,           // System-wide RAM usage
    ram_total_mb: u64,                  // Total system RAM
    gpu_usage_app: Option<u32>,         // This app's GPU SM utilization %
    gpu_usage_system: Option<u32>,      // System-wide GPU utilization %
    vram_usage_app_mb: Option<u64>,     // This app's VRAM usage
    vram_usage_system_mb: Option<u64>,  // System-wide VRAM usage
    vram_total_mb: Option<u64>,         // Total VRAM available
    #[cfg(feature = "windowed")]
    nvml: Option<nvml_wrapper::Nvml>,
    #[cfg(feature = "windowed")]
    nvml_device_index: u32,
    last_update: std::time::Instant,
}

impl SystemMonitor {
    pub fn new() -> Self {
        // Use new_all() to get initial baseline data
        let mut system = System::new_all();
        let os_pid = std::process::id();
        let pid = Pid::from_u32(os_pid);

        // Initial CPU refresh to establish baseline
        system.refresh_cpu_usage();

        // Wait minimum interval then refresh again for accurate CPU measurement
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);

        // Refresh both process and system CPU for accurate readings
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        system.refresh_cpu_usage();

        // Try to initialize NVML for NVIDIA GPU monitoring
        #[cfg(feature = "windowed")]
        let (nvml, nvml_device_index) = Self::init_nvml();

        let ram_total_mb = system.total_memory() / (1024 * 1024);

        Self {
            system,
            pid,
            os_pid,
            cpu_usage_app: 0.0,
            cpu_usage_system: 0.0,
            ram_usage_app_mb: 0.0,
            ram_usage_system_mb: 0,
            ram_total_mb,
            gpu_usage_app: None,
            gpu_usage_system: None,
            vram_usage_app_mb: None,
            vram_usage_system_mb: None,
            vram_total_mb: None,
            #[cfg(feature = "windowed")]
            nvml,
            #[cfg(feature = "windowed")]
            nvml_device_index,
            last_update: std::time::Instant::now(),
        }
    }

    #[cfg(feature = "windowed")]
    fn init_nvml() -> (Option<nvml_wrapper::Nvml>, u32) {
        match nvml_wrapper::Nvml::init() {
            Ok(nvml) => {
                // Use device 0 by default
                (Some(nvml), 0)
            }
            Err(_) => (None, 0),
        }
    }

    /// Update system stats (call periodically, not every frame)
    pub fn update(&mut self) {
        // Only update at minimum CPU update interval to get accurate readings
        let min_interval = sysinfo::MINIMUM_CPU_UPDATE_INTERVAL.as_millis() as u128;
        if self.last_update.elapsed().as_millis() < min_interval.max(200) {
            return;
        }
        self.last_update = std::time::Instant::now();

        // Refresh process info with CPU and memory
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[self.pid]),
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );

        // Refresh system-wide CPU usage
        self.system.refresh_cpu_usage();

        if let Some(process) = self.system.process(self.pid) {
            self.cpu_usage_app = process.cpu_usage();
            self.ram_usage_app_mb = process.memory() as f32 / (1024.0 * 1024.0);
        }

        // System-wide CPU usage
        self.cpu_usage_system = self.system.global_cpu_usage();

        // System-wide RAM usage
        self.system.refresh_memory();
        self.ram_usage_system_mb = self.system.used_memory() / (1024 * 1024);

        // Update GPU stats if NVML is available
        #[cfg(feature = "windowed")]
        if let Some(ref nvml) = self.nvml {
            if let Ok(device) = nvml.device_by_index(self.nvml_device_index) {
                // System-wide GPU utilization
                if let Ok(util) = device.utilization_rates() {
                    self.gpu_usage_system = Some(util.gpu);
                }

                // Per-process GPU utilization (SM usage)
                self.gpu_usage_app = None;
                if let Ok(samples) = device.process_utilization_stats(None) {
                    for sample in samples {
                        if sample.pid == self.os_pid {
                            self.gpu_usage_app = Some(sample.sm_util);
                            break;
                        }
                    }
                }

                // System VRAM usage and total
                if let Ok(mem) = device.memory_info() {
                    self.vram_usage_system_mb = Some(mem.used / (1024 * 1024));
                    self.vram_total_mb = Some(mem.total / (1024 * 1024));
                }

                // Per-process VRAM usage for this app
                self.vram_usage_app_mb = None;
                if let Ok(processes) = device.running_graphics_processes() {
                    for proc in processes {
                        if proc.pid == self.os_pid {
                            if let nvml_wrapper::enums::device::UsedGpuMemory::Used(bytes) = proc.used_gpu_memory {
                                self.vram_usage_app_mb = Some(bytes / (1024 * 1024));
                            }
                            break;
                        }
                    }
                }
                // Also check compute processes if not found in graphics
                if self.vram_usage_app_mb.is_none() {
                    if let Ok(processes) = device.running_compute_processes() {
                        for proc in processes {
                            if proc.pid == self.os_pid {
                                if let nvml_wrapper::enums::device::UsedGpuMemory::Used(bytes) = proc.used_gpu_memory {
                                    self.vram_usage_app_mb = Some(bytes / (1024 * 1024));
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Format stats for display based on overlay mode
    pub fn format_stats(&self, mode: OverlayMode) -> String {
        match mode {
            OverlayMode::Off => String::new(),
            OverlayMode::App => self.format_app_stats(),
            OverlayMode::Full => self.format_full_stats(),
        }
    }

    /// App-only stats (F key)
    fn format_app_stats(&self) -> String {
        let mut parts = vec![
            format!("CPU: {:.0}%", self.cpu_usage_app),
            format!("RAM: {:.0}MB", self.ram_usage_app_mb),
        ];

        // GPU: app only, or system if app not available
        if let Some(gpu) = self.gpu_usage_app {
            parts.push(format!("GPU: {}%", gpu));
        } else if let Some(gpu) = self.gpu_usage_system {
            parts.push(format!("GPU: {}%", gpu));
        }

        // VRAM: app only
        if let Some(vram) = self.vram_usage_app_mb {
            parts.push(format!("VRAM: {}MB", vram));
        }

        parts.join(" | ")
    }

    /// Full stats with system values (G key)
    /// Format: CPU: app%(sys%) | RAM: app(sys/total)MB | GPU: app%(sys%) | VRAM: app(sys/total)MB
    fn format_full_stats(&self) -> String {
        let mut parts = vec![];

        // CPU: app%(system%)
        parts.push(format!("CPU: {:.0}%({:.0}%)", self.cpu_usage_app, self.cpu_usage_system));

        // RAM: app(system/total)MB
        parts.push(format!("RAM: {:.0}({}/{}MB)", self.ram_usage_app_mb, self.ram_usage_system_mb, self.ram_total_mb));

        // GPU: app%(system%)
        let app_gpu = self.gpu_usage_app.map(|v| format!("{}%", v)).unwrap_or("-".into());
        let sys_gpu = self.gpu_usage_system.map(|v| format!("{}%", v)).unwrap_or("-".into());
        parts.push(format!("GPU: {}({})", app_gpu, sys_gpu));

        // VRAM: app(system/total)MB
        let app_vram = self.vram_usage_app_mb.map(|v| v.to_string()).unwrap_or("-".into());
        let sys_vram = self.vram_usage_system_mb.map(|v| v.to_string()).unwrap_or("-".into());
        let total_vram = self.vram_total_mb.map(|v| v.to_string()).unwrap_or("-".into());
        parts.push(format!("VRAM: {}({}/{}MB)", app_vram, sys_vram, total_vram));

        parts.join(" | ")
    }
}

impl Default for SystemMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared input handler for all 3D examples
pub struct InputHandler {
    pub pressed_keys: HashSet<KeyCode>,
    pub mouse_captured: bool,
    pub overlay_mode: OverlayMode,
    config: CameraConfig,
    frame_times: VecDeque<f32>,
    last_fps: f32,
    system_monitor: SystemMonitor,
}

impl InputHandler {
    /// Create a new input handler with the given camera configuration
    pub fn new(config: CameraConfig) -> Self {
        Self {
            pressed_keys: HashSet::new(),
            mouse_captured: false,
            overlay_mode: OverlayMode::Off,
            config,
            frame_times: VecDeque::with_capacity(60),
            last_fps: 0.0,
            system_monitor: SystemMonitor::new(),
        }
    }

    /// Initialize camera with the configured position and orientation
    pub fn setup_camera(&self, camera: &mut FlyCamera) {
        camera.position = self.config.initial_position;
        camera.look_at(self.config.look_at_target);
    }

    /// Handle a key event, returning an action if special handling is needed
    pub fn handle_key(&mut self, event: KeyEvent) -> Option<InputAction> {
        let PhysicalKey::Code(key_code) = event.physical_key else {
            return None;
        };

        match event.state {
            ElementState::Pressed => {
                self.pressed_keys.insert(key_code);
            }
            ElementState::Released => {
                self.pressed_keys.remove(&key_code);
                return None; // Only handle press actions
            }
        }

        // Handle special keys on press
        match key_code {
            KeyCode::Escape => {
                if self.mouse_captured {
                    Some(InputAction::ToggleCapture)
                } else {
                    Some(InputAction::Exit)
                }
            }
            KeyCode::Tab => Some(InputAction::ToggleCapture),
            KeyCode::KeyF => Some(InputAction::ToggleOverlayApp),
            KeyCode::KeyG => Some(InputAction::ToggleOverlayFull),
            KeyCode::KeyR => Some(InputAction::ResetRoll),
            KeyCode::KeyT | KeyCode::Home => Some(InputAction::ResetCamera),
            _ => None,
        }
    }

    /// Handle mouse motion when captured
    pub fn handle_mouse_motion(&self, camera: &mut FlyCamera, delta: (f64, f64)) {
        if self.mouse_captured {
            camera.look(delta.0 as f32, delta.1 as f32);
        }
    }

    /// Handle scroll wheel for speed adjustment
    pub fn handle_scroll(&self, camera: &mut FlyCamera, delta: MouseScrollDelta) {
        let scroll = match delta {
            MouseScrollDelta::LineDelta(_, y) => y,
            MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.02,
        };
        camera.adjust_speed(scroll);
    }

    /// Update camera based on currently pressed keys
    pub fn update_camera(&self, camera: &mut FlyCamera, dt: f32) {
        // WASD movement
        if self.pressed_keys.contains(&KeyCode::KeyW) {
            camera.move_forward(dt, true);
        }
        if self.pressed_keys.contains(&KeyCode::KeyS) {
            camera.move_forward(dt, false);
        }
        if self.pressed_keys.contains(&KeyCode::KeyA) {
            camera.move_right(dt, false);
        }
        if self.pressed_keys.contains(&KeyCode::KeyD) {
            camera.move_right(dt, true);
        }

        // Up/Down movement (Space/Ctrl)
        if self.pressed_keys.contains(&KeyCode::Space) {
            camera.move_up(dt, true);
        }
        if self.pressed_keys.contains(&KeyCode::ControlLeft)
            || self.pressed_keys.contains(&KeyCode::ControlRight)
        {
            camera.move_up(dt, false);
        }

        // Roll (Q/E)
        if self.pressed_keys.contains(&KeyCode::KeyQ) {
            camera.roll_camera(-dt * 2.0);
        }
        if self.pressed_keys.contains(&KeyCode::KeyE) {
            camera.roll_camera(dt * 2.0);
        }
    }

    /// Reset roll to horizontal
    pub fn reset_roll(&self, camera: &mut FlyCamera) {
        camera.reset_roll();
    }

    /// Reset camera to initial configuration
    pub fn reset_camera(&self, camera: &mut FlyCamera) {
        camera.reset();
        camera.position = self.config.initial_position;
        camera.look_at(self.config.look_at_target);
    }

    /// Toggle mouse capture state
    pub fn toggle_capture(&mut self, window: &Window) {
        if self.mouse_captured {
            self.release_capture(window);
        } else {
            self.capture(window);
        }
    }

    /// Capture mouse (for mouse click into window)
    pub fn capture(&mut self, window: &Window) {
        if !self.mouse_captured {
            self.mouse_captured = true;
            // Try Locked first (best for FPS), fall back to Confined
            if window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                let _ = window.set_cursor_grab(CursorGrabMode::Confined);
            }
            window.set_cursor_visible(false);
        }
    }

    /// Release mouse capture
    pub fn release_capture(&mut self, window: &Window) {
        if self.mouse_captured {
            self.mouse_captured = false;
            let _ = window.set_cursor_grab(CursorGrabMode::None);
            window.set_cursor_visible(true);
        }
    }

    /// Toggle app-only overlay (F key) - toggles between Off and App mode
    pub fn toggle_overlay_app(&mut self) {
        self.overlay_mode = match self.overlay_mode {
            OverlayMode::App => OverlayMode::Off,
            _ => OverlayMode::App,
        };
    }

    /// Toggle full overlay (G key) - toggles between Off and Full mode
    pub fn toggle_overlay_full(&mut self) {
        self.overlay_mode = match self.overlay_mode {
            OverlayMode::Full => OverlayMode::Off,
            _ => OverlayMode::Full,
        };
    }

    /// Update frame timing for FPS calculation
    pub fn update_frame_time(&mut self, dt: f32) {
        self.frame_times.push_back(dt);
        if self.frame_times.len() > 60 {
            self.frame_times.pop_front();
        }

        // Calculate FPS from average frame time
        if !self.frame_times.is_empty() {
            let avg_dt: f32 = self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32;
            self.last_fps = 1.0 / avg_dt;
        }

        // Update system stats
        self.system_monitor.update();
    }

    /// Update window title with debug info based on overlay mode
    pub fn update_window_title(&self, window: &Window, base_title: &str, camera: &FlyCamera) {
        match self.overlay_mode {
            OverlayMode::Off => {
                window.set_title(base_title);
            }
            mode => {
                let pos = camera.position;
                let yaw_deg = camera.get_yaw().to_degrees();
                let pitch_deg = camera.get_pitch().to_degrees();
                let sys_stats = self.system_monitor.format_stats(mode);
                let title = format!(
                    "{} | FPS: {:.0} | {} | Pos: ({:.1}, {:.1}, {:.1}) | Yaw: {:.0} Pitch: {:.0} | Speed: {:.1}",
                    base_title, self.last_fps, sys_stats, pos.x, pos.y, pos.z, yaw_deg, pitch_deg, camera.move_speed
                );
                window.set_title(&title);
            }
        }
    }
}

/// Standard window title suffix for all 3D examples
pub const CONTROLS_HINT: &str = "WASD+Space/Ctrl, Q/E, Mouse, Tab, Scroll, R/T, F/G, Esc";

/// Generate a standard window title for a demo
pub fn demo_title(demo_num: u8, name: &str) -> String {
    format!("Demo {}: {} | {}", demo_num, name, CONTROLS_HINT)
}
