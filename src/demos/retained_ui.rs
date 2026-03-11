//! Demo 9: Retained UI
//!
//! Renders a small retained sample scene through the shared retained UI
//! realization path and the shared interim Ui2D scene host.

use super::{
    ui2d_runtime::{create_showcase_ui2d_deck, load_dejavu_font_atlas, ShowcaseUi2dDeck},
    Demo, DemoContext, DemoId, DemoType, NamedScrollTarget,
};
use crate::camera::FlyCamera;
use crate::input::CameraConfig;
use crate::retained::showcase::ShowcaseSceneDeckTarget;
use crate::retained::text::{TextColors, TextRenderSpace};
use crate::retained::ui::UiRenderSpace;
use anyhow::Result;
use winit::keyboard::KeyCode;

const FEED_SCROLL_STEP: f32 = 24.0;
const RETAINED_UI_KEYBINDINGS: &[(&str, &str)] = &[
    ("WASD", "Pan"),
    ("Arrows", "Zoom"),
    ("Q/E", "Rotate"),
    ("R", "Reset rotation"),
    ("T", "Reset all"),
    ("Y", "Next retained scene"),
    ("O", "Toggle active scene state"),
    ("U/J", "Scroll active retained scene"),
];

fn render_space(screen_height: f32) -> UiRenderSpace {
    UiRenderSpace {
        x_offset: 0.0,
        screen_height,
    }
}

pub struct RetainedUiDemo {
    deck: ShowcaseUi2dDeck,
}

impl RetainedUiDemo {
    pub fn new(ctx: &DemoContext) -> Result<Self> {
        let atlas = load_dejavu_font_atlas()?;
        Ok(Self {
            deck: create_showcase_ui2d_deck(
                ctx,
                atlas,
                retained_text_colors(),
                text_render_space(ctx.height as f32),
                render_space(ctx.height as f32),
            ),
        })
    }
}

fn retained_text_colors() -> TextColors {
    TextColors {
        heading: [0.13, 0.16, 0.23],
        active: [0.20, 0.24, 0.31],
        completed: [0.39, 0.45, 0.55],
        placeholder: [0.58, 0.64, 0.72],
        body: [0.23, 0.29, 0.36],
        info: [0.42, 0.48, 0.58],
    }
}

fn text_render_space(screen_height: f32) -> TextRenderSpace {
    TextRenderSpace {
        x_offset: 0.0,
        screen_height,
        italic_codepoint_offset: 0x10000,
    }
}

impl Demo for RetainedUiDemo {
    fn name(&self) -> &'static str {
        "Retained UI"
    }

    fn id(&self) -> DemoId {
        DemoId::RetainedUi
    }

    fn demo_type(&self) -> DemoType {
        DemoType::Ui2D
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        RETAINED_UI_KEYBINDINGS
    }

    fn camera_config(&self) -> CameraConfig {
        CameraConfig::default()
    }

    fn update(&mut self, _dt: f32, _camera: &mut FlyCamera) {}

    fn prepare_frame(&mut self, queue: &wgpu::Queue) {
        let _ = queue;
        self.deck.prepare_frame();
    }

    fn needs_redraw(&self) -> bool {
        self.deck.needs_redraw()
    }

    fn apply_2d_view_controls(
        &mut self,
        offset_delta: [f32; 2],
        scale_factor: f32,
        rotation_delta: f32,
    ) {
        self.deck
            .apply_view_controls(offset_delta, scale_factor, rotation_delta);
    }

    fn reset_2d_rotation(&mut self) {
        self.deck.reset_rotation();
    }

    fn reset_2d_all(&mut self) {
        self.deck.reset_all();
    }

    fn named_scroll_target(&mut self) -> Option<&mut dyn NamedScrollTarget> {
        Some(&mut self.deck)
    }

    fn has_named_scroll_target(&self) -> bool {
        true
    }

    fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        _time: f32,
    ) {
        self.deck.render(render_pass, queue);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.deck.resize(width, height);
    }

    fn handle_key_pressed(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::KeyY => {
                let _ = self.deck.cycle_showcase_scene();
                true
            }
            KeyCode::KeyO => self.deck.toggle_active_showcase_state(),
            KeyCode::KeyU => self.deck.adjust_active_showcase_scroll(-FEED_SCROLL_STEP),
            KeyCode::KeyJ => self.deck.adjust_active_showcase_scroll(FEED_SCROLL_STEP),
            _ => false,
        }
    }
}
