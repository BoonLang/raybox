use crate::retained::fixed_scene::FixedUi2dSceneUpdate;
use crate::retained::text::{
    FixedTextRuntimeUpdate, FixedTextSceneData, FixedTextSceneState, TextColors,
};
use crate::retained::ui::{GpuUiRuntimeUpdate, GpuUiSceneData};
use crate::retained::RetainedScene;
use crate::text::VectorFontAtlas;

pub struct PreparedRetainedUiScene {
    pub text_data: FixedTextSceneData,
    pub ui_data: GpuUiSceneData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ui2dSceneInitCapacities {
    pub text_capacity: usize,
    pub grid_index_capacity: usize,
    pub primitive_capacity: usize,
}

pub enum SharedRetainedUiUpdate {
    Full(PreparedRetainedUiScene),
    Partial {
        text: Option<FixedTextRuntimeUpdate>,
        ui: Option<GpuUiRuntimeUpdate>,
    },
}

impl SharedRetainedUiUpdate {
    pub fn needs_full_rebuild(&self) -> bool {
        matches!(self, Self::Full(_))
    }
}

impl From<FixedUi2dSceneUpdate> for SharedRetainedUiUpdate {
    fn from(update: FixedUi2dSceneUpdate) -> Self {
        match update {
            FixedUi2dSceneUpdate::Full { text_data, ui_data } => {
                Self::Full(PreparedRetainedUiScene { text_data, ui_data })
            }
            FixedUi2dSceneUpdate::Partial {
                ui_patches,
                text_patch,
            } => Self::Partial {
                text: text_patch.map(FixedTextRuntimeUpdate::Partial),
                ui: (!ui_patches.is_empty()).then_some(GpuUiRuntimeUpdate::Partial(ui_patches)),
            },
        }
    }
}

pub trait SharedRetainedUiSceneState {
    fn shared_atlas(&self) -> &VectorFontAtlas;
    fn shared_text_state(&self) -> &FixedTextSceneState;
    fn build_prepared_retained_ui_scene(&self, colors: &TextColors) -> PreparedRetainedUiScene;
    fn take_prepared_retained_ui_update(
        &mut self,
        colors: &TextColors,
    ) -> Option<SharedRetainedUiUpdate>;
    fn ui2d_scene_init_capacities(
        &self,
        prepared: &PreparedRetainedUiScene,
    ) -> Ui2dSceneInitCapacities;
    fn mark_retained_ui_view_transform_dirty(&mut self);
    fn set_retained_ui_viewport_size(&mut self, width: u32, height: u32);
    fn shared_scene(&self) -> &RetainedScene;
    fn shared_scene_mut(&mut self) -> &mut RetainedScene;
}
