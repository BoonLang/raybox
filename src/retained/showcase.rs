use super::fixed_scene::{
    BuiltFixedUi2dScene, FixedUi2dSceneModelBuilder, FixedUi2dSceneModelCapture,
};
use super::samples::{SampleSceneAction, SampleSceneConfig, SampleSceneKind, SampleSceneModel};
use super::text::TextColors;
use super::text_scene::{OwnedTextSceneBlock, WrappedTextSceneModel};
use super::ui::UiRenderSpace;
use super::{NamedScrollSceneModel, RetainedScene, SceneMode, TextRole};
use crate::retained::text::TextRenderSpace;
use crate::text::VectorFontAtlas;

const SHOWCASE_TEXT: &str = "Retained UI scenes should stay cheap while idle, carry their semantic state through rebuilds, and let the runtime choose how to realize the scene. This wrapped text scene exists to exercise that shared path outside TodoMVC.\n\nScrolling and scene mutation should work through the same retained model layer in both Ui2D and UiPhysical.";

#[derive(Clone)]
pub enum ShowcaseSceneModel {
    Sample(SampleSceneModel),
    WrappedText(WrappedTextSceneModel),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShowcaseSceneAction {
    TogglePrimaryState,
    AdjustPrimaryScroll(f32),
}

pub trait ShowcaseSceneDeckTarget {
    fn cycle_showcase_scene(&mut self) -> bool;
    fn apply_active_showcase_action(&mut self, action: ShowcaseSceneAction) -> bool;

    fn toggle_active_showcase_state(&mut self) -> bool {
        self.apply_active_showcase_action(ShowcaseSceneAction::TogglePrimaryState)
    }

    fn adjust_active_showcase_scroll(&mut self, delta_y: f32) -> bool {
        self.apply_active_showcase_action(ShowcaseSceneAction::AdjustPrimaryScroll(delta_y))
    }
}

impl ShowcaseSceneModel {
    pub fn default_deck_models(scene_mode: SceneMode) -> Vec<Self> {
        vec![
            Self::settings_panel(scene_mode),
            Self::scrolling_feed(scene_mode),
            Self::wrapped_text(scene_mode),
        ]
    }

    pub fn settings_panel(scene_mode: SceneMode) -> Self {
        Self::Sample(SampleSceneModel::new_with_mode(
            SampleSceneKind::SettingsPanel,
            scene_mode,
        ))
    }

    pub fn scrolling_feed(scene_mode: SceneMode) -> Self {
        Self::Sample(SampleSceneModel::new_with_mode(
            SampleSceneKind::ScrollingFeed,
            scene_mode,
        ))
    }

    pub fn wrapped_text(scene_mode: SceneMode) -> Self {
        Self::WrappedText(WrappedTextSceneModel {
            scene_mode,
            heading: Some(OwnedTextSceneBlock {
                text: "RETAINED UI SHOWCASE".to_string(),
                font_size: 28.0,
                role: TextRole::Heading,
            }),
            body: OwnedTextSceneBlock {
                text: SHOWCASE_TEXT.to_string(),
                font_size: 16.0,
                role: TextRole::Body,
            },
            frame_size: None,
            margin: 20.0,
            body_line_height: 22.0,
            body_top_padding: 12.0,
            scroll_offset: 0.0,
            grid_dims: [64, 48],
            grid_cell_capacity: 64,
            clip_name: "showcase_text_clip",
            scroll_name: "showcase_text_scroll",
            heading_name: "showcase_text_heading",
            line_name_prefix: "showcase_text_line_",
        })
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Sample(model) => model.label(),
            Self::WrappedText(_) => "Retained UI Text",
        }
    }

    pub fn apply_action(&self, scene: &mut RetainedScene, action: ShowcaseSceneAction) -> bool {
        match (self, action) {
            (Self::Sample(model), ShowcaseSceneAction::TogglePrimaryState) => {
                model.apply_action(scene, SampleSceneAction::ToggleSettingsPanel)
            }
            (Self::Sample(model), ShowcaseSceneAction::AdjustPrimaryScroll(delta)) => {
                model.apply_action(scene, SampleSceneAction::AdjustFeedScroll(delta))
            }
            (Self::WrappedText(model), ShowcaseSceneAction::TogglePrimaryState) => {
                model.toggle_heading_emphasis(scene)
            }
            (Self::WrappedText(model), ShowcaseSceneAction::AdjustPrimaryScroll(delta)) => {
                model.adjust_scroll(scene, delta)
            }
        }
    }
}

pub fn showcase_text_colors() -> TextColors {
    TextColors {
        heading: [0.13, 0.16, 0.23],
        active: [0.20, 0.24, 0.31],
        completed: [0.39, 0.45, 0.55],
        placeholder: [0.58, 0.64, 0.72],
        body: [0.23, 0.29, 0.36],
        info: [0.42, 0.48, 0.58],
    }
}

impl NamedScrollSceneModel for ShowcaseSceneModel {
    fn set_named_scroll_offset(
        &self,
        scene: &mut RetainedScene,
        name: &str,
        offset_y: f32,
    ) -> bool {
        match self {
            Self::Sample(model) => model.set_named_scroll_offset(scene, name, offset_y),
            Self::WrappedText(model) => model.set_named_scroll_offset(scene, name, offset_y),
        }
    }
}

impl FixedUi2dSceneModelBuilder for ShowcaseSceneModel {
    fn build_fixed_ui2d_scene(
        &self,
        viewport_size: [u32; 2],
        atlas: &VectorFontAtlas,
        colors: &TextColors,
        text_space: TextRenderSpace,
        ui_space: UiRenderSpace,
    ) -> BuiltFixedUi2dScene {
        match self {
            Self::Sample(model) => model.build_fixed_ui2d_scene(
                SampleSceneConfig {
                    width: viewport_size[0] as f32,
                    height: viewport_size[1] as f32,
                },
                atlas,
                colors,
                text_space,
                ui_space,
            ),
            Self::WrappedText(model) => model.build_fixed_ui2d_scene(
                atlas,
                viewport_size[0] as f32,
                viewport_size[1] as f32,
                colors,
                text_space,
                ui_space,
            ),
        }
    }
}

impl FixedUi2dSceneModelCapture for ShowcaseSceneModel {
    fn capture_from_scene(&mut self, scene: &RetainedScene) {
        match self {
            Self::Sample(model) => model.capture_from_scene(scene),
            Self::WrappedText(model) => model.capture_from_scene(scene),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ShowcaseSceneModel;
    use crate::retained::fixed_scene::FixedUi2dSceneModelBuilder;
    use crate::retained::text::{TextColors, TextRenderSpace};
    use crate::retained::ui::UiRenderSpace;
    use crate::retained::SceneMode;
    use crate::text::{VectorFont, VectorFontAtlas};

    fn load_test_atlas() -> VectorFontAtlas {
        let font_data = std::fs::read("assets/fonts/DejaVuSans.ttf").expect("load test font");
        let font = VectorFont::from_ttf(&font_data).expect("parse test font");
        VectorFontAtlas::from_font(&font)
    }

    fn colors() -> TextColors {
        TextColors {
            heading: [0.2, 0.2, 0.2],
            active: [0.3, 0.3, 0.3],
            completed: [0.4, 0.4, 0.4],
            placeholder: [0.5, 0.5, 0.5],
            body: [0.6, 0.6, 0.6],
            info: [0.7, 0.7, 0.7],
        }
    }

    #[test]
    fn wrapped_text_showcase_builds_without_grid_overflow() {
        let atlas = load_test_atlas();
        let scene = ShowcaseSceneModel::wrapped_text(SceneMode::Ui2D).build_fixed_ui2d_scene(
            [1280, 720],
            &atlas,
            &colors(),
            TextRenderSpace {
                x_offset: 0.0,
                screen_height: 720.0,
                italic_codepoint_offset: 0x10000,
            },
            UiRenderSpace {
                x_offset: 0.0,
                screen_height: 720.0,
            },
        );

        assert!(scene.init.text_data.char_count > 0);
    }

    #[test]
    fn physical_showcase_models_build_physical_scenes() {
        let model = ShowcaseSceneModel::default_deck_models(SceneMode::UiPhysical)
            .into_iter()
            .next()
            .expect("showcase scene");
        let atlas = load_test_atlas();
        let built = model.build_fixed_ui2d_scene(
            [800, 600],
            &atlas,
            &colors(),
            TextRenderSpace {
                x_offset: 0.0,
                screen_height: 600.0,
                italic_codepoint_offset: 0x10000,
            },
            UiRenderSpace {
                x_offset: 0.0,
                screen_height: 600.0,
            },
        );

        assert_eq!(built.state.scene.mode(), SceneMode::UiPhysical);
    }
}
