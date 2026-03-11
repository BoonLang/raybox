//! Demo 4: 2D Text on the shared retained Ui2D scene host.

use super::{
    ui2d_runtime::{load_dejavu_font_atlas, ModeledFixedUi2dSceneHost},
    Demo, DemoContext, DemoId, DemoType, NamedScrollTarget,
};
use crate::camera::FlyCamera;
use crate::input::CameraConfig;
use crate::retained::text::{TextColors, TextRenderSpace};
use crate::retained::text_scene::{OwnedTextSceneBlock, WrappedTextSceneModel};
use crate::retained::ui::UiRenderSpace;
use crate::retained::{SceneMode, TextRole};
use anyhow::Result;
use winit::keyboard::KeyCode;

const LOREM: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum. Curabitur pretium tincidunt lacus. Nulla gravida orci a odio. Nullam varius, turpis et commodo pharetra, est eros bibendum elit, nec luctus magna felis sollicitudin mauris. Integer in mauris eu nibh euismod gravida. Duis ac tellus et risus vulputate vehicula. Donec lobortis risus a elit. Etiam tempor. Ut ullamcorper, ligula eu tempor congue, eros est euismod turpis, id tincidunt sapien risus a quam. Maecenas fermentum consequat mi. Donec fermentum. Pellentesque malesuada nulla a mi. Duis sapien sem, aliquet sed, vulputate eget, feugiat non, orci. Sed neque. Sed eget lacus. Mauris non dui nec urna suscipit nonummy. Fusce fermentum fermentum arcu. Vestibulum ante ipsum primis in faucibus orci luctus et ultrices posuere cubilia curae.";

const TEXT_MARGIN: f32 = 20.0;
const BODY_FONT_SIZE: f32 = 16.0;
const BODY_LINE_HEIGHT: f32 = BODY_FONT_SIZE * 1.4;
const HEADING_FONT_SIZE: f32 = 30.0;
const TEXT2D_GRID_DIMS: [u32; 2] = [64, 48];
const TEXT2D_GRID_CELL_CAPACITY: usize = 24;
const HEADING_TOP_PADDING: f32 = 12.0;
const TEXT_SCROLL_STEP: f32 = 48.0;
const TEXT2D_KEYBINDINGS: &[(&str, &str)] = &[
    ("WASD", "Pan"),
    ("Arrows", "Zoom"),
    ("Q/E", "Rotate"),
    ("R", "Reset rotation"),
    ("T", "Reset all"),
    ("Y", "Toggle heading emphasis"),
    ("U/J", "Scroll text"),
];

fn render_space(screen_height: f32) -> UiRenderSpace {
    UiRenderSpace {
        x_offset: 0.0,
        screen_height,
    }
}

fn text_render_space(screen_height: f32) -> TextRenderSpace {
    TextRenderSpace {
        x_offset: 0.0,
        screen_height,
        italic_codepoint_offset: 0x10000,
    }
}

fn text_colors() -> TextColors {
    TextColors {
        heading: [0.16, 0.18, 0.22],
        active: [0.22, 0.22, 0.24],
        completed: [0.44, 0.44, 0.46],
        placeholder: [0.55, 0.55, 0.58],
        body: [0.20, 0.20, 0.22],
        info: [0.40, 0.40, 0.44],
    }
}

pub struct Text2DDemo {
    host: ModeledFixedUi2dSceneHost<WrappedTextSceneModel>,
}

impl Text2DDemo {
    fn scene_model(full_text: String, emphasized_heading: bool) -> WrappedTextSceneModel {
        WrappedTextSceneModel {
            scene_mode: SceneMode::Ui2D,
            heading: Some(OwnedTextSceneBlock {
                text: if emphasized_heading {
                    "VECTOR SDF TEXT ENGINE".to_string()
                } else {
                    "VECTOR UI TEXT ENGINE".to_string()
                },
                font_size: HEADING_FONT_SIZE,
                role: if emphasized_heading {
                    TextRole::Heading
                } else {
                    TextRole::Info
                },
            }),
            body: OwnedTextSceneBlock {
                text: full_text,
                font_size: BODY_FONT_SIZE,
                role: TextRole::Body,
            },
            frame_size: None,
            margin: TEXT_MARGIN,
            body_line_height: BODY_LINE_HEIGHT,
            body_top_padding: HEADING_TOP_PADDING,
            scroll_offset: 0.0,
            grid_dims: TEXT2D_GRID_DIMS,
            grid_cell_capacity: TEXT2D_GRID_CELL_CAPACITY,
            clip_name: "text2d_clip",
            scroll_name: "text2d_scroll",
            heading_name: "text2d_heading",
            line_name_prefix: "text2d_line_",
        }
    }

    pub fn new(ctx: &DemoContext) -> Result<Self> {
        let atlas = load_dejavu_font_atlas()?;
        let full_text = format!(
            "{} {} {} {} {} {}",
            LOREM, LOREM, LOREM, LOREM, LOREM, LOREM
        );
        let model = Self::scene_model(full_text, true);
        let host = ModeledFixedUi2dSceneHost::new(
            ctx,
            "Text2D",
            model.clone(),
            atlas.clone(),
            text_colors(),
            text_render_space(ctx.height as f32),
            render_space(ctx.height as f32),
        );
        Ok(Self { host })
    }

    fn toggle_heading_emphasis(&mut self) -> bool {
        let model = self.host.model().clone();
        self.host
            .mutate_scene_and_capture(|scene| model.toggle_heading_emphasis(scene))
    }

    fn adjust_scroll(&mut self, delta_y: f32) -> bool {
        let model = self.host.model().clone();
        self.host
            .mutate_scene_and_capture(|scene| model.adjust_scroll(scene, delta_y))
    }
}

impl Demo for Text2DDemo {
    fn name(&self) -> &'static str {
        "2D Text"
    }

    fn id(&self) -> DemoId {
        DemoId::Text2D
    }

    fn demo_type(&self) -> DemoType {
        DemoType::Ui2D
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        TEXT2D_KEYBINDINGS
    }

    fn camera_config(&self) -> CameraConfig {
        CameraConfig::default()
    }

    fn update(&mut self, _dt: f32, _camera: &mut FlyCamera) {}

    fn prepare_frame(&mut self, queue: &wgpu::Queue) {
        let _ = queue;
        self.host.prepare_frame();
    }

    fn needs_redraw(&self) -> bool {
        self.host.needs_redraw()
    }

    fn apply_2d_view_controls(
        &mut self,
        offset_delta: [f32; 2],
        scale_factor: f32,
        rotation_delta: f32,
    ) {
        self.host
            .apply_view_controls(offset_delta, scale_factor, rotation_delta);
    }

    fn reset_2d_rotation(&mut self) {
        self.host.reset_rotation();
    }

    fn reset_2d_all(&mut self) {
        self.host.reset_all();
    }

    fn named_scroll_target(&mut self) -> Option<&mut dyn NamedScrollTarget> {
        Some(&mut self.host)
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
        self.host.render(render_pass, queue);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.host.resize_and_rebuild(width, height);
    }

    fn handle_key_pressed(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::KeyY => self.toggle_heading_emphasis(),
            KeyCode::KeyU => self.adjust_scroll(-TEXT_SCROLL_STEP),
            KeyCode::KeyJ => self.adjust_scroll(TEXT_SCROLL_STEP),
            _ => false,
        }
    }
}
