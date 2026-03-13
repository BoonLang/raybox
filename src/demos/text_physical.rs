use super::{
    ui2d_runtime::load_dejavu_font_atlas,
    ui_physical_runtime::{
        create_wrapped_text_ui_physical_host, ThemedUiPhysicalHost, UiPhysicalHostConfig,
        WrappedTextUiPhysicalDeck,
    },
    ui_physical_theme::{ThemeId, UiPhysicalThemeState, PHYSICAL_THEME_OPTIONS},
    Demo, DemoContext, DemoId, DemoType, NamedScrollTarget,
};
use crate::camera::FlyCamera;
use crate::demo_core::{ui_physical_card_camera_preset, UiPhysicalCameraPreset};
use crate::input::CameraConfig;
use crate::retained::text::TextRenderSpace;
use crate::retained::text_scene::{OwnedTextSceneBlock, WrappedTextSceneModel};
use crate::retained::ui::UiRenderSpace;
use crate::retained::{SceneMode, TextRole};
use anyhow::Result;
use winit::keyboard::KeyCode;

const LOREM: &str = "Retained physical UI should support text-heavy scenes without collapsing back into Todo-shaped assumptions. This demo exercises wrapped retained text, scrolling, and semantic text mutation through the shared UiPhysical runtime path.\n\nA retained physical scene should stay stable while idle, rebuild only what changed, and let the runtime choose how to realize the card, lighting, and text presentation.\n\nScrolling this text should work through the same retained model + named scroll infrastructure that powers other scenes.";
const TEXT_MARGIN: f32 = 20.0;
const BODY_FONT_SIZE: f32 = 20.0;
const BODY_LINE_HEIGHT: f32 = 31.0;
const HEADING_FONT_SIZE: f32 = 34.0;
const TEXT_GRID_DIMS: [u32; 2] = [64, 48];
const TEXT_GRID_CELL_CAPACITY: usize = 64;
const HEADING_TOP_PADDING: f32 = 12.0;
const TEXT_SCROLL_STEP: f32 = 48.0;
const TEXT_PARAGRAPH_COUNT: usize = 5;
const TEXT_PHYSICAL_FRAME_SIZE: [f32; 2] = [760.0, 560.0];
const TEXT_PHYSICAL_KEYBINDINGS: &[(&str, &str)] = &[
    ("WASD", "Move"),
    ("Mouse", "Look"),
    ("Space/Ctrl", "Up/Down"),
    ("Scroll", "Speed"),
    ("R", "Reset roll"),
    ("T", "Reset camera"),
    ("Tab", "Capture mouse"),
    ("Y", "Toggle heading emphasis"),
    ("U/J", "Scroll text"),
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

fn scene_model() -> WrappedTextSceneModel {
    WrappedTextSceneModel {
        scene_mode: SceneMode::UiPhysical,
        heading: Some(OwnedTextSceneBlock {
            text: "RETAINED UI PHYSICAL TEXT".to_string(),
            font_size: HEADING_FONT_SIZE,
            role: TextRole::Heading,
        }),
        body: OwnedTextSceneBlock {
            text: std::iter::repeat(LOREM)
                .take(TEXT_PARAGRAPH_COUNT)
                .collect::<Vec<_>>()
                .join("\n\n"),
            font_size: BODY_FONT_SIZE,
            role: TextRole::Body,
        },
        frame_size: Some(TEXT_PHYSICAL_FRAME_SIZE),
        margin: TEXT_MARGIN,
        body_line_height: BODY_LINE_HEIGHT,
        body_top_padding: HEADING_TOP_PADDING,
        scroll_offset: 0.0,
        grid_dims: TEXT_GRID_DIMS,
        grid_cell_capacity: TEXT_GRID_CELL_CAPACITY,
        clip_name: "text_physical_clip",
        scroll_name: "text_physical_scroll",
        heading_name: "text_physical_heading",
        line_name_prefix: "text_physical_line_",
    }
}

pub struct TextPhysicalDemo {
    host: ThemedUiPhysicalHost<WrappedTextUiPhysicalDeck>,
}

impl TextPhysicalDemo {
    pub fn new(ctx: &DemoContext) -> Result<Self> {
        let atlas = load_dejavu_font_atlas()?;
        let current_theme = ThemeId::Professional;
        let dark_mode = true;
        let theme_state = UiPhysicalThemeState::new(current_theme, dark_mode);
        let host = create_wrapped_text_ui_physical_host(
            ctx,
            "Retained UI Physical Text",
            scene_model(),
            atlas,
            theme_state,
            text_render_space(ctx.height as f32),
            ui_render_space(ctx.height as f32),
            UiPhysicalHostConfig {
                label: "Retained UI Physical Text Buffer".to_string(),
                max_char_instances: 1024,
                max_ui_primitives: 64,
                max_grid_indices: 16384,
                grid_cell_capacity: TEXT_GRID_CELL_CAPACITY,
            },
            0.0,
        );
        Ok(Self { host })
    }

    fn toggle_heading_emphasis(&mut self) -> bool {
        let model = self.host.scene().model().clone();
        self.host
            .scene_mut()
            .mutate_active_scene_and_capture(|scene| model.toggle_heading_emphasis(scene))
    }

    fn adjust_scroll(&mut self, delta_y: f32) -> bool {
        let model = self.host.scene().model().clone();
        self.host
            .scene_mut()
            .mutate_active_scene_and_capture(|scene| model.adjust_scroll(scene, delta_y))
    }
}

impl Demo for TextPhysicalDemo {
    fn name(&self) -> &'static str {
        "Text Physical"
    }

    fn id(&self) -> DemoId {
        DemoId::TextPhysical
    }

    fn demo_type(&self) -> DemoType {
        DemoType::UiPhysical
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        TEXT_PHYSICAL_KEYBINDINGS
    }

    fn camera_config(&self) -> CameraConfig {
        let preset = self
            .ui_physical_camera_preset()
            .unwrap_or_else(|| ui_physical_card_camera_preset(TEXT_PHYSICAL_FRAME_SIZE));
        CameraConfig::new(preset.fallback_offset, glam::Vec3::ZERO)
    }

    fn ui_physical_camera_preset(&self) -> Option<UiPhysicalCameraPreset> {
        let mut preset = ui_physical_card_camera_preset(TEXT_PHYSICAL_FRAME_SIZE);
        preset.fallback_offset = glam::Vec3::new(0.0, 9.2, 0.01);
        preset.min_distance = 4.6;
        preset.max_distance = 9.4;
        preset.clamp_x = 5.4;
        preset.max_height = 8.4;
        preset.clamp_z = 7.8;
        Some(preset)
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
            KeyCode::KeyY => self.toggle_heading_emphasis(),
            KeyCode::KeyU => self.adjust_scroll(-TEXT_SCROLL_STEP),
            KeyCode::KeyJ => self.adjust_scroll(TEXT_SCROLL_STEP),
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
