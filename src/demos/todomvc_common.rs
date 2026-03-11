use super::{
    gpu_runtime_common::{load_shared_vector_font_atlas, ITALIC_CODEPOINT_OFFSET},
    ui2d_runtime::{
        StateBackedUi2dHost, StateBackedUi2dSceneDeck, Ui2dRuntimeTextUpdate, Ui2dRuntimeUiUpdate,
        Ui2dRuntimeUpdate, Ui2dSceneInitData, Ui2dSceneState,
    },
    ui_physical_runtime::{
        StateBackedUiPhysicalHost, StateBackedUiPhysicalSceneDeck, UiPhysicalGeometryMode,
        UiPhysicalHostConfig, UiPhysicalLayout, UiPhysicalRuntimeUpdate, UiPhysicalSceneBootstrap,
        UiPhysicalSceneState,
    },
    DemoContext, NamedScrollTarget,
};
use crate::demo_core::{ListCommandTarget, ListFilter};
use crate::retained::text::{
    build_fixed_text_run_slot_buffer, build_fixed_text_scene_data,
    build_text_instances_for_node as build_retained_text_instances_for_node,
    rebuild_fixed_text_scene_state_for_scene, update_fixed_text_scene_slots_for_scene,
    FixedTextRunLayout, FixedTextSceneData, FixedTextScenePatch, FixedTextSceneState,
    TextRenderSpace,
};
use crate::retained::ui::{
    build_gpu_ui_patch_for_slot as build_retained_gpu_ui_patch_for_slot,
    build_gpu_ui_patches_for_slots as build_retained_gpu_ui_patches_for_slots,
    build_gpu_ui_scene as build_retained_gpu_ui_scene,
    build_ui_primitives_for_node as build_retained_ui_primitives_for_node, GpuUiSceneData,
    UiRenderSpace,
};
use crate::retained::{set_named_scroll_offset, NodeId, RenderNode, RetainedScene};
use crate::text::{FixedCharGridSpec, VectorFontAtlas};
use crate::todomvc_retained::{TodoMvcResourceDirty, TodoMvcRetainedScene};
use anyhow::Result;
use std::collections::BTreeSet;

pub use crate::retained::text::{GpuCharInstanceEx, TextColors};
pub use crate::retained::ui::GpuUiPrimitive;

pub const VIRTUAL_WIDTH: f32 = 700.0;
pub const VIRTUAL_HEIGHT: f32 = 700.0;
pub const X_OFFSET: f32 = 0.0;
pub const SCREEN_H: f32 = VIRTUAL_HEIGHT;

pub const PIXEL_TO_WORLD: f32 = 1.0 / 100.0;
pub const PIXEL_CENTER_X: f32 = 350.0;
pub const PIXEL_CENTER_Z: f32 = 302.0;

pub const CLASSIC_DECAL_PRIM_START: u32 = 7;

pub struct TodoMvcRetainedState {
    atlas: VectorFontAtlas,
    retained_scene: TodoMvcRetainedScene,
    text_state: FixedTextSceneState,
}

pub type TodoMvcUi2dHost = StateBackedUi2dHost<TodoMvcRetainedState>;
pub type TodoMvcUi2dDeck = StateBackedUi2dSceneDeck<TodoMvcRetainedState>;
pub type TodoMvcUiPhysicalHost = StateBackedUiPhysicalHost<TodoMvcRetainedState>;
pub type TodoMvcUiPhysicalDeck = StateBackedUiPhysicalSceneDeck<TodoMvcRetainedState>;

pub const TODO_UI_STATIC_PRIM_COUNT: usize = 15;
pub const TODO_UI_PRIMS_PER_ITEM: usize = 3;
pub const TODO_UI_SLOT_ITEM_READ: u16 = 0;
pub const TODO_UI_SLOT_ITEM_FINISH: u16 = 1;
pub const TODO_UI_SLOT_ITEM_WALK: u16 = 2;
pub const TODO_UI_SLOT_ITEM_BUY: u16 = 3;
pub const TODO_UI_SLOT_FILTER_ALL: u16 = 100;
pub const TODO_UI_SLOT_FILTER_ACTIVE: u16 = 101;
pub const TODO_UI_SLOT_FILTER_COMPLETED: u16 = 102;
pub const TODO_TEXT_RUN_COUNT: usize = 15;
pub const TODO_TEXT_RUN_HEADING: u16 = 0;
pub const TODO_TEXT_RUN_PLACEHOLDER: u16 = 1;
pub const TODO_TEXT_RUN_ITEM_READ_LABEL: u16 = 2;
pub const TODO_TEXT_RUN_ITEM_FINISH_LABEL: u16 = 3;
pub const TODO_TEXT_RUN_ITEM_WALK_LABEL: u16 = 4;
pub const TODO_TEXT_RUN_ITEM_BUY_LABEL: u16 = 5;
pub const TODO_TEXT_RUN_ITEMS_LEFT_COUNT: u16 = 6;
pub const TODO_TEXT_RUN_ITEMS_LEFT_SUFFIX: u16 = 7;
pub const TODO_TEXT_RUN_FILTER_ALL: u16 = 8;
pub const TODO_TEXT_RUN_FILTER_ACTIVE: u16 = 9;
pub const TODO_TEXT_RUN_FILTER_COMPLETED: u16 = 10;
pub const TODO_TEXT_RUN_CLEAR_COMPLETED: u16 = 11;
pub const TODO_TEXT_RUN_INFO_EDIT: u16 = 12;
pub const TODO_TEXT_RUN_INFO_AUTHOR: u16 = 13;
pub const TODO_TEXT_RUN_INFO_BRAND: u16 = 14;
pub const TODO_TEXT_GRID_DIMS: [u32; 2] = [64, 48];
pub const TODO_TEXT_GRID_BOUNDS: [f32; 4] = [0.0, 0.0, VIRTUAL_WIDTH, VIRTUAL_HEIGHT];
pub const TODO_TEXT_GRID_CELL_CAPACITY: usize = 16;
pub const TODO_TEXT_GRID_INDEX_CAPACITY: usize = (TODO_TEXT_GRID_DIMS[0] as usize)
    * (TODO_TEXT_GRID_DIMS[1] as usize)
    * TODO_TEXT_GRID_CELL_CAPACITY;

const TODO_TEXT_RUN_CAPACITIES: [usize; TODO_TEXT_RUN_COUNT] =
    [8, 32, 48, 48, 48, 48, 4, 16, 8, 8, 12, 20, 32, 32, 20];

pub fn todo_text_grid_spec() -> FixedCharGridSpec {
    FixedCharGridSpec {
        dims: TODO_TEXT_GRID_DIMS,
        bounds: TODO_TEXT_GRID_BOUNDS,
        cell_capacity: TODO_TEXT_GRID_CELL_CAPACITY,
    }
}

pub fn todo_text_run_layout() -> FixedTextRunLayout<'static> {
    FixedTextRunLayout {
        run_capacities: &TODO_TEXT_RUN_CAPACITIES,
        grid_spec: todo_text_grid_spec(),
    }
}

pub fn rgb(r: u8, g: u8, b: u8) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a]
}

pub fn rgb3(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

pub fn todo_text_run_capacity(run_index: usize) -> usize {
    TODO_TEXT_RUN_CAPACITIES[run_index]
}

pub const fn todo_item_ui_slot_for_index(index: usize) -> u16 {
    match index {
        0 => TODO_UI_SLOT_ITEM_READ,
        1 => TODO_UI_SLOT_ITEM_FINISH,
        2 => TODO_UI_SLOT_ITEM_WALK,
        3 => TODO_UI_SLOT_ITEM_BUY,
        _ => panic!("unexpected TodoMVC item ui slot index"),
    }
}

pub fn todo_text_run_offset(run_index: usize) -> usize {
    TODO_TEXT_RUN_CAPACITIES[..run_index].iter().copied().sum()
}

pub fn px_to_world_x(px: f32) -> f32 {
    (px - PIXEL_CENTER_X) * PIXEL_TO_WORLD
}

pub fn px_to_world_z(py: f32) -> f32 {
    -(py - PIXEL_CENTER_Z) * PIXEL_TO_WORLD
}

pub fn create_todomvc_ui2d_host(ctx: &DemoContext) -> Result<TodoMvcUi2dHost> {
    let text_colors = classic_text_colors();
    let state = TodoMvcRetainedState::new(crate::retained::SceneMode::Ui2D, ctx.width, ctx.height)?;
    Ok(StateBackedUi2dHost::new(ctx, "TodoMVC", state, text_colors))
}

pub fn create_todomvc_ui2d_deck(ctx: &DemoContext) -> Result<TodoMvcUi2dDeck> {
    Ok(StateBackedUi2dSceneDeck::new(vec![
        create_todomvc_ui2d_host(ctx)?,
    ]))
}

pub fn create_todomvc_ui_physical_host(
    ctx: &DemoContext,
    colors: &TextColors,
    ui_primitives_label: &str,
) -> Result<TodoMvcUiPhysicalHost> {
    let state = TodoMvcRetainedState::new(
        crate::retained::SceneMode::UiPhysical,
        ctx.width,
        ctx.height,
    )?;
    Ok(StateBackedUiPhysicalHost::new(
        ctx,
        state,
        colors,
        UiPhysicalHostConfig {
            label: ui_primitives_label.to_string(),
            max_char_instances: 512,
            max_ui_primitives: 256,
            max_grid_indices: TODO_TEXT_GRID_INDEX_CAPACITY,
            grid_cell_capacity: TODO_TEXT_GRID_CELL_CAPACITY,
        },
    ))
}

pub fn create_todomvc_ui_physical_deck(
    ctx: &DemoContext,
    colors: &TextColors,
    ui_primitives_label: &str,
) -> Result<TodoMvcUiPhysicalDeck> {
    Ok(StateBackedUiPhysicalSceneDeck::new(vec![
        create_todomvc_ui_physical_host(ctx, colors, ui_primitives_label)?,
    ]))
}

fn todomvc_physical_layout(scene: &RetainedScene) -> UiPhysicalLayout {
    let fallback = UiPhysicalLayout {
        center_px: [350.0, 398.0],
        bounds_px: [75.0, 225.8, 625.0, 570.0],
        corner_radius_px: 12.0,
        content_inset_px: 0.0,
        elevation_px: 0.0,
        depth_px: 8.0,
        fill_color: [248.0 / 255.0, 250.0 / 255.0, 252.0 / 255.0, 1.0],
        accent_color: [0.0; 4],
        detail_color: [0.0; 4],
        outline_color: [203.0 / 255.0, 213.0 / 255.0, 225.0 / 255.0, 1.0],
        outline_width_px: 1.0,
        shadow_color: [15.0 / 255.0, 23.0 / 255.0, 42.0 / 255.0, 0.16],
        shadow_offset_px: [0.0, 14.0],
        shadow_extra_size_px: [12.0, 12.0],
        pixel_to_world: UiPhysicalLayout::DEFAULT_PIXEL_TO_WORLD,
        geometry_mode: UiPhysicalGeometryMode::StackedCard,
    };

    let Some(card) = scene.node_named("card") else {
        return fallback;
    };
    let Some(bounds) = scene.resolved_bounds(card.id) else {
        return fallback;
    };

    let min_y_up = SCREEN_H - (bounds.y + bounds.height);
    let max_y_up = SCREEN_H - bounds.y;

    UiPhysicalLayout {
        center_px: [bounds.x + bounds.width * 0.5, (min_y_up + max_y_up) * 0.5],
        bounds_px: [bounds.x, min_y_up, bounds.x + bounds.width, max_y_up],
        corner_radius_px: 12.0,
        content_inset_px: 0.0,
        elevation_px: 0.0,
        depth_px: 8.0,
        fill_color: [248.0 / 255.0, 250.0 / 255.0, 252.0 / 255.0, 1.0],
        accent_color: [0.0; 4],
        detail_color: [0.0; 4],
        outline_color: [203.0 / 255.0, 213.0 / 255.0, 225.0 / 255.0, 1.0],
        outline_width_px: 1.0,
        shadow_color: [15.0 / 255.0, 23.0 / 255.0, 42.0 / 255.0, 0.16],
        shadow_offset_px: [0.0, 14.0],
        shadow_extra_size_px: [12.0, 12.0],
        pixel_to_world: UiPhysicalLayout::DEFAULT_PIXEL_TO_WORLD,
        geometry_mode: UiPhysicalGeometryMode::StackedCard,
    }
}

impl TodoMvcRetainedState {
    pub fn new(mode: crate::retained::SceneMode, width: u32, height: u32) -> Result<Self> {
        let atlas = load_shared_vector_font_atlas()?;
        let mut retained_scene =
            crate::todomvc_retained::TodoMvcRetainedScene::new(mode, width, height);
        let (text_state, _) =
            build_text_state_from_scene(retained_scene.scene(), &atlas, &classic_text_colors());
        retained_scene.scene_mut().clear_dirty();
        Ok(Self {
            atlas,
            retained_scene,
            text_state,
        })
    }

    pub fn atlas(&self) -> &VectorFontAtlas {
        &self.atlas
    }

    pub fn text_state(&self) -> &FixedTextSceneState {
        &self.text_state
    }

    pub fn scene(&self) -> &RetainedScene {
        self.retained_scene.scene()
    }

    pub fn scene_mut(&mut self) -> &mut RetainedScene {
        self.retained_scene.scene_mut()
    }

    pub fn retained_scene(&self) -> &TodoMvcRetainedScene {
        &self.retained_scene
    }

    pub fn retained_scene_mut(&mut self) -> &mut TodoMvcRetainedScene {
        &mut self.retained_scene
    }

    pub fn mark_view_transform_dirty(&mut self) {
        self.retained_scene.mark_view_transform_dirty();
    }

    pub fn set_viewport_size(&mut self, width: u32, height: u32) {
        self.retained_scene.set_viewport_size(width, height);
    }

    pub fn take_scene_resource_update(&mut self, colors: &TextColors) -> Option<Ui2dRuntimeUpdate> {
        let dirty = self.retained_scene.take_resource_dirty();
        if !dirty.any() {
            return None;
        }
        Some(build_scene_resource_update(
            &mut self.text_state,
            self.retained_scene.scene(),
            &self.atlas,
            colors,
            &dirty,
        ))
    }

    pub fn toggle_item(&mut self, index: usize) -> bool {
        self.retained_scene.toggle_item(index)
    }

    pub fn set_item_completed(&mut self, index: usize, completed: bool) -> bool {
        self.retained_scene.set_item_completed(index, completed)
    }

    pub fn set_item_label(&mut self, index: usize, label: impl Into<String>) -> bool {
        self.retained_scene.set_item_label(index, label)
    }

    pub fn set_filter(&mut self, filter: ListFilter) -> bool {
        self.retained_scene.set_filter(filter)
    }

    pub fn set_scroll_offset(&mut self, y: f32) {
        self.retained_scene.set_scroll_offset(y);
    }
}

fn build_fixed_text_scene_data_from_scene(
    scene: &RetainedScene,
    atlas: &VectorFontAtlas,
    colors: &TextColors,
) -> FixedTextSceneData {
    build_fixed_text_scene_data(todo_text_run_layout(), atlas, |run_index| {
        build_text_run_instances(scene, atlas, colors, run_index)
    })
}

fn build_fixed_text_state_from_scene(
    scene: &RetainedScene,
    atlas: &VectorFontAtlas,
    colors: &TextColors,
) -> (FixedTextSceneState, FixedTextSceneData) {
    crate::retained::text::build_fixed_text_scene_state_for_scene(
        scene,
        todo_text_run_layout(),
        atlas,
        colors,
        text_render_space(),
    )
}

fn build_gpu_ui_scene_data_from_scene(scene: &RetainedScene) -> GpuUiSceneData {
    build_retained_gpu_ui_scene(scene, render_space())
}

impl Ui2dSceneState for TodoMvcRetainedState {
    fn atlas(&self) -> &VectorFontAtlas {
        &self.atlas
    }

    fn text_state(&self) -> &FixedTextSceneState {
        &self.text_state
    }

    fn build_ui2d_init_data(&self) -> Ui2dSceneInitData {
        let (_, text_data) =
            build_fixed_text_state_from_scene(self.scene(), self.atlas(), &classic_text_colors());
        let ui_data = build_gpu_ui_scene_data_from_scene(self.scene());
        Ui2dSceneInitData {
            text_data,
            ui_primitives: ui_data.primitives,
            text_capacity: 512,
            grid_index_capacity: self.text_state.layout().grid_index_capacity().max(1),
            primitive_capacity: 256,
        }
    }

    fn take_ui2d_runtime_update(&mut self, colors: &TextColors) -> Option<Ui2dRuntimeUpdate> {
        self.take_scene_resource_update(colors)
    }

    fn mark_view_transform_dirty(&mut self) {
        self.retained_scene.mark_view_transform_dirty();
    }

    fn set_viewport_size(&mut self, width: u32, height: u32) {
        self.retained_scene.set_viewport_size(width, height);
    }

    fn scene(&self) -> &RetainedScene {
        self.retained_scene.scene()
    }

    fn scene_mut(&mut self) -> &mut RetainedScene {
        self.retained_scene.scene_mut()
    }
}

impl UiPhysicalSceneState for TodoMvcRetainedState {
    fn atlas(&self) -> &VectorFontAtlas {
        &self.atlas
    }

    fn text_state(&self) -> &FixedTextSceneState {
        &self.text_state
    }

    fn build_ui_physical_bootstrap(&self, colors: &TextColors) -> UiPhysicalSceneBootstrap {
        UiPhysicalSceneBootstrap {
            text_data: build_fixed_text_scene_data_from_scene(self.scene(), self.atlas(), colors),
            ui_data: build_gpu_ui_scene_data_from_scene(self.scene()),
            layout: todomvc_physical_layout(self.scene()),
        }
    }

    fn take_ui_physical_resource_update(
        &mut self,
        colors: &TextColors,
    ) -> Option<UiPhysicalRuntimeUpdate> {
        self.take_scene_resource_update(colors).map(Into::into)
    }

    fn mark_view_transform_dirty(&mut self) {
        self.retained_scene.mark_view_transform_dirty();
    }

    fn set_viewport_size(&mut self, width: u32, height: u32) {
        self.retained_scene.set_viewport_size(width, height);
    }

    fn scene(&self) -> &RetainedScene {
        self.retained_scene.scene()
    }

    fn scene_mut(&mut self) -> &mut RetainedScene {
        self.retained_scene.scene_mut()
    }

    fn physical_layout(&self) -> UiPhysicalLayout {
        todomvc_physical_layout(self.scene())
    }
}

impl ListCommandTarget for TodoMvcRetainedState {
    fn toggle_item(&mut self, index: usize) -> bool {
        self.toggle_item(index)
    }

    fn set_item_completed(&mut self, index: usize, completed: bool) -> bool {
        self.set_item_completed(index, completed)
    }

    fn set_item_label(&mut self, index: usize, label: &str) -> bool {
        self.set_item_label(index, label)
    }

    fn set_filter(&mut self, filter: ListFilter) -> bool {
        self.set_filter(filter)
    }

    fn set_scroll_offset(&mut self, offset_y: f32) {
        self.set_scroll_offset(offset_y);
    }
}

impl NamedScrollTarget for TodoMvcRetainedState {
    fn set_named_scroll_offset(&mut self, name: &str, offset_y: f32) -> bool {
        set_named_scroll_offset(self.scene_mut(), name, offset_y)
    }
}

fn todo_text_run_node(scene: &RetainedScene, run_index: usize) -> Option<&RenderNode> {
    scene.node_with_text_slot(run_index as u16)
}

pub fn todo_text_run_index_for_node(
    scene: &RetainedScene,
    node_id: crate::retained::NodeId,
) -> Option<usize> {
    let node = scene.node(node_id)?;
    let text_slot = node.text_slot? as usize;
    (text_slot < TODO_TEXT_RUN_COUNT).then_some(text_slot)
}

fn render_space() -> UiRenderSpace {
    UiRenderSpace {
        x_offset: X_OFFSET,
        screen_height: SCREEN_H,
    }
}

fn text_render_space() -> TextRenderSpace {
    TextRenderSpace {
        x_offset: X_OFFSET,
        screen_height: SCREEN_H,
        italic_codepoint_offset: ITALIC_CODEPOINT_OFFSET,
    }
}

pub fn build_ui_primitives_for_node(
    scene: &RetainedScene,
    node: &RenderNode,
) -> Option<Vec<GpuUiPrimitive>> {
    build_retained_ui_primitives_for_node(scene, node, render_space()).map(|primitives| {
        primitives
            .into_iter()
            .map(crate::retained::ui::pack_gpu_ui_primitive)
            .collect()
    })
}

pub fn build_ui_patch_for_slot(
    scene: &RetainedScene,
    ui_slot: u16,
) -> Option<crate::retained::ui::GpuUiPatch> {
    build_retained_gpu_ui_patch_for_slot(scene, ui_slot, render_space())
}

pub fn build_ui_patches_for_slots(
    scene: &RetainedScene,
    ui_slots: &BTreeSet<u16>,
) -> Vec<crate::retained::ui::GpuUiPatch> {
    build_retained_gpu_ui_patches_for_slots(scene, ui_slots, render_space())
}

pub fn build_ui_primitives_from_scene(scene: &RetainedScene) -> Vec<GpuUiPrimitive> {
    build_retained_gpu_ui_scene(scene, render_space()).primitives
}

pub fn build_ui_scene_data_from_scene(scene: &RetainedScene) -> GpuUiSceneData {
    build_retained_gpu_ui_scene(scene, render_space())
}

pub fn classic_text_colors() -> TextColors {
    TextColors {
        heading: rgb3(184, 63, 69),
        active: rgb3(72, 72, 72),
        completed: rgb3(148, 148, 148),
        placeholder: rgb3(153, 153, 153),
        body: rgb3(17, 17, 17),
        info: rgb3(77, 77, 77),
    }
}

pub fn build_text_run_instances(
    scene: &RetainedScene,
    atlas: &VectorFontAtlas,
    colors: &TextColors,
    run_index: usize,
) -> Vec<GpuCharInstanceEx> {
    let Some(node) = todo_text_run_node(scene, run_index) else {
        return Vec::new();
    };
    build_retained_text_instances_for_node(scene, node.id, atlas, colors, text_render_space())
}

pub fn build_text_run_slot_buffer(
    scene: &RetainedScene,
    atlas: &VectorFontAtlas,
    colors: &TextColors,
    run_index: usize,
) -> (Vec<GpuCharInstanceEx>, u32) {
    let instances = build_text_run_instances(scene, atlas, colors, run_index);
    build_fixed_text_run_slot_buffer(instances, todo_text_run_capacity(run_index), atlas)
}

pub fn build_text_scene_data_from_scene(
    scene: &RetainedScene,
    atlas: &VectorFontAtlas,
    colors: &TextColors,
) -> FixedTextSceneData {
    build_fixed_text_scene_data(todo_text_run_layout(), atlas, |run_index| {
        build_text_run_instances(scene, atlas, colors, run_index)
    })
}

pub fn build_text_state_from_scene(
    scene: &RetainedScene,
    atlas: &VectorFontAtlas,
    colors: &TextColors,
) -> (FixedTextSceneState, FixedTextSceneData) {
    crate::retained::text::build_fixed_text_scene_state_for_scene(
        scene,
        todo_text_run_layout(),
        atlas,
        colors,
        text_render_space(),
    )
}

pub fn rebuild_text_state_from_scene(
    state: &mut FixedTextSceneState,
    scene: &RetainedScene,
    atlas: &VectorFontAtlas,
    colors: &TextColors,
) -> FixedTextSceneData {
    rebuild_fixed_text_scene_state_for_scene(state, scene, atlas, colors, text_render_space())
}

pub fn update_text_state_for_nodes(
    state: &mut FixedTextSceneState,
    scene: &RetainedScene,
    atlas: &VectorFontAtlas,
    colors: &TextColors,
    nodes: impl IntoIterator<Item = NodeId>,
) -> FixedTextScenePatch {
    let text_slots = nodes
        .into_iter()
        .filter_map(|id| todo_text_run_index_for_node(scene, id).map(|idx| idx as u16))
        .collect::<BTreeSet<_>>();

    update_fixed_text_scene_slots_for_scene(
        state,
        scene,
        atlas,
        text_slots,
        colors,
        text_render_space(),
    )
}

pub fn build_text_resource_update(
    state: &mut FixedTextSceneState,
    scene: &RetainedScene,
    atlas: &VectorFontAtlas,
    colors: &TextColors,
    full_rebuild: bool,
    dirty_nodes: impl IntoIterator<Item = NodeId>,
) -> Option<Ui2dRuntimeTextUpdate> {
    let dirty_nodes = dirty_nodes.into_iter().collect::<Vec<_>>();
    if !full_rebuild && dirty_nodes.is_empty() {
        return None;
    }

    Some(if full_rebuild {
        Ui2dRuntimeTextUpdate::Full(build_fixed_text_scene_data_from_scene(scene, atlas, colors))
    } else {
        Ui2dRuntimeTextUpdate::Partial(update_text_state_for_nodes(
            state,
            scene,
            atlas,
            colors,
            dirty_nodes,
        ))
    })
}

pub fn build_ui_resource_update(
    scene: &RetainedScene,
    full_rebuild: bool,
    dirty_slots: &BTreeSet<u16>,
) -> Option<Ui2dRuntimeUiUpdate> {
    if !full_rebuild && dirty_slots.is_empty() {
        return None;
    }

    Some(if full_rebuild {
        Ui2dRuntimeUiUpdate::Full(build_gpu_ui_scene_data_from_scene(scene))
    } else {
        Ui2dRuntimeUiUpdate::Partial(
            build_ui_patches_for_slots(scene, dirty_slots)
                .into_iter()
                .map(|patch| crate::retained::ui::GpuUiPatch {
                    offset: patch.offset,
                    primitives: patch.primitives,
                })
                .collect(),
        )
    })
}

pub fn build_scene_resource_update(
    state: &mut FixedTextSceneState,
    scene: &RetainedScene,
    atlas: &VectorFontAtlas,
    colors: &TextColors,
    dirty: &TodoMvcResourceDirty,
) -> Ui2dRuntimeUpdate {
    Ui2dRuntimeUpdate {
        text: if dirty.text {
            build_text_resource_update(
                state,
                scene,
                atlas,
                colors,
                dirty.full_text
                    || (dirty.text_style_nodes.is_empty() && dirty.text_layout_nodes.is_empty()),
                dirty
                    .text_style_nodes
                    .iter()
                    .chain(dirty.text_layout_nodes.iter())
                    .copied(),
            )
        } else {
            None
        },
        ui: if dirty.ui {
            build_ui_resource_update(
                scene,
                dirty.full_ui || dirty.ui_slots.is_empty(),
                &dirty.ui_slots,
            )
        } else {
            None
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retained::text::FixedTextGridCache;
    use crate::retained::SceneMode;
    use crate::todomvc_retained::TodoMvcRetainedScene;

    #[test]
    fn text_grid_cache_matches_full_rebuild_after_label_change() {
        let atlas = load_shared_vector_font_atlas().expect("load shared vector font atlas");
        let colors = classic_text_colors();
        let mut retained = TodoMvcRetainedScene::new(SceneMode::Ui2D, 800, 600);

        let initial = build_text_scene_data_from_scene(retained.scene(), &atlas, &colors);
        let mut cache =
            FixedTextGridCache::new(&initial.char_instances, &atlas, todo_text_run_layout());

        retained.set_item_label(1, "Finish the retained renderer migration");
        let (run_slots, _) = build_text_run_slot_buffer(retained.scene(), &atlas, &colors, 3);
        let changed_cells = cache.update_run_slots(&atlas, 3, &run_slots);
        let rebuilt = build_text_scene_data_from_scene(retained.scene(), &atlas, &colors);

        assert!(!changed_cells.is_empty());
        assert_eq!(cache.cells, rebuilt.char_grid_cells);
        assert_eq!(cache.indices, rebuilt.char_grid_indices);
    }

    #[test]
    fn ui_patch_matches_full_ui_build_for_selected_slots() {
        let retained = TodoMvcRetainedScene::new(SceneMode::Ui2D, 800, 600);
        let ui_data = build_ui_scene_data_from_scene(retained.scene());
        let patches = build_ui_patches_for_slots(
            retained.scene(),
            &[TODO_UI_SLOT_ITEM_FINISH, TODO_UI_SLOT_FILTER_ALL]
                .into_iter()
                .collect(),
        );

        assert_eq!(patches.len(), 2);
        for patch in patches {
            assert_eq!(
                patch.primitives,
                ui_data.primitives[patch.offset..patch.offset + patch.primitives.len()]
            );
        }
    }
}
