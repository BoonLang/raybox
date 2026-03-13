//! Demo 8: TodoMVC 3D - Physical 3D rendering of TodoMVC UI
//!
//! A 3D card with extruded/carved text, PBR materials, and 4 switchable themes
//! (Professional, Neobrutalism, Glassmorphism, Neumorphism) with light/dark mode.
//!
//! Non-pixel-perfect AI-generated theme references (for visual direction only):
//! - assets/todomvc_3d/reference_professional.jpg
//! - assets/todomvc_3d/reference_neobrutalism.jpg
//! - assets/todomvc_3d/reference_glassmorphism.jpg
//! - assets/todomvc_3d/reference_neumorphism.jpg

use super::{
    todomvc_common::{create_todomvc_ui_physical_deck, CLASSIC_DECAL_PRIM_START},
    ui_physical_runtime::{StateBackedUiPhysicalSceneDeck, ThemedUiPhysicalHost},
    Demo, DemoContext, DemoId, DemoType, ListCommandTarget, NamedScrollTarget,
};
use crate::camera::FlyCamera;
use crate::demo_core::UiPhysicalCameraPreset;
use crate::demos::todomvc_common::TodoMvcRetainedState;
use crate::input::CameraConfig;
use anyhow::Result;

pub use super::ui_physical_theme::{ThemeId, UiPhysicalThemeState, PHYSICAL_THEME_OPTIONS};

// ---- Keybindings ----

const KEYBINDINGS_TODOMVC_3D: &[(&str, &str)] = &[
    ("WASD", "Move"),
    ("Mouse", "Look"),
    ("Space/Ctrl", "Up/Down"),
    ("Scroll", "Speed"),
    ("R", "Reset roll"),
    ("T", "Reset camera"),
    ("Tab", "Capture mouse"),
    ("N", "Cycle theme"),
    ("M", "Toggle dark mode"),
];

// ---- Demo struct ----

pub struct TodoMvc3DDemo {
    host: ThemedUiPhysicalHost<StateBackedUiPhysicalSceneDeck<TodoMvcRetainedState>>,
}

impl TodoMvc3DDemo {
    pub fn new(ctx: &DemoContext) -> Result<Self> {
        let current_theme = ThemeId::Classic2D;
        let dark_mode = false;
        let theme_state = UiPhysicalThemeState::new(current_theme, dark_mode);
        let colors = theme_state.text_colors();
        let deck =
            create_todomvc_ui_physical_deck(ctx, &colors, "TodoMVC 3D UI Primitives Buffer")?;
        let theme_uniforms = theme_state.theme_uniforms();
        let host = ThemedUiPhysicalHost::new(
            ctx,
            "TodoMVC 3D",
            deck,
            theme_state,
            CLASSIC_DECAL_PRIM_START as f32,
            &theme_uniforms,
        );

        Ok(Self { host })
    }
}

impl Demo for TodoMvc3DDemo {
    fn name(&self) -> &'static str {
        "TodoMVC 3D"
    }

    fn id(&self) -> DemoId {
        DemoId::TodoMvc3D
    }

    fn demo_type(&self) -> DemoType {
        DemoType::UiPhysical
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        KEYBINDINGS_TODOMVC_3D
    }

    fn camera_config(&self) -> CameraConfig {
        CameraConfig {
            initial_position: glam::Vec3::new(0.0, 0.0, 8.5),
            look_at_target: glam::Vec3::new(0.0, 0.0, 0.0),
        }
    }

    fn ui_physical_camera_preset(&self) -> Option<UiPhysicalCameraPreset> {
        Some(UiPhysicalCameraPreset {
            fallback_offset: glam::Vec3::new(0.0, 0.0, 8.5),
            min_distance: 4.0,
            max_distance: 14.0,
            min_elevation: -1.0,
            max_elevation: 1.0,
            clamp_x: 7.0,
            min_height: 1.5,
            max_height: 11.0,
            clamp_z: 10.0,
        })
    }

    fn update(&mut self, _dt: f32, _camera: &mut FlyCamera) {
        // No per-frame updates needed
    }

    fn prepare_frame(&mut self, queue: &wgpu::Queue) {
        self.host.prepare_frame(queue);
    }

    fn needs_redraw(&self) -> bool {
        self.host.needs_redraw()
    }

    fn update_camera_uniforms(&self, queue: &wgpu::Queue, camera: &FlyCamera, time: f32) {
        self.host.update_uniforms(queue, camera, time);
    }

    fn set_window_scale_factor(&mut self, scale_factor: f32) {
        self.host.set_scale_factor(scale_factor);
    }

    fn set_named_theme(
        &mut self,
        theme: &str,
        dark_mode: Option<bool>,
    ) -> Option<(&'static str, bool)> {
        self.host.set_named_theme(theme, dark_mode)
    }

    fn named_theme_options(&self) -> &'static [&'static str] {
        PHYSICAL_THEME_OPTIONS
    }

    fn list_command_target(&mut self) -> Option<&mut dyn ListCommandTarget> {
        Some(&mut self.host)
    }

    fn has_list_command_target(&self) -> bool {
        true
    }

    fn named_scroll_target(&mut self) -> Option<&mut dyn NamedScrollTarget> {
        Some(&mut self.host)
    }

    fn has_named_scroll_target(&self) -> bool {
        true
    }

    fn handle_key_pressed(&mut self, code: winit::keyboard::KeyCode) -> bool {
        match code {
            winit::keyboard::KeyCode::KeyN => {
                self.host.cycle_theme();
                true
            }
            winit::keyboard::KeyCode::KeyM => {
                self.host.toggle_dark_mode();
                true
            }
            _ => false,
        }
    }

    fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        _queue: &wgpu::Queue,
        _time: f32,
    ) {
        self.host.render(render_pass);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.host.resize(width, height);
    }
}
