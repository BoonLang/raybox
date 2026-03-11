use super::{
    set_named_scroll_offset, ElementKind, NamedScrollSceneModel, Rect, RenderNodeDescriptor,
    RenderNodeKind, RetainedScene, SceneMode, ScrollState, SelectionState, TextNode, TextRole,
    ToggleState, UiVisualRole, UiVisualStyle,
};
use crate::retained::fixed_scene::{
    build_fixed_ui2d_scene, BuiltFixedUi2dScene, FixedUi2dSceneModelBuilder,
    FixedUi2dSceneModelCapture,
};
use crate::retained::text::{
    assign_text_slots_and_build_layout, OwnedTextRunLayout, TextColors, TextRenderSpace,
};
use crate::retained::ui::UiRenderSpace;
use crate::text::{FixedCharGridSpec, VectorFontAtlas};

fn rgb(r: u8, g: u8, b: u8) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

fn rgba(r: u8, g: u8, b: u8, a: f32) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a]
}

fn panel_shadow_style(scene_mode: SceneMode) -> UiVisualStyle {
    let (base_color, offset, extra_size) = match scene_mode {
        SceneMode::UiPhysical => (rgba(15, 23, 42, 0.28), [0.0, 18.0], [14.0, 12.0]),
        _ => (rgba(15, 23, 42, 0.18), [0.0, 12.0], [0.0, 0.0]),
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

fn panel_fill_style(scene_mode: SceneMode) -> UiVisualStyle {
    let (base_color, accent_color, detail_color) = match scene_mode {
        SceneMode::UiPhysical => (
            rgba(228, 236, 246, 0.998),
            rgba(176, 210, 245, 0.54),
            rgba(106, 170, 236, 0.38),
        ),
        _ => (
            rgba(248, 250, 252, 0.96),
            rgba(224, 242, 254, 0.24),
            rgba(186, 230, 253, 0.18),
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

fn separator_style() -> UiVisualStyle {
    UiVisualStyle {
        base_color: rgba(203, 213, 225, 1.0),
        accent_color: [0.0; 4],
        detail_color: [0.0; 4],
        stroke_width: 1.0,
        corner_radius: 0.0,
        offset: [0.0, 0.0],
        extra_size: [0.0, 0.0],
    }
}

fn checkbox_style() -> UiVisualStyle {
    UiVisualStyle {
        base_color: rgb(148, 163, 184),
        accent_color: rgb(14, 165, 233),
        detail_color: rgb(2, 132, 199),
        stroke_width: 1.6,
        corner_radius: 0.0,
        offset: [0.0, 0.0],
        extra_size: [0.0, 0.0],
    }
}

fn strike_style() -> UiVisualStyle {
    UiVisualStyle {
        base_color: rgba(100, 116, 139, 1.0),
        accent_color: [0.0; 4],
        detail_color: [0.0; 4],
        stroke_width: 2.0,
        corner_radius: 0.0,
        offset: [0.0, 0.0],
        extra_size: [0.0, 0.0],
    }
}

fn selection_style() -> UiVisualStyle {
    UiVisualStyle {
        base_color: rgba(37, 99, 235, 1.0),
        accent_color: [0.0; 4],
        detail_color: [0.0; 4],
        stroke_width: 1.0,
        corner_radius: 8.0,
        offset: [-6.0, -5.0],
        extra_size: [12.0, 10.0],
    }
}

fn outline_style(scene_mode: SceneMode) -> UiVisualStyle {
    UiVisualStyle {
        base_color: match scene_mode {
            SceneMode::UiPhysical => rgba(84, 110, 150, 1.0),
            _ => rgba(148, 163, 184, 1.0),
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

pub const SAMPLE_SCENE_WIDTH: f32 = 420.0;
pub const SAMPLE_SCENE_HEIGHT: f32 = 260.0;
const FEED_ROWS: [(&str, &str); 5] = [
    ("Review release notes", "Waiting on docs sign-off"),
    ("Cut beta build", "Queued after asset import finishes"),
    ("Notify QA channel", "Pinned checklist updated"),
    ("Prepare rollout post", "Marketing copy is ready"),
    ("Archive previous sprint", "Move links into handbook"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleSceneKind {
    SettingsPanel,
    ScrollingFeed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SampleSceneState {
    SettingsPanel { enabled: bool },
    ScrollingFeed { scroll_offset: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SampleSceneAction {
    ToggleSettingsPanel,
    AdjustFeedScroll(f32),
}

#[derive(Debug, Clone, Copy)]
pub struct SampleSceneConfig {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct SampleSceneModel {
    pub scene_mode: SceneMode,
    pub kind: SampleSceneKind,
    pub state: SampleSceneState,
}

pub trait SampleSceneDeckTarget {
    fn active_sample_scene_kind(&self) -> SampleSceneKind;
    fn cycle_sample_scene(&mut self) -> bool;
    fn apply_active_sample_scene_action(&mut self, action: SampleSceneAction) -> bool;

    fn toggle_active_settings_panel(&mut self) -> bool {
        if self.active_sample_scene_kind() != SampleSceneKind::SettingsPanel {
            return false;
        }
        self.apply_active_sample_scene_action(SampleSceneAction::ToggleSettingsPanel)
    }

    fn adjust_active_feed_scroll(&mut self, delta_y: f32) -> bool {
        if self.active_sample_scene_kind() != SampleSceneKind::ScrollingFeed {
            return false;
        }
        self.apply_active_sample_scene_action(SampleSceneAction::AdjustFeedScroll(delta_y))
    }
}

impl Default for SampleSceneConfig {
    fn default() -> Self {
        Self {
            width: SAMPLE_SCENE_WIDTH,
            height: SAMPLE_SCENE_HEIGHT,
        }
    }
}

impl SampleSceneModel {
    pub fn new(kind: SampleSceneKind) -> Self {
        Self::new_with_mode(kind, SceneMode::Ui2D)
    }

    pub fn new_with_mode(kind: SampleSceneKind, scene_mode: SceneMode) -> Self {
        Self {
            scene_mode,
            kind,
            state: default_sample_scene_state(kind),
        }
    }

    pub fn label(&self) -> &'static str {
        match self.kind {
            SampleSceneKind::SettingsPanel => "Retained UI Settings",
            SampleSceneKind::ScrollingFeed => "Retained UI Feed",
        }
    }

    pub fn build_scene(&self, viewport: SampleSceneConfig) -> (RetainedScene, OwnedTextRunLayout) {
        let (mut scene, layout) = match self.kind {
            SampleSceneKind::SettingsPanel => {
                build_settings_panel_scene_parts(viewport, self.scene_mode)
            }
            SampleSceneKind::ScrollingFeed => {
                build_scrolling_feed_scene_parts(viewport, self.scene_mode)
            }
        };
        let _ = self.apply_to_scene(&mut scene);
        (scene, layout)
    }

    pub fn build_fixed_ui2d_scene(
        &self,
        viewport: SampleSceneConfig,
        atlas: &VectorFontAtlas,
        colors: &TextColors,
        text_space: TextRenderSpace,
        ui_space: UiRenderSpace,
    ) -> BuiltFixedUi2dScene {
        let (scene, layout) = self.build_scene(viewport);
        build_fixed_ui2d_scene(scene, layout.layout(), atlas, colors, text_space, ui_space)
    }

    pub fn capture_state_from_scene(&mut self, scene: &RetainedScene) {
        if let Some(state) = sample_scene_state_from_scene(self.kind, scene) {
            self.state = state;
        }
    }

    pub fn apply_to_scene(&self, scene: &mut RetainedScene) -> bool {
        apply_sample_scene_state(scene, self.state)
    }

    pub fn apply_action(&self, scene: &mut RetainedScene, action: SampleSceneAction) -> bool {
        apply_sample_scene_action(self.kind, scene, action)
    }
}

impl NamedScrollSceneModel for SampleSceneModel {
    fn set_named_scroll_offset(
        &self,
        scene: &mut RetainedScene,
        name: &str,
        offset_y: f32,
    ) -> bool {
        if self.kind != SampleSceneKind::ScrollingFeed || name != "feed_scroll" {
            return false;
        }
        set_named_scroll_offset(scene, name, offset_y)
    }
}

impl FixedUi2dSceneModelBuilder for SampleSceneModel {
    fn build_fixed_ui2d_scene(
        &self,
        viewport_size: [u32; 2],
        atlas: &VectorFontAtlas,
        colors: &TextColors,
        text_space: TextRenderSpace,
        ui_space: UiRenderSpace,
    ) -> BuiltFixedUi2dScene {
        self.build_fixed_ui2d_scene(
            SampleSceneConfig {
                width: viewport_size[0] as f32,
                height: viewport_size[1] as f32,
            },
            atlas,
            colors,
            text_space,
            ui_space,
        )
    }
}

impl FixedUi2dSceneModelCapture for SampleSceneModel {
    fn capture_from_scene(&mut self, scene: &RetainedScene) {
        self.capture_state_from_scene(scene);
    }
}

pub fn apply_sample_scene_action_to_active<T: SampleSceneDeckTarget>(
    target: &mut T,
    action: SampleSceneAction,
) -> bool {
    target.apply_active_sample_scene_action(action)
}

fn sample_grid_spec(width: f32, height: f32) -> FixedCharGridSpec {
    FixedCharGridSpec {
        dims: [32, 20],
        bounds: [0.0, 0.0, width, height],
        cell_capacity: 64,
    }
}

fn assign_dynamic_ui_slots(scene: &mut RetainedScene) {
    let stale_slots = scene
        .nodes()
        .values()
        .filter(|node| node.ui_primitive_count == 0 && node.ui_slot.is_some())
        .map(|node| node.id)
        .collect::<Vec<_>>();
    for id in stale_slots {
        let _ = scene.set_ui_slot(id, None);
    }

    let ui_nodes = scene
        .descendants_sorted_by_resolved_position(scene.root())
        .into_iter()
        .filter(|node| node.ui_primitive_count > 0)
        .map(|node| node.id)
        .collect::<Vec<_>>();
    for (slot_index, id) in ui_nodes.into_iter().enumerate() {
        let ui_slot = u16::try_from(slot_index).expect("ui slot overflow");
        let _ = scene.set_ui_slot(id, Some(ui_slot));
    }
}

fn fit_panel(viewport: SampleSceneConfig, preferred: [f32; 2], margin: [f32; 2]) -> Rect {
    let width = preferred[0].min((viewport.width - margin[0] * 2.0).max(120.0));
    let height = preferred[1].min((viewport.height - margin[1] * 2.0).max(96.0));
    Rect::new(
        ((viewport.width - width) * 0.5).max(margin[0]),
        ((viewport.height - height) * 0.5).max(margin[1]),
        width,
        height,
    )
}

fn inset_rect(rect: Rect, inset: [f32; 2]) -> Rect {
    Rect::new(
        rect.x + inset[0],
        rect.y + inset[1],
        (rect.width - inset[0] * 2.0).max(1.0),
        (rect.height - inset[1] * 2.0).max(1.0),
    )
}

pub fn sample_text_run_layout() -> OwnedTextRunLayout {
    sample_text_run_layout_for_size(SAMPLE_SCENE_WIDTH, SAMPLE_SCENE_HEIGHT)
}

pub fn sample_text_run_layout_for_size(width: f32, height: f32) -> OwnedTextRunLayout {
    build_settings_panel_scene_parts(SampleSceneConfig { width, height }, SceneMode::Ui2D).1
}

pub fn scrolling_feed_text_run_layout() -> OwnedTextRunLayout {
    scrolling_feed_text_run_layout_for_size(SAMPLE_SCENE_WIDTH, SAMPLE_SCENE_HEIGHT)
}

pub fn scrolling_feed_text_run_layout_for_size(width: f32, height: f32) -> OwnedTextRunLayout {
    build_scrolling_feed_scene_parts(SampleSceneConfig { width, height }, SceneMode::Ui2D).1
}

pub fn build_settings_panel_scene() -> RetainedScene {
    build_settings_panel_scene_with_config(SampleSceneConfig::default(), SceneMode::Ui2D)
}

fn build_settings_panel_scene_parts(
    config: SampleSceneConfig,
    scene_mode: SceneMode,
) -> (RetainedScene, OwnedTextRunLayout) {
    let mut scene = build_settings_panel_scene_base(config, scene_mode);
    assign_dynamic_ui_slots(&mut scene);
    let layout = assign_text_slots_and_build_layout(
        &mut scene,
        sample_grid_spec(config.width, config.height),
        16,
    );
    (scene, layout)
}

pub fn default_sample_scene_state(kind: SampleSceneKind) -> SampleSceneState {
    match kind {
        SampleSceneKind::SettingsPanel => SampleSceneState::SettingsPanel { enabled: true },
        SampleSceneKind::ScrollingFeed => SampleSceneState::ScrollingFeed { scroll_offset: 0.0 },
    }
}

pub fn build_settings_panel_scene_with_config(
    config: SampleSceneConfig,
    scene_mode: SceneMode,
) -> RetainedScene {
    build_settings_panel_scene_parts(config, scene_mode).0
}

fn build_settings_panel_scene_base(
    config: SampleSceneConfig,
    scene_mode: SceneMode,
) -> RetainedScene {
    let mut scene = RetainedScene::new(scene_mode);
    let root = scene.root();
    assert!(scene.set_bounds(root, Rect::new(0.0, 0.0, config.width, config.height)));
    let preferred = match scene_mode {
        SceneMode::UiPhysical => [392.0, 224.0],
        _ => [320.0, 188.0],
    };
    let panel = fit_panel(config, preferred, [28.0, 24.0]);
    let physical_padding = if scene_mode == SceneMode::UiPhysical {
        [6.0, 6.0]
    } else {
        [0.0, 0.0]
    };
    let title_x = panel.x + 24.0 + physical_padding[0];
    let title_y = panel.y + 12.0 + physical_padding[1];
    let separator_y = panel.y + (panel.height * 0.37).min(70.0) + physical_padding[1];
    let footer_y = panel.y + panel.height - 30.0 - physical_padding[1];

    scene
        .append_node(
            root,
            RenderNodeDescriptor::new(RenderNodeKind::Primitive, ElementKind::Shadow, panel)
                .named("panel_shadow")
                .with_ui_visual_role(UiVisualRole::BoxShadow)
                .with_ui_visual_style(panel_shadow_style(scene_mode)),
        )
        .expect("panel shadow");

    scene
        .append_node(
            root,
            RenderNodeDescriptor::new(RenderNodeKind::Primitive, ElementKind::Panel, panel)
                .named("panel_fill")
                .with_ui_visual_role(UiVisualRole::FilledSurface)
                .with_ui_visual_style(panel_fill_style(scene_mode))
                .with_material(6.0, 10.0, 18.0),
        )
        .expect("panel fill");

    scene
        .append_node(
            root,
            RenderNodeDescriptor::new(
                RenderNodeKind::Primitive,
                ElementKind::Separator,
                Rect::new(panel.x + 24.0, separator_y, panel.width - 48.0, 1.0),
            )
            .named("panel_separator")
            .with_ui_visual_role(UiVisualRole::SeparatorLine)
            .with_ui_visual_style(separator_style()),
        )
        .expect("panel separator");

    scene
        .append_node(
            root,
            RenderNodeDescriptor::new(
                RenderNodeKind::Primitive,
                ElementKind::Heading,
                Rect::new(title_x, title_y, (panel.width - 48.0).min(170.0), 24.0),
            )
            .named("panel_title")
            .with_text("Release settings", 21.0)
            .with_text_role(TextRole::Heading),
        )
        .expect("panel title");

    scene
        .append_node(
            root,
            RenderNodeDescriptor::new(
                RenderNodeKind::Primitive,
                ElementKind::Checkbox,
                Rect::new(
                    panel.x + 24.0 + physical_padding[0],
                    separator_y - 38.0,
                    34.0,
                    34.0,
                ),
            )
            .named("notifications_checkbox")
            .with_ui_primitive_count(2)
            .with_toggle_state(ToggleState::On)
            .with_ui_visual_role(UiVisualRole::CheckboxControl)
            .with_ui_visual_style(checkbox_style()),
        )
        .expect("checkbox");

    scene
        .append_node(
            root,
            RenderNodeDescriptor::new(
                RenderNodeKind::Primitive,
                ElementKind::Text,
                Rect::new(
                    panel.x + 70.0 + physical_padding[0],
                    separator_y - 34.0,
                    (panel.width - (94.0 + physical_padding[0] * 2.0)).max(120.0),
                    24.0,
                ),
            )
            .named("completed_label")
            .with_ui_primitive_count(1)
            .with_text("Enable release checklist", 20.0)
            .with_text_role(TextRole::Completed)
            .with_ui_visual_role(UiVisualRole::CompletedTextDecoration)
            .with_ui_visual_style(strike_style()),
        )
        .expect("completed label");

    scene
        .append_node(
            root,
            RenderNodeDescriptor::new(
                RenderNodeKind::Primitive,
                ElementKind::Text,
                Rect::new(
                    panel.x + 24.0 + physical_padding[0],
                    separator_y + 8.0,
                    panel.width - (48.0 + physical_padding[0] * 2.0),
                    18.0,
                ),
            )
            .named("status_line")
            .with_text("Notify the team before shipping", 15.0)
            .with_text_role(TextRole::Body),
        )
        .expect("status line");

    scene
        .append_node(
            root,
            RenderNodeDescriptor::new(
                RenderNodeKind::Primitive,
                ElementKind::Button,
                Rect::new(
                    panel.x + 28.0 + physical_padding[0],
                    separator_y + 38.0,
                    98.0,
                    30.0,
                ),
            )
            .named("selected_filter")
            .with_ui_primitive_count(1)
            .with_selection_state(SelectionState::Selected)
            .with_ui_visual_role(UiVisualRole::SelectionOutline)
            .with_ui_visual_style(selection_style()),
        )
        .expect("selected button");

    scene
        .append_node(
            root,
            RenderNodeDescriptor::new(
                RenderNodeKind::Primitive,
                ElementKind::Text,
                Rect::new(
                    panel.x + 46.0 + physical_padding[0],
                    separator_y + 45.0,
                    60.0,
                    16.0,
                ),
            )
            .named("button_label")
            .with_text("Primary", 14.0)
            .with_text_role(TextRole::Body),
        )
        .expect("button label");

    scene
        .append_node(
            root,
            RenderNodeDescriptor::new(
                RenderNodeKind::Primitive,
                ElementKind::Info,
                Rect::new(
                    panel.x + 24.0 + physical_padding[0],
                    footer_y,
                    panel.width - (48.0 + physical_padding[0] * 2.0),
                    16.0,
                ),
            )
            .named("footer_info")
            .with_text("Last synced 2 min ago", 13.0)
            .with_text_role(TextRole::Info),
        )
        .expect("footer info");

    scene
}

pub fn build_scrolling_feed_scene() -> RetainedScene {
    build_scrolling_feed_scene_with_config(SampleSceneConfig::default(), SceneMode::Ui2D)
}

fn build_scrolling_feed_scene_parts(
    config: SampleSceneConfig,
    scene_mode: SceneMode,
) -> (RetainedScene, OwnedTextRunLayout) {
    let mut scene = build_scrolling_feed_scene_base(config, scene_mode);
    assign_dynamic_ui_slots(&mut scene);
    let layout = assign_text_slots_and_build_layout(
        &mut scene,
        sample_grid_spec(config.width, config.height),
        12,
    );
    (scene, layout)
}

pub fn build_scrolling_feed_scene_with_config(
    config: SampleSceneConfig,
    scene_mode: SceneMode,
) -> RetainedScene {
    build_scrolling_feed_scene_parts(config, scene_mode).0
}

fn build_scrolling_feed_scene_base(
    config: SampleSceneConfig,
    scene_mode: SceneMode,
) -> RetainedScene {
    let mut scene = RetainedScene::new(scene_mode);
    let root = scene.root();
    assert!(scene.set_bounds(root, Rect::new(0.0, 0.0, config.width, config.height)));
    let preferred = match scene_mode {
        SceneMode::UiPhysical => [428.0, 252.0],
        _ => [360.0, 212.0],
    };
    let panel = fit_panel(config, preferred, [24.0, 20.0]);
    let physical_padding = if scene_mode == SceneMode::UiPhysical {
        [6.0, 6.0]
    } else {
        [0.0, 0.0]
    };
    let title_x = panel.x + 24.0 + physical_padding[0];
    let title_y = panel.y + 14.0 + physical_padding[1];
    let separator_y = title_y + 30.0;
    let clip_rect = inset_rect(
        panel,
        [24.0 + physical_padding[0], 58.0 + physical_padding[1]],
    );
    let footer_y = panel.y + panel.height - 28.0 - physical_padding[1];

    scene
        .append_node(
            root,
            RenderNodeDescriptor::new(RenderNodeKind::Primitive, ElementKind::Shadow, panel)
                .named("feed_shadow")
                .with_ui_visual_role(UiVisualRole::BoxShadow)
                .with_ui_visual_style(panel_shadow_style(scene_mode)),
        )
        .expect("feed shadow");

    scene
        .append_node(
            root,
            RenderNodeDescriptor::new(RenderNodeKind::Primitive, ElementKind::Panel, panel)
                .named("feed_panel")
                .with_ui_visual_role(UiVisualRole::FilledSurface)
                .with_ui_visual_style(panel_fill_style(scene_mode))
                .with_material(6.0, 10.0, 18.0),
        )
        .expect("feed panel");

    scene
        .append_node(
            root,
            RenderNodeDescriptor::new(
                RenderNodeKind::Primitive,
                ElementKind::Heading,
                Rect::new(title_x, title_y, (panel.width - 48.0).min(180.0), 22.0),
            )
            .named("feed_title")
            .with_text("Release feed", 20.0)
            .with_text_role(TextRole::Heading),
        )
        .expect("feed title");

    scene
        .append_node(
            root,
            RenderNodeDescriptor::new(
                RenderNodeKind::Primitive,
                ElementKind::Separator,
                Rect::new(title_x, separator_y, panel.width - 48.0, 1.0),
            )
            .named("feed_separator")
            .with_ui_visual_role(UiVisualRole::SeparatorLine)
            .with_ui_visual_style(separator_style()),
        )
        .expect("feed separator");

    let clip = scene
        .append_node(
            root,
            RenderNodeDescriptor::new(RenderNodeKind::Clip, ElementKind::Clip, clip_rect)
                .named("feed_clip")
                .with_clip()
                .with_ui_visual_role(UiVisualRole::OutlineRect)
                .with_ui_visual_style(outline_style(scene_mode))
                .with_material(6.0, 10.0, 18.0),
        )
        .expect("feed clip");

    let scroll_root = scene
        .append_node(
            clip,
            RenderNodeDescriptor::new(
                RenderNodeKind::ScrollRoot,
                ElementKind::ScrollContainer,
                clip_rect,
            )
            .named("feed_scroll")
            .with_scroll(ScrollState {
                offset: [0.0, 54.0],
                viewport_size: [clip_rect.width, clip_rect.height],
                content_size: [clip_rect.width, 230.0],
            }),
        )
        .expect("feed scroll root");

    for (index, (title, info)) in FEED_ROWS.iter().enumerate() {
        let base_y = clip_rect.y + index as f32 * 42.0;
        scene
            .append_node(
                scroll_root,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Text,
                    Rect::new(
                        clip_rect.x,
                        base_y,
                        (clip_rect.width - 52.0).max(160.0),
                        18.0,
                    ),
                )
                .named(format!("feed_row_{index}_title"))
                .with_text(*title, 15.0)
                .with_text_role(TextRole::Body),
            )
            .expect("feed row title");

        scene
            .append_node(
                scroll_root,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Info,
                    Rect::new(
                        clip_rect.x,
                        base_y + 18.0,
                        (clip_rect.width - 32.0).max(180.0),
                        15.0,
                    ),
                )
                .named(format!("feed_row_{index}_info"))
                .with_text(*info, 12.0)
                .with_text_role(TextRole::Info),
            )
            .expect("feed row info");
    }

    scene
        .append_node(
            root,
            RenderNodeDescriptor::new(
                RenderNodeKind::Primitive,
                ElementKind::Info,
                Rect::new(title_x, footer_y, panel.width - 48.0, 16.0),
            )
            .named("feed_footer")
            .with_text("Only visible rows are realized", 13.0)
            .with_text_role(TextRole::Info),
        )
        .expect("feed footer");

    scene
}

pub fn set_settings_panel_enabled(scene: &mut RetainedScene, enabled: bool) -> bool {
    let Some(checkbox_id) = scene
        .node_named("notifications_checkbox")
        .map(|node| node.id)
    else {
        return false;
    };
    let Some(label_id) = scene.node_named("completed_label").map(|node| node.id) else {
        return false;
    };
    let Some(status_id) = scene.node_named("status_line").map(|node| node.id) else {
        return false;
    };
    let Some(button_id) = scene.node_named("selected_filter").map(|node| node.id) else {
        return false;
    };

    let current_enabled =
        scene.node(checkbox_id).and_then(|node| node.toggle_state) == Some(ToggleState::On);
    if current_enabled == enabled {
        return false;
    }

    let status_text = if enabled {
        "Notify the team before shipping"
    } else {
        "Notifications paused for this release"
    };

    let toggle_changed = scene.set_toggle_state(
        checkbox_id,
        Some(if enabled {
            ToggleState::On
        } else {
            ToggleState::Off
        }),
    );
    let label_changed = scene.set_text_role(
        label_id,
        Some(if enabled {
            TextRole::Completed
        } else {
            TextRole::Body
        }),
    );
    let status_changed = scene.set_text(status_id, Some(TextNode::new(status_text, 15.0)));
    let selection_changed = scene.set_selection_state(
        button_id,
        Some(if enabled {
            SelectionState::Selected
        } else {
            SelectionState::Unselected
        }),
    );

    toggle_changed || label_changed || status_changed || selection_changed
}

pub fn toggle_settings_panel_state(scene: &mut RetainedScene) -> bool {
    let enabled = scene
        .node_named("notifications_checkbox")
        .and_then(|node| scene.node(node.id))
        .and_then(|node| node.toggle_state)
        == Some(ToggleState::On);
    set_settings_panel_enabled(scene, !enabled)
}

pub fn sample_scene_state_from_scene(
    kind: SampleSceneKind,
    scene: &RetainedScene,
) -> Option<SampleSceneState> {
    match kind {
        SampleSceneKind::SettingsPanel => Some(SampleSceneState::SettingsPanel {
            enabled: scene
                .node_named("notifications_checkbox")
                .and_then(|node| scene.node(node.id))
                .and_then(|node| node.toggle_state)
                == Some(ToggleState::On),
        }),
        SampleSceneKind::ScrollingFeed => scene
            .node_named("feed_scroll")
            .and_then(|node| scene.node(node.id))
            .and_then(|node| node.scroll)
            .map(|scroll| SampleSceneState::ScrollingFeed {
                scroll_offset: scroll.offset[1],
            }),
    }
}

pub fn set_scrolling_feed_offset(scene: &mut RetainedScene, offset_y: f32) -> bool {
    let Some(scroll_root) = scene.node_named("feed_scroll").map(|node| node.id) else {
        return false;
    };
    let Some(mut scroll) = scene.node(scroll_root).and_then(|node| node.scroll) else {
        return false;
    };

    let max_offset = (scroll.content_size[1] - scroll.viewport_size[1]).max(0.0);
    let next_offset = offset_y.clamp(0.0, max_offset);
    if next_offset == scroll.offset[1] {
        return false;
    }

    scroll.offset[1] = next_offset;
    scene.set_scroll_state(scroll_root, Some(scroll))
}

pub fn adjust_scrolling_feed_offset(scene: &mut RetainedScene, delta_y: f32) -> bool {
    let current_offset = scene
        .node_named("feed_scroll")
        .and_then(|node| scene.node(node.id))
        .and_then(|node| node.scroll)
        .map(|scroll| scroll.offset[1])
        .unwrap_or(0.0);
    set_scrolling_feed_offset(scene, current_offset + delta_y)
}

pub fn apply_sample_scene_state(scene: &mut RetainedScene, state: SampleSceneState) -> bool {
    match state {
        SampleSceneState::SettingsPanel { enabled } => set_settings_panel_enabled(scene, enabled),
        SampleSceneState::ScrollingFeed { scroll_offset } => {
            set_scrolling_feed_offset(scene, scroll_offset)
        }
    }
}

pub fn apply_sample_scene_action(
    kind: SampleSceneKind,
    scene: &mut RetainedScene,
    action: SampleSceneAction,
) -> bool {
    match (kind, action) {
        (SampleSceneKind::SettingsPanel, SampleSceneAction::ToggleSettingsPanel) => {
            toggle_settings_panel_state(scene)
        }
        (SampleSceneKind::ScrollingFeed, SampleSceneAction::AdjustFeedScroll(delta_y)) => {
            adjust_scrolling_feed_offset(scene, delta_y)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        adjust_scrolling_feed_offset, apply_sample_scene_action, apply_sample_scene_state,
        build_scrolling_feed_scene, build_scrolling_feed_scene_with_config,
        build_settings_panel_scene, build_settings_panel_scene_with_config,
        sample_scene_state_from_scene, set_scrolling_feed_offset, set_settings_panel_enabled,
        toggle_settings_panel_state, SampleSceneAction, SampleSceneConfig, SampleSceneKind,
        SampleSceneModel, SampleSceneState,
    };
    use crate::retained::text::{TextColors, TextRenderSpace};
    use crate::retained::ui::{
        build_gpu_ui_patches_for_slots, build_gpu_ui_scene, UiRenderSpace, PRIM_CHECKMARK_V,
        PRIM_LINE, PRIM_STROKED_CIRCLE, PRIM_STROKED_RECT,
    };
    use crate::retained::RetainedScene;
    use crate::retained::SceneMode;
    use crate::text::{VectorFont, VectorFontAtlas};
    use std::collections::BTreeSet;

    fn render_space() -> UiRenderSpace {
        UiRenderSpace {
            x_offset: 0.0,
            screen_height: 260.0,
        }
    }

    fn load_test_atlas() -> VectorFontAtlas {
        let font_data = std::fs::read("assets/fonts/DejaVuSans.ttf").expect("load test font");
        let font = VectorFont::from_ttf(&font_data).expect("parse test font");
        VectorFontAtlas::from_font(&font, 32)
    }

    fn text_colors() -> TextColors {
        TextColors {
            heading: [0.2, 0.2, 0.2],
            active: [0.3, 0.3, 0.3],
            completed: [0.4, 0.4, 0.4],
            placeholder: [0.5, 0.5, 0.5],
            body: [0.6, 0.6, 0.6],
            info: [0.7, 0.7, 0.7],
        }
    }

    fn ui_slot(scene: &RetainedScene, name: &str) -> u16 {
        scene
            .node_named(name)
            .and_then(|node| node.ui_slot)
            .expect("named ui slot")
    }

    #[test]
    fn sample_scene_builds_gpu_ui_from_non_todomvc_scene() {
        let scene = build_settings_panel_scene();
        let ui_scene = build_gpu_ui_scene(&scene, render_space());

        assert_eq!(ui_scene.primitive_count, 7);
        assert_eq!(
            ui_scene.primitives[0].params[3],
            super::super::ui::PRIM_BOX_SHADOW
        );
        assert_eq!(
            ui_scene.primitives[1].params[3],
            super::super::ui::PRIM_FILLED_RECT
        );
        assert_eq!(ui_scene.primitives[2].params[3], PRIM_LINE);
        assert_eq!(ui_scene.primitives[3].params[3], PRIM_STROKED_CIRCLE);
        assert_eq!(ui_scene.primitives[4].params[3], PRIM_CHECKMARK_V);
        assert_eq!(ui_scene.primitives[5].params[3], PRIM_LINE);
        assert_eq!(ui_scene.primitives[6].params[3], PRIM_STROKED_RECT);
    }

    #[test]
    fn sample_scene_slot_patches_match_full_scene_ranges() {
        let scene = build_settings_panel_scene();
        let full = build_gpu_ui_scene(&scene, render_space());
        let slots = BTreeSet::from([
            ui_slot(&scene, "notifications_checkbox"),
            ui_slot(&scene, "selected_filter"),
            ui_slot(&scene, "completed_label"),
        ]);
        let patches = build_gpu_ui_patches_for_slots(&scene, &slots, render_space());

        assert_eq!(patches.len(), 3);
        assert_eq!(patches[0].offset, 3);
        assert_eq!(patches[1].offset, 5);
        assert_eq!(patches[2].offset, 6);

        for patch in patches {
            assert_eq!(
                patch.primitives,
                full.primitives[patch.offset..patch.offset + patch.primitives.len()]
            );
        }
    }

    #[test]
    fn scrolling_feed_scene_builds_static_chrome() {
        let scene = build_scrolling_feed_scene();
        let ui_scene = build_gpu_ui_scene(&scene, render_space());

        assert_eq!(ui_scene.primitive_count, 4);
        assert_eq!(
            ui_scene.primitives[0].params[3],
            super::super::ui::PRIM_BOX_SHADOW
        );
        assert_eq!(
            ui_scene.primitives[1].params[3],
            super::super::ui::PRIM_FILLED_RECT
        );
        assert_eq!(ui_scene.primitives[2].params[3], PRIM_LINE);
        assert_eq!(ui_scene.primitives[3].params[3], PRIM_STROKED_RECT);
    }

    #[test]
    fn settings_panel_toggle_mutates_semantic_state() {
        let mut scene = build_settings_panel_scene();
        scene.clear_dirty();

        assert!(toggle_settings_panel_state(&mut scene));
        assert_eq!(
            scene
                .node_named("notifications_checkbox")
                .and_then(|node| node.toggle_state),
            Some(super::ToggleState::Off)
        );
        assert_eq!(
            scene
                .node_named("completed_label")
                .and_then(|node| node.text_role),
            Some(super::TextRole::Body)
        );
        assert_eq!(
            scene
                .node_named("status_line")
                .and_then(|node| node.text.as_ref())
                .map(|text| text.text.as_ref()),
            Some("Notifications paused for this release")
        );
        assert_eq!(
            scene
                .node_named("selected_filter")
                .and_then(|node| node.selection_state),
            Some(super::SelectionState::Unselected)
        );
    }

    #[test]
    fn scrolling_feed_offset_mutates_scroll_state() {
        let mut scene = build_scrolling_feed_scene();
        scene.clear_dirty();

        assert!(adjust_scrolling_feed_offset(&mut scene, 24.0));
        assert_eq!(
            scene
                .node_named("feed_scroll")
                .and_then(|node| node.scroll)
                .map(|scroll| scroll.offset[1]),
            Some(78.0)
        );
    }

    #[test]
    fn settings_panel_enabled_setter_applies_exact_state() {
        let mut scene = build_settings_panel_scene();
        scene.clear_dirty();

        assert!(set_settings_panel_enabled(&mut scene, false));
        assert_eq!(
            scene
                .node_named("notifications_checkbox")
                .and_then(|node| node.toggle_state),
            Some(super::ToggleState::Off)
        );
        assert!(!set_settings_panel_enabled(&mut scene, false));
        assert!(set_settings_panel_enabled(&mut scene, true));
    }

    #[test]
    fn scrolling_feed_offset_setter_applies_exact_offset() {
        let mut scene = build_scrolling_feed_scene();
        scene.clear_dirty();

        assert!(set_scrolling_feed_offset(&mut scene, 120.0));
        let expected_offset = scene
            .node_named("feed_scroll")
            .and_then(|node| node.scroll)
            .map(|scroll| {
                (120.0f32).clamp(
                    0.0,
                    (scroll.content_size[1] - scroll.viewport_size[1]).max(0.0),
                )
            })
            .unwrap_or(0.0);
        assert_eq!(
            scene
                .node_named("feed_scroll")
                .and_then(|node| node.scroll)
                .map(|scroll| scroll.offset[1]),
            Some(expected_offset)
        );
        assert!(!set_scrolling_feed_offset(&mut scene, expected_offset));
    }

    #[test]
    fn sample_scene_state_helpers_round_trip_settings_panel_state() {
        let mut scene = build_settings_panel_scene();
        assert!(apply_sample_scene_state(
            &mut scene,
            SampleSceneState::SettingsPanel { enabled: false }
        ));
        assert_eq!(
            sample_scene_state_from_scene(SampleSceneKind::SettingsPanel, &scene),
            Some(SampleSceneState::SettingsPanel { enabled: false })
        );
    }

    #[test]
    fn sample_scene_state_helpers_round_trip_scrolling_feed_state() {
        let mut scene = build_scrolling_feed_scene();
        assert!(apply_sample_scene_state(
            &mut scene,
            SampleSceneState::ScrollingFeed {
                scroll_offset: 120.0
            }
        ));
        let expected_offset = scene
            .node_named("feed_scroll")
            .and_then(|node| node.scroll)
            .map(|scroll| scroll.offset[1])
            .expect("scroll state");
        assert_eq!(
            sample_scene_state_from_scene(SampleSceneKind::ScrollingFeed, &scene),
            Some(SampleSceneState::ScrollingFeed {
                scroll_offset: expected_offset,
            })
        );
    }

    #[test]
    fn sample_scene_model_build_scene_applies_saved_semantic_state() {
        let model = SampleSceneModel {
            scene_mode: SceneMode::Ui2D,
            kind: SampleSceneKind::SettingsPanel,
            state: SampleSceneState::SettingsPanel { enabled: false },
        };
        let (scene, _) = model.build_scene(SampleSceneConfig::default());
        assert_eq!(
            sample_scene_state_from_scene(SampleSceneKind::SettingsPanel, &scene),
            Some(SampleSceneState::SettingsPanel { enabled: false })
        );
    }

    #[test]
    fn sample_scene_model_builds_at_large_viewport_without_grid_overflow() {
        let atlas = load_test_atlas();
        let scene = SampleSceneModel::new(SampleSceneKind::SettingsPanel).build_fixed_ui2d_scene(
            SampleSceneConfig {
                width: 1920.0,
                height: 1080.0,
            },
            &atlas,
            &text_colors(),
            TextRenderSpace {
                x_offset: 0.0,
                screen_height: 1080.0,
                italic_codepoint_offset: 0x10000,
            },
            UiRenderSpace {
                x_offset: 0.0,
                screen_height: 1080.0,
            },
        );

        assert!(scene.init.text_data.char_count > 0);
        assert!(scene.init.ui_data.primitive_count > 0);
    }

    #[test]
    fn sample_scene_action_routes_by_scene_kind() {
        let mut settings = build_settings_panel_scene();
        settings.clear_dirty();
        assert!(apply_sample_scene_action(
            SampleSceneKind::SettingsPanel,
            &mut settings,
            SampleSceneAction::ToggleSettingsPanel,
        ));
        assert_eq!(
            sample_scene_state_from_scene(SampleSceneKind::SettingsPanel, &settings),
            Some(SampleSceneState::SettingsPanel { enabled: false })
        );

        let mut feed = build_scrolling_feed_scene();
        feed.clear_dirty();
        assert!(apply_sample_scene_action(
            SampleSceneKind::ScrollingFeed,
            &mut feed,
            SampleSceneAction::AdjustFeedScroll(24.0),
        ));
        assert_eq!(
            sample_scene_state_from_scene(SampleSceneKind::ScrollingFeed, &feed),
            Some(SampleSceneState::ScrollingFeed {
                scroll_offset: 78.0
            })
        );

        assert!(!apply_sample_scene_action(
            SampleSceneKind::SettingsPanel,
            &mut settings,
            SampleSceneAction::AdjustFeedScroll(24.0),
        ));
    }

    #[test]
    fn settings_panel_scene_respects_configured_viewport_size() {
        let scene = build_settings_panel_scene_with_config(
            SampleSceneConfig {
                width: 320.0,
                height: 220.0,
            },
            SceneMode::Ui2D,
        );

        assert_eq!(
            scene
                .node(scene.root())
                .map(|node| (node.bounds.width, node.bounds.height)),
            Some((320.0, 220.0))
        );
        let panel = scene.node_named("panel_fill").expect("panel fill");
        assert!(panel.bounds.width <= 320.0);
        assert!(panel.bounds.height <= 220.0);
    }

    #[test]
    fn sample_scene_model_respects_scene_mode() {
        let (scene, _) =
            SampleSceneModel::new_with_mode(SampleSceneKind::SettingsPanel, SceneMode::UiPhysical)
                .build_scene(SampleSceneConfig::default());

        assert_eq!(scene.mode(), SceneMode::UiPhysical);
    }

    #[test]
    fn scrolling_feed_scene_respects_configured_viewport_size() {
        let scene = build_scrolling_feed_scene_with_config(
            SampleSceneConfig {
                width: 300.0,
                height: 200.0,
            },
            SceneMode::Ui2D,
        );

        assert_eq!(
            scene
                .node(scene.root())
                .map(|node| (node.bounds.width, node.bounds.height)),
            Some((300.0, 200.0))
        );
        let clip = scene.node_named("feed_clip").expect("feed clip");
        let scroll = scene
            .node_named("feed_scroll")
            .and_then(|node| node.scroll)
            .expect("feed scroll state");
        assert_eq!(
            scroll.viewport_size,
            [clip.bounds.width, clip.bounds.height]
        );
        assert!(clip.bounds.width <= 300.0);
        assert!(clip.bounds.height <= 200.0);
    }
}
