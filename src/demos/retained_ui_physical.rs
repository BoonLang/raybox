use super::{
    ui2d_runtime::load_dejavu_font_atlas,
    ui_physical_runtime::{
        create_showcase_ui_physical_host, ShowcaseUiPhysicalDeck, ThemedUiPhysicalHost,
        UiPhysicalHostConfig,
    },
    ui_physical_theme::{ThemeId, UiPhysicalThemeState, PHYSICAL_THEME_OPTIONS},
    Demo, DemoContext, DemoId, DemoType, NamedScrollTarget,
};
use crate::camera::FlyCamera;
use crate::demo_core::{
    ui_physical_card_camera_config, ui_physical_card_camera_preset, UiPhysicalCameraPreset,
};
use crate::input::CameraConfig;
use crate::retained::showcase::ShowcaseSceneDeckTarget;
use crate::retained::text::TextRenderSpace;
use crate::retained::ui::UiRenderSpace;
use anyhow::Result;
use winit::keyboard::KeyCode;

const FEED_SCROLL_STEP: f32 = 24.0;
const SHOWCASE_PHYSICAL_CARD_SIZE: [f32; 2] = [392.0, 224.0];
const PHYSICAL_SAMPLE_KEYBINDINGS: &[(&str, &str)] = &[
    ("WASD", "Inspect move"),
    ("Mouse", "Orbit"),
    ("Space/Ctrl", "Lift/lower"),
    ("Scroll", "Zoom"),
    ("R", "Reset roll"),
    ("T", "Reset camera"),
    ("Tab", "Capture mouse"),
    ("Y", "Switch retained sample"),
    ("O", "Toggle active scene state"),
    ("U/J", "Scroll active retained scene"),
    ("N", "Cycle theme"),
    ("M", "Toggle dark mode"),
];

fn text_render_space(screen_height: f32) -> TextRenderSpace {
    TextRenderSpace {
        x_offset: 0.0,
        screen_height,
        italic_codepoint_offset: 0x10000,
    }
}

fn ui_render_space(screen_height: f32) -> UiRenderSpace {
    UiRenderSpace {
        x_offset: 0.0,
        screen_height,
    }
}

pub struct RetainedUiPhysicalDemo {
    host: ThemedUiPhysicalHost<ShowcaseUiPhysicalDeck>,
}

impl RetainedUiPhysicalDemo {
    pub fn new(ctx: &DemoContext) -> Result<Self> {
        let current_theme = ThemeId::Professional;
        let dark_mode = true;
        let theme_state = UiPhysicalThemeState::new(current_theme, dark_mode);
        let atlas = load_dejavu_font_atlas()?;
        let config = UiPhysicalHostConfig {
            label: "Retained UI Physical Buffer".to_string(),
            max_char_instances: 512,
            max_ui_primitives: 256,
            max_grid_indices: 8192,
            grid_cell_capacity: 8,
        };
        let host = create_showcase_ui_physical_host(
            ctx,
            "Retained UI Physical",
            atlas,
            theme_state,
            text_render_space(ctx.height as f32),
            ui_render_space(ctx.height as f32),
            config,
            0.0,
        );

        Ok(Self { host })
    }
}

impl Demo for RetainedUiPhysicalDemo {
    fn name(&self) -> &'static str {
        "Retained UI Physical"
    }

    fn id(&self) -> DemoId {
        DemoId::RetainedUiPhysical
    }

    fn demo_type(&self) -> DemoType {
        DemoType::UiPhysical
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        PHYSICAL_SAMPLE_KEYBINDINGS
    }

    fn camera_config(&self) -> CameraConfig {
        ui_physical_card_camera_config(SHOWCASE_PHYSICAL_CARD_SIZE)
    }

    fn ui_physical_camera_preset(&self) -> Option<UiPhysicalCameraPreset> {
        Some(ui_physical_card_camera_preset(SHOWCASE_PHYSICAL_CARD_SIZE))
    }

    fn update(&mut self, _dt: f32, _camera: &mut FlyCamera) {}

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

    fn named_scroll_target(&mut self) -> Option<&mut dyn NamedScrollTarget> {
        Some(&mut self.host)
    }

    fn has_named_scroll_target(&self) -> bool {
        true
    }

    fn handle_key_pressed(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::KeyY => {
                let _ = self.host.cycle_showcase_scene();
                true
            }
            KeyCode::KeyO => self.host.toggle_active_showcase_state(),
            KeyCode::KeyU => self.host.adjust_active_showcase_scroll(-FEED_SCROLL_STEP),
            KeyCode::KeyJ => self.host.adjust_active_showcase_scroll(FEED_SCROLL_STEP),
            KeyCode::KeyN => {
                self.host.cycle_theme();
                true
            }
            KeyCode::KeyM => {
                self.host.toggle_dark_mode();
                true
            }
            _ => false,
        }
    }

    fn named_theme_options(&self) -> &'static [&'static str] {
        PHYSICAL_THEME_OPTIONS
    }

    fn set_named_theme(
        &mut self,
        theme: &str,
        dark_mode: Option<bool>,
    ) -> Option<(&'static str, bool)> {
        self.host.set_named_theme(theme, dark_mode)
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
