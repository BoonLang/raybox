use super::{
    scroll_offset_for_node, set_named_scroll_offset, ElementKind, NamedScrollSceneModel, Rect,
    RenderNodeDescriptor, RenderNodeKind, RetainedScene, SceneMode, ScrollState, TextRole,
    UiVisualRole, UiVisualStyle,
};
use crate::retained::fixed_scene::{
    build_fixed_ui2d_scene, BuiltFixedUi2dScene, FixedUi2dSceneInit, FixedUi2dSceneModelBuilder,
    FixedUi2dSceneModelCapture, FixedUi2dSceneState,
};
use crate::retained::text::{assign_text_slots_and_build_layout, FixedTextRunLayout};
use crate::retained::text::{
    build_fixed_text_scene_state_for_scene, OwnedTextRunLayout, TextColors, TextRenderSpace,
};
use crate::retained::ui::{build_gpu_ui_scene, UiRenderSpace};
use crate::retained::TextNode;
use crate::text::{FixedCharGridSpec, VectorFontAtlas};
use std::borrow::Cow;

fn frame_shadow_style(scene_mode: SceneMode) -> UiVisualStyle {
    let (base_color, offset, extra_size) = match scene_mode {
        SceneMode::UiPhysical => (
            [15.0 / 255.0, 23.0 / 255.0, 42.0 / 255.0, 0.28],
            [0.0, 20.0],
            [16.0, 13.0],
        ),
        _ => (
            [15.0 / 255.0, 23.0 / 255.0, 42.0 / 255.0, 0.16],
            [0.0, 18.0],
            [0.0, 0.0],
        ),
    };
    UiVisualStyle {
        base_color,
        accent_color: [0.0; 4],
        detail_color: [0.0; 4],
        stroke_width: 0.0,
        corner_radius: 18.0,
        offset,
        extra_size,
    }
}

fn frame_fill_style(scene_mode: SceneMode) -> UiVisualStyle {
    let (base_color, accent_color, detail_color) = match scene_mode {
        SceneMode::UiPhysical => (
            [230.0 / 255.0, 237.0 / 255.0, 246.0 / 255.0, 0.998],
            [182.0 / 255.0, 214.0 / 255.0, 245.0 / 255.0, 0.52],
            [111.0 / 255.0, 173.0 / 255.0, 235.0 / 255.0, 0.38],
        ),
        _ => (
            [248.0 / 255.0, 250.0 / 255.0, 252.0 / 255.0, 0.96],
            [225.0 / 255.0, 236.0 / 255.0, 251.0 / 255.0, 0.32],
            [191.0 / 255.0, 219.0 / 255.0, 254.0 / 255.0, 0.24],
        ),
    };
    UiVisualStyle {
        base_color,
        accent_color,
        detail_color,
        stroke_width: 0.0,
        corner_radius: 18.0,
        offset: [0.0, 0.0],
        extra_size: [0.0, 0.0],
    }
}

fn frame_outline_style(scene_mode: SceneMode) -> UiVisualStyle {
    UiVisualStyle {
        base_color: match scene_mode {
            SceneMode::UiPhysical => [86.0 / 255.0, 112.0 / 255.0, 152.0 / 255.0, 1.0],
            _ => [203.0 / 255.0, 213.0 / 255.0, 225.0 / 255.0, 1.0],
        },
        accent_color: [0.0; 4],
        detail_color: [0.0; 4],
        stroke_width: if scene_mode == SceneMode::UiPhysical {
            1.8
        } else {
            1.0
        },
        corner_radius: 18.0,
        offset: [0.0, 0.0],
        extra_size: [0.0, 0.0],
    }
}

#[derive(Clone, Copy)]
pub struct TextSceneBlock<'a> {
    pub text: &'a str,
    pub font_size: f32,
    pub role: TextRole,
}

#[derive(Clone)]
pub struct OwnedTextSceneBlock {
    pub text: String,
    pub font_size: f32,
    pub role: TextRole,
}

#[derive(Clone)]
pub struct WrappedTextSceneModel {
    pub scene_mode: SceneMode,
    pub heading: Option<OwnedTextSceneBlock>,
    pub body: OwnedTextSceneBlock,
    pub frame_size: Option<[f32; 2]>,
    pub margin: f32,
    pub body_line_height: f32,
    pub body_top_padding: f32,
    pub scroll_offset: f32,
    pub grid_dims: [u32; 2],
    pub grid_cell_capacity: usize,
    pub clip_name: &'static str,
    pub scroll_name: &'static str,
    pub heading_name: &'static str,
    pub line_name_prefix: &'static str,
}

pub struct WrappedTextSceneConfig<'a> {
    pub scene_mode: SceneMode,
    pub heading: Option<TextSceneBlock<'a>>,
    pub body: TextSceneBlock<'a>,
    pub width: f32,
    pub height: f32,
    pub frame_size: Option<[f32; 2]>,
    pub margin: f32,
    pub body_line_height: f32,
    pub body_top_padding: f32,
    pub scroll_offset: f32,
    pub grid_dims: [u32; 2],
    pub grid_cell_capacity: usize,
    pub clip_name: &'static str,
    pub scroll_name: &'static str,
    pub heading_name: &'static str,
    pub line_name_prefix: &'static str,
}

pub struct BuiltWrappedTextScene {
    pub scene: RetainedScene,
    text_layout: OwnedTextRunLayout,
}

impl BuiltWrappedTextScene {
    pub fn layout(&self) -> FixedTextRunLayout<'_> {
        self.text_layout.layout()
    }

    pub fn build_ui2d_init(
        &self,
        atlas: &VectorFontAtlas,
        colors: &TextColors,
        text_space: TextRenderSpace,
        ui_space: UiRenderSpace,
    ) -> FixedUi2dSceneInit {
        let ui_data = build_gpu_ui_scene(&self.scene, ui_space);
        let (_text_state, text_data) = build_fixed_text_scene_state_for_scene(
            &self.scene,
            self.layout(),
            atlas,
            colors,
            text_space,
        );
        FixedUi2dSceneInit { text_data, ui_data }
    }

    pub fn into_fixed_ui2d_scene(
        self,
        atlas: &VectorFontAtlas,
        colors: &TextColors,
        text_space: TextRenderSpace,
        ui_space: UiRenderSpace,
    ) -> (FixedUi2dSceneState, FixedUi2dSceneInit) {
        let layout = self.text_layout;
        FixedUi2dSceneState::new(
            self.scene,
            layout.layout(),
            atlas,
            colors,
            text_space,
            ui_space,
        )
    }
}

impl WrappedTextSceneModel {
    pub fn build_scene(
        &self,
        atlas: &VectorFontAtlas,
        width: f32,
        height: f32,
    ) -> BuiltWrappedTextScene {
        build_wrapped_text_scene(
            atlas,
            WrappedTextSceneConfig {
                scene_mode: self.scene_mode,
                heading: self.heading.as_ref().map(|heading| TextSceneBlock {
                    text: heading.text.as_str(),
                    font_size: heading.font_size,
                    role: heading.role,
                }),
                body: TextSceneBlock {
                    text: self.body.text.as_str(),
                    font_size: self.body.font_size,
                    role: self.body.role,
                },
                width,
                height,
                frame_size: self.frame_size,
                margin: self.margin,
                body_line_height: self.body_line_height,
                body_top_padding: self.body_top_padding,
                scroll_offset: self.scroll_offset,
                grid_dims: self.grid_dims,
                grid_cell_capacity: self.grid_cell_capacity,
                clip_name: self.clip_name,
                scroll_name: self.scroll_name,
                heading_name: self.heading_name,
                line_name_prefix: self.line_name_prefix,
            },
        )
    }

    pub fn build_fixed_ui2d_scene(
        &self,
        atlas: &VectorFontAtlas,
        width: f32,
        height: f32,
        colors: &TextColors,
        text_space: TextRenderSpace,
        ui_space: UiRenderSpace,
    ) -> BuiltFixedUi2dScene {
        let BuiltWrappedTextScene { scene, text_layout } = self.build_scene(atlas, width, height);
        build_fixed_ui2d_scene(
            scene,
            text_layout.layout(),
            atlas,
            colors,
            text_space,
            ui_space,
        )
    }

    pub fn heading_is_emphasized(&self) -> bool {
        self.heading
            .as_ref()
            .is_some_and(|heading| heading.role == TextRole::Heading)
    }

    pub fn toggle_heading_emphasis(&self, scene: &mut RetainedScene) -> bool {
        let Some(heading_id) = scene.node_named(self.heading_name).map(|node| node.id) else {
            return false;
        };
        let next_emphasized = !self.heading_is_emphasized();
        let font_size = self
            .heading
            .as_ref()
            .map(|heading| heading.font_size)
            .unwrap_or(30.0);
        let role_changed = scene.set_text_role(
            heading_id,
            Some(if next_emphasized {
                TextRole::Heading
            } else {
                TextRole::Info
            }),
        );
        let text_changed = scene.set_text(
            heading_id,
            Some(TextNode::new(
                if next_emphasized {
                    "VECTOR SDF TEXT ENGINE"
                } else {
                    "VECTOR UI TEXT ENGINE"
                },
                font_size,
            )),
        );
        role_changed || text_changed
    }

    pub fn set_scroll_offset(&self, scene: &mut RetainedScene, y: f32) -> bool {
        set_named_scroll_offset(scene, self.scroll_name, y)
    }

    pub fn adjust_scroll(&self, scene: &mut RetainedScene, delta_y: f32) -> bool {
        self.set_scroll_offset(scene, self.scroll_offset + delta_y)
    }
}

impl NamedScrollSceneModel for WrappedTextSceneModel {
    fn set_named_scroll_offset(
        &self,
        scene: &mut RetainedScene,
        name: &str,
        offset_y: f32,
    ) -> bool {
        if name != self.scroll_name {
            return false;
        }
        set_named_scroll_offset(scene, self.scroll_name, offset_y)
    }
}

impl FixedUi2dSceneModelBuilder for WrappedTextSceneModel {
    fn build_fixed_ui2d_scene(
        &self,
        viewport_size: [u32; 2],
        atlas: &VectorFontAtlas,
        colors: &TextColors,
        text_space: TextRenderSpace,
        ui_space: UiRenderSpace,
    ) -> BuiltFixedUi2dScene {
        self.build_fixed_ui2d_scene(
            atlas,
            viewport_size[0] as f32,
            viewport_size[1] as f32,
            colors,
            text_space,
            ui_space,
        )
    }
}

impl FixedUi2dSceneModelCapture for WrappedTextSceneModel {
    fn capture_from_scene(&mut self, scene: &RetainedScene) {
        if let Some(node) = scene.node_named(self.heading_name) {
            if let Some(heading) = self.heading.as_mut() {
                if let Some(text) = node.text.as_ref() {
                    heading.text = text.text.to_string();
                    heading.font_size = text.font_size;
                }
                if let Some(role) = node.text_role {
                    heading.role = role;
                }
            }
        }

        if let Some(offset) = scroll_offset_for_node(scene, self.scroll_name) {
            self.scroll_offset = offset;
        }
    }
}

fn line_width(atlas: &VectorFontAtlas, text: &str, font_size: f32) -> f32 {
    text.chars()
        .filter_map(|ch| atlas.glyphs.get(&(ch as u32)))
        .map(|entry| entry.advance * font_size)
        .sum::<f32>()
}

fn wrap_text_lines(
    atlas: &VectorFontAtlas,
    text: &str,
    font_size: f32,
    max_width: f32,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0.0;
    let space_width = atlas
        .glyphs
        .get(&(' ' as u32))
        .map(|entry| entry.advance * font_size)
        .unwrap_or(0.3 * font_size);

    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }

        for word in paragraph.split_whitespace() {
            let word_width = line_width(atlas, word, font_size);
            let required_width = if current.is_empty() {
                word_width
            } else {
                current_width + space_width + word_width
            };

            if !current.is_empty() && required_width > max_width {
                lines.push(std::mem::take(&mut current));
                current_width = 0.0;
            }

            if !current.is_empty() {
                current.push(' ');
                current_width += space_width;
            }
            current.push_str(word);
            current_width += word_width;
        }

        if !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current_width = 0.0;
        }
    }

    lines
}

pub fn build_wrapped_text_scene(
    atlas: &VectorFontAtlas,
    config: WrappedTextSceneConfig<'_>,
) -> BuiltWrappedTextScene {
    let [frame_width, frame_height] = config.frame_size.unwrap_or([config.width, config.height]);
    let origin_x = ((config.width - frame_width) * 0.5).max(0.0);
    let origin_y = ((config.height - frame_height) * 0.5).max(0.0);
    let physical_frame_padding =
        if config.scene_mode == SceneMode::UiPhysical && config.frame_size.is_some() {
            8.0
        } else {
            0.0
        };
    let content_margin = config.margin + physical_frame_padding;
    let body_top_padding = config.body_top_padding + physical_frame_padding * 0.5;
    let lines = wrap_text_lines(
        atlas,
        config.body.text,
        config.body.font_size,
        frame_width - content_margin * 2.0,
    );
    let heading_height = config
        .heading
        .map(|heading| heading.font_size + body_top_padding)
        .unwrap_or(0.0);
    let content_height =
        (content_margin * 2.0 + heading_height + lines.len() as f32 * config.body_line_height)
            .max(frame_height);
    let mut scroll = ScrollState::new([frame_width, frame_height], [frame_width, content_height]);
    let max_offset = (scroll.content_size[1] - scroll.viewport_size[1]).max(0.0);
    scroll.offset[1] = config.scroll_offset.clamp(0.0, max_offset);

    let mut scene = RetainedScene::new(config.scene_mode);
    let root = scene.root();
    assert!(scene.set_bounds(root, Rect::new(0.0, 0.0, config.width, config.height)));

    if config.frame_size.is_some() {
        let frame_rect = Rect::new(origin_x, origin_y, frame_width, frame_height);
        scene
            .append_node(
                root,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Shadow,
                    frame_rect,
                )
                .named("text_frame_shadow")
                .with_ui_visual_role(UiVisualRole::BoxShadow)
                .with_ui_visual_style(frame_shadow_style(config.scene_mode)),
            )
            .expect("text frame shadow");
        scene
            .append_node(
                root,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Panel,
                    frame_rect,
                )
                .named("text_frame_fill")
                .with_ui_visual_role(UiVisualRole::FilledSurface)
                .with_ui_visual_style(frame_fill_style(config.scene_mode))
                .with_material(6.0, 10.0, 18.0),
            )
            .expect("text frame fill");
        scene
            .append_node(
                root,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Panel,
                    frame_rect,
                )
                .named("text_frame_outline")
                .with_ui_visual_role(UiVisualRole::OutlineRect)
                .with_ui_visual_style(frame_outline_style(config.scene_mode))
                .with_material(6.0, 10.0, 18.0),
            )
            .expect("text frame outline");
    }

    let clip = scene
        .append_node(
            root,
            RenderNodeDescriptor::new(
                RenderNodeKind::Clip,
                ElementKind::Clip,
                Rect::new(origin_x, origin_y, frame_width, frame_height),
            )
            .named(config.clip_name)
            .with_clip(),
        )
        .expect("text scene clip");

    let scroll_root = scene
        .append_node(
            clip,
            RenderNodeDescriptor::new(
                RenderNodeKind::ScrollRoot,
                ElementKind::ScrollContainer,
                Rect::new(origin_x, origin_y, frame_width, frame_height),
            )
            .named(config.scroll_name)
            .with_scroll(scroll),
        )
        .expect("text scene scroll root");

    let mut next_y = origin_y + content_margin;

    if let Some(heading) = config.heading {
        let heading_width = line_width(atlas, heading.text, heading.font_size);
        scene
            .append_node(
                scroll_root,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Heading,
                    Rect::new(
                        origin_x + content_margin,
                        next_y,
                        heading_width,
                        heading.font_size,
                    ),
                )
                .named(config.heading_name)
                .with_text(Cow::Owned(heading.text.to_string()), heading.font_size)
                .with_text_role(heading.role),
            )
            .expect("text scene heading");
        next_y += heading.font_size + body_top_padding;
    }

    for (line_idx, line) in lines.iter().enumerate() {
        let width = line_width(atlas, line, config.body.font_size);
        let y = next_y + line_idx as f32 * config.body_line_height;
        scene
            .append_node(
                scroll_root,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Text,
                    Rect::new(origin_x + content_margin, y, width, config.body.font_size),
                )
                .named(format!("{}{}", config.line_name_prefix, line_idx))
                .with_text(Cow::Owned(line.clone()), config.body.font_size)
                .with_text_role(config.body.role),
            )
            .expect("text scene body line");
    }

    let text_layout = assign_text_slots_and_build_layout(
        &mut scene,
        FixedCharGridSpec {
            dims: config.grid_dims,
            bounds: [0.0, 0.0, config.width, config.height],
            cell_capacity: config.grid_cell_capacity,
        },
        4,
    );

    BuiltWrappedTextScene { scene, text_layout }
}

#[cfg(test)]
mod tests {
    use super::{
        build_wrapped_text_scene, set_named_scroll_offset, OwnedTextSceneBlock, TextSceneBlock,
        WrappedTextSceneConfig, WrappedTextSceneModel,
    };
    use crate::retained::{SceneMode, TextRole};
    use crate::text::{VectorFont, VectorFontAtlas};

    fn load_test_atlas() -> VectorFontAtlas {
        let font_data = std::fs::read("assets/fonts/DejaVuSans.ttf").expect("load test font");
        let font = VectorFont::from_ttf(&font_data).expect("parse test font");
        VectorFontAtlas::from_font(&font, 32)
    }

    #[test]
    fn wrapped_text_scene_builds_heading_and_lines() {
        let atlas = load_test_atlas();
        let built = build_wrapped_text_scene(
            &atlas,
            WrappedTextSceneConfig {
                scene_mode: SceneMode::Ui2D,
                heading: Some(TextSceneBlock {
                    text: "Heading",
                    font_size: 24.0,
                    role: TextRole::Heading,
                }),
                body: TextSceneBlock {
                    text: "alpha beta gamma delta epsilon zeta eta theta",
                    font_size: 14.0,
                    role: TextRole::Body,
                },
                width: 180.0,
                height: 140.0,
                frame_size: None,
                margin: 12.0,
                body_line_height: 18.0,
                body_top_padding: 10.0,
                scroll_offset: 0.0,
                grid_dims: [16, 12],
                grid_cell_capacity: 8,
                clip_name: "clip",
                scroll_name: "scroll",
                heading_name: "heading",
                line_name_prefix: "line_",
            },
        );

        assert!(built.scene.node_named("heading").is_some());
        assert!(built.scene.node_named("line_0").is_some());
        assert!(built.scene.node_named("scroll").is_some());
        assert!(built.layout().run_capacities.len() > 1);
    }

    #[test]
    fn wrapped_text_scene_model_builds_heading_and_lines() {
        let atlas = load_test_atlas();
        let model = WrappedTextSceneModel {
            scene_mode: SceneMode::Ui2D,
            heading: Some(OwnedTextSceneBlock {
                text: "Heading".to_string(),
                font_size: 24.0,
                role: TextRole::Heading,
            }),
            body: OwnedTextSceneBlock {
                text: "alpha beta gamma delta epsilon zeta eta theta".to_string(),
                font_size: 14.0,
                role: TextRole::Body,
            },
            frame_size: None,
            margin: 12.0,
            body_line_height: 18.0,
            body_top_padding: 10.0,
            scroll_offset: 0.0,
            grid_dims: [16, 12],
            grid_cell_capacity: 8,
            clip_name: "clip",
            scroll_name: "scroll",
            heading_name: "heading",
            line_name_prefix: "line_",
        };
        let built = model.build_scene(&atlas, 180.0, 140.0);

        assert!(built.scene.node_named("heading").is_some());
        assert!(built.scene.node_named("line_0").is_some());
        assert!(built.layout().run_capacities.len() > 1);
        assert_eq!(built.scene.mode(), SceneMode::Ui2D);
    }

    #[test]
    fn wrapped_text_scene_model_captures_heading_state_from_scene() {
        let atlas = load_test_atlas();
        let mut model = WrappedTextSceneModel {
            scene_mode: SceneMode::Ui2D,
            heading: Some(OwnedTextSceneBlock {
                text: "Heading".to_string(),
                font_size: 24.0,
                role: TextRole::Heading,
            }),
            body: OwnedTextSceneBlock {
                text: "alpha beta gamma".to_string(),
                font_size: 14.0,
                role: TextRole::Body,
            },
            frame_size: None,
            margin: 12.0,
            body_line_height: 18.0,
            body_top_padding: 10.0,
            scroll_offset: 0.0,
            grid_dims: [16, 12],
            grid_cell_capacity: 8,
            clip_name: "clip",
            scroll_name: "scroll",
            heading_name: "heading",
            line_name_prefix: "line_",
        };
        let mut built = model.build_scene(&atlas, 180.0, 140.0);
        let heading = built.scene.node_named("heading").expect("heading").id;
        assert!(built.scene.set_text_role(heading, Some(TextRole::Info)));
        assert!(built.scene.set_text(
            heading,
            Some(crate::retained::TextNode::new("Changed", 20.0))
        ));

        super::FixedUi2dSceneModelCapture::capture_from_scene(&mut model, &built.scene);

        let heading = model.heading.expect("heading model");
        assert_eq!(heading.text, "Changed");
        assert_eq!(heading.font_size, 20.0);
        assert_eq!(heading.role, TextRole::Info);
    }

    #[test]
    fn wrapped_text_scene_model_captures_scroll_state_from_scene() {
        let atlas = load_test_atlas();
        let mut model = WrappedTextSceneModel {
            scene_mode: SceneMode::Ui2D,
            heading: None,
            body: OwnedTextSceneBlock {
                text: "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron"
                    .to_string(),
                font_size: 14.0,
                role: TextRole::Body,
            },
            frame_size: None,
            margin: 12.0,
            body_line_height: 18.0,
            body_top_padding: 10.0,
            scroll_offset: 0.0,
            grid_dims: [16, 12],
            grid_cell_capacity: 8,
            clip_name: "clip",
            scroll_name: "scroll",
            heading_name: "heading",
            line_name_prefix: "line_",
        };
        let mut built = model.build_scene(&atlas, 120.0, 80.0);
        assert!(set_named_scroll_offset(&mut built.scene, "scroll", 24.0));

        super::FixedUi2dSceneModelCapture::capture_from_scene(&mut model, &built.scene);

        assert!(model.scroll_offset > 0.0);
    }

    #[test]
    fn wrapped_text_scene_model_respects_scene_mode() {
        let atlas = load_test_atlas();
        let model = WrappedTextSceneModel {
            scene_mode: SceneMode::UiPhysical,
            heading: None,
            body: OwnedTextSceneBlock {
                text: "physical wrapped text".to_string(),
                font_size: 16.0,
                role: TextRole::Body,
            },
            frame_size: Some([240.0, 180.0]),
            margin: 16.0,
            body_line_height: 20.0,
            body_top_padding: 10.0,
            scroll_offset: 0.0,
            grid_dims: [16, 12],
            grid_cell_capacity: 8,
            clip_name: "clip",
            scroll_name: "scroll",
            heading_name: "heading",
            line_name_prefix: "line_",
        };

        let built = model.build_scene(&atlas, 320.0, 240.0);
        assert_eq!(built.scene.mode(), SceneMode::UiPhysical);
    }
}
