use crate::retained::text::{
    build_fixed_text_scene_data,
    build_text_instances_for_node as build_retained_text_instances_for_node, FixedTextRunLayout,
    FixedTextSceneData, GpuCharInstanceEx, TextColors, TextRenderSpace,
};
use crate::retained::ui::{
    build_gpu_ui_scene as build_retained_gpu_ui_scene, GpuUiPrimitive, GpuUiSceneData,
    UiRenderSpace,
};
use crate::retained::RetainedScene;
use crate::text::{FixedCharGridSpec, VectorFontAtlas};

pub const VIRTUAL_WIDTH: f32 = 700.0;
pub const VIRTUAL_HEIGHT: f32 = 700.0;
pub const X_OFFSET: f32 = 0.0;
pub const SCREEN_H: f32 = VIRTUAL_HEIGHT;

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

pub fn rgb3(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
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

pub fn todo_text_render_space(italic_codepoint_offset: u32) -> TextRenderSpace {
    TextRenderSpace {
        x_offset: X_OFFSET,
        screen_height: SCREEN_H,
        italic_codepoint_offset,
    }
}

pub fn todo_ui_render_space() -> UiRenderSpace {
    UiRenderSpace {
        x_offset: X_OFFSET,
        screen_height: SCREEN_H,
    }
}

pub fn build_text_scene_data_from_scene(
    scene: &RetainedScene,
    atlas: &VectorFontAtlas,
    colors: &TextColors,
    italic_codepoint_offset: u32,
) -> FixedTextSceneData {
    let render_space = todo_text_render_space(italic_codepoint_offset);
    build_fixed_text_scene_data(todo_text_run_layout(), atlas, |run_index| {
        let Some(node) = scene.node_with_text_slot(run_index as u16) else {
            return Vec::new();
        };
        build_retained_text_instances_for_node(scene, node.id, atlas, colors, render_space)
    })
}

pub fn build_ui_scene_data_from_scene(scene: &RetainedScene) -> GpuUiSceneData {
    build_retained_gpu_ui_scene(scene, todo_ui_render_space())
}

pub fn build_ui_primitives_from_scene(scene: &RetainedScene) -> Vec<GpuUiPrimitive> {
    build_ui_scene_data_from_scene(scene).primitives
}

pub fn build_text_run_instances(
    scene: &RetainedScene,
    atlas: &VectorFontAtlas,
    colors: &TextColors,
    run_index: usize,
    italic_codepoint_offset: u32,
) -> Vec<GpuCharInstanceEx> {
    let Some(node) = scene.node_with_text_slot(run_index as u16) else {
        return Vec::new();
    };
    build_retained_text_instances_for_node(
        scene,
        node.id,
        atlas,
        colors,
        todo_text_render_space(italic_codepoint_offset),
    )
}
