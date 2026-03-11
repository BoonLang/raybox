use super::text::{
    build_fixed_text_scene_state_for_scene, rebuild_fixed_text_scene_state_for_scene,
    update_fixed_text_scene_slots_for_scene, FixedTextRunLayout, FixedTextSceneData,
    FixedTextScenePatch, FixedTextSceneState, TextColors, TextRenderSpace,
};
use super::ui::{
    build_gpu_ui_patches_for_slots, build_gpu_ui_scene, GpuUiPatch, GpuUiSceneData, UiRenderSpace,
};
use super::RetainedScene;
use crate::text::VectorFontAtlas;

pub struct FixedUi2dSceneState {
    pub scene: RetainedScene,
    pub text_state: FixedTextSceneState,
}

pub struct FixedUi2dSceneInit {
    pub text_data: FixedTextSceneData,
    pub ui_data: GpuUiSceneData,
}

pub struct BuiltFixedUi2dScene {
    pub state: FixedUi2dSceneState,
    pub init: FixedUi2dSceneInit,
}

pub trait FixedUi2dSceneModelBuilder {
    fn build_fixed_ui2d_scene(
        &self,
        viewport_size: [u32; 2],
        atlas: &VectorFontAtlas,
        colors: &TextColors,
        text_space: TextRenderSpace,
        ui_space: UiRenderSpace,
    ) -> BuiltFixedUi2dScene;
}

pub trait FixedUi2dSceneModelCapture {
    fn capture_from_scene(&mut self, scene: &RetainedScene);
}

pub enum FixedUi2dSceneUpdate {
    Full {
        text_data: FixedTextSceneData,
        ui_data: GpuUiSceneData,
    },
    Partial {
        ui_patches: Vec<GpuUiPatch>,
        text_patch: Option<FixedTextScenePatch>,
    },
}

impl FixedUi2dSceneState {
    pub fn new(
        scene: RetainedScene,
        layout: FixedTextRunLayout<'_>,
        atlas: &VectorFontAtlas,
        colors: &TextColors,
        text_space: TextRenderSpace,
        ui_space: UiRenderSpace,
    ) -> (Self, FixedUi2dSceneInit) {
        let ui_data = build_gpu_ui_scene(&scene, ui_space);
        let (text_state, text_data) =
            build_fixed_text_scene_state_for_scene(&scene, layout, atlas, colors, text_space);
        (
            Self { scene, text_state },
            FixedUi2dSceneInit { text_data, ui_data },
        )
    }

    pub fn clear_dirty(&mut self) {
        self.scene.clear_dirty();
    }

    pub fn take_update(
        &mut self,
        atlas: &VectorFontAtlas,
        colors: &TextColors,
        text_space: TextRenderSpace,
        ui_space: UiRenderSpace,
    ) -> Option<FixedUi2dSceneUpdate> {
        let dirty = self.scene.take_dirty();
        if dirty.is_empty() {
            return None;
        }

        let resources = self.scene.classify_resource_dirty(&dirty);
        if resources.full_text || resources.full_ui {
            let ui_data = build_gpu_ui_scene(&self.scene, ui_space);
            let text_data = rebuild_fixed_text_scene_state_for_scene(
                &mut self.text_state,
                &self.scene,
                atlas,
                colors,
                text_space,
            );
            return Some(FixedUi2dSceneUpdate::Full { text_data, ui_data });
        }

        let ui_patches = if resources.ui_slots.is_empty() {
            Vec::new()
        } else {
            build_gpu_ui_patches_for_slots(&self.scene, &resources.ui_slots, ui_space)
        };

        let text_patch = if resources.text_slots.is_empty() {
            None
        } else {
            Some(update_fixed_text_scene_slots_for_scene(
                &mut self.text_state,
                &self.scene,
                atlas,
                resources.text_slots,
                colors,
                text_space,
            ))
        };

        Some(FixedUi2dSceneUpdate::Partial {
            ui_patches,
            text_patch,
        })
    }
}

pub fn build_fixed_ui2d_scene(
    scene: RetainedScene,
    layout: FixedTextRunLayout<'_>,
    atlas: &VectorFontAtlas,
    colors: &TextColors,
    text_space: TextRenderSpace,
    ui_space: UiRenderSpace,
) -> BuiltFixedUi2dScene {
    let (state, init) =
        FixedUi2dSceneState::new(scene, layout, atlas, colors, text_space, ui_space);
    BuiltFixedUi2dScene { state, init }
}

#[cfg(test)]
mod tests {
    use super::{FixedUi2dSceneState, FixedUi2dSceneUpdate};
    use crate::retained::samples::{
        adjust_scrolling_feed_offset, build_scrolling_feed_scene, build_settings_panel_scene,
        sample_text_run_layout, scrolling_feed_text_run_layout, toggle_settings_panel_state,
    };
    use crate::retained::text::{TextColors, TextRenderSpace};
    use crate::retained::ui::UiRenderSpace;
    use crate::text::{VectorFont, VectorFontAtlas};

    fn load_test_atlas() -> VectorFontAtlas {
        let font_data = std::fs::read("assets/fonts/DejaVuSans.ttf").expect("load test font");
        let font = VectorFont::from_ttf(&font_data).expect("parse test font");
        VectorFontAtlas::from_font(&font, 32)
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

    fn text_space() -> TextRenderSpace {
        TextRenderSpace {
            x_offset: 0.0,
            screen_height: 260.0,
            italic_codepoint_offset: 0x10000,
        }
    }

    fn ui_space() -> UiRenderSpace {
        UiRenderSpace {
            x_offset: 0.0,
            screen_height: 260.0,
        }
    }

    #[test]
    fn fixed_ui2d_scene_state_produces_partial_update_for_text_and_ui_mutation() {
        let atlas = load_test_atlas();
        let layout = sample_text_run_layout();
        let (mut state, init) = FixedUi2dSceneState::new(
            build_settings_panel_scene(),
            layout.layout(),
            &atlas,
            &colors(),
            text_space(),
            ui_space(),
        );
        assert!(init.text_data.char_count > 0);
        assert!(init.ui_data.primitive_count > 0);
        state.clear_dirty();

        assert!(toggle_settings_panel_state(&mut state.scene));
        let Some(update) = state.take_update(&atlas, &colors(), text_space(), ui_space()) else {
            panic!("expected retained update");
        };

        match update {
            FixedUi2dSceneUpdate::Partial {
                ui_patches,
                text_patch,
            } => {
                assert!(!ui_patches.is_empty());
                assert!(text_patch.is_some());
            }
            FixedUi2dSceneUpdate::Full { .. } => panic!("expected partial update"),
        }
    }

    #[test]
    fn fixed_ui2d_scene_state_produces_partial_update_for_scroll_mutation() {
        let atlas = load_test_atlas();
        let layout = scrolling_feed_text_run_layout();
        let (mut state, _) = FixedUi2dSceneState::new(
            build_scrolling_feed_scene(),
            layout.layout(),
            &atlas,
            &colors(),
            text_space(),
            ui_space(),
        );
        state.clear_dirty();

        assert!(adjust_scrolling_feed_offset(&mut state.scene, 24.0));
        let expected_slots = state.scene.classify_resource_dirty(state.scene.dirty());
        let Some(update) = state.take_update(&atlas, &colors(), text_space(), ui_space()) else {
            panic!("expected retained update");
        };

        match update {
            FixedUi2dSceneUpdate::Partial { text_patch, .. } => match text_patch {
                Some(text_patch) => {
                    assert_eq!(
                        text_patch.run_updates.len(),
                        expected_slots.text_slots.len()
                    );
                    assert!(text_patch.run_updates.len() < layout.run_capacities().len());
                }
                None => panic!("expected text patch"),
            },
            FixedUi2dSceneUpdate::Full { .. } => panic!("expected partial update"),
        }
    }
}
