use super::{
    gpu_runtime_common::{
        build_font_gpu_data, create_bind_group_layout_with_storage, create_bind_group_with_storage,
        create_fullscreen_pipeline, create_storage_buffers, UiStorageBuffers,
    },
    ui_physical_theme::{
        tune_generic_ui_physical_text_colors, ThemeUniforms, UiPhysicalThemeState,
    },
    DemoContext,
};
use crate::camera::FlyCamera;
use crate::demo_core::{ListCommandTarget, ListFilter, NamedScrollTarget};
use crate::retained::fixed_scene::{
    BuiltFixedUi2dScene, FixedUi2dSceneModelBuilder, FixedUi2dSceneModelCapture,
    FixedUi2dSceneState, FixedUi2dSceneUpdate,
};
use crate::retained::samples::{SampleSceneAction, SampleSceneDeckTarget, SampleSceneModel};
use crate::retained::showcase::{ShowcaseSceneAction, ShowcaseSceneDeckTarget, ShowcaseSceneModel};
use crate::retained::text::{
    apply_fixed_text_runtime_update, build_fixed_text_scene_state_for_scene,
    FixedTextRuntimeUpdate, FixedTextSceneData, FixedTextSceneState, GpuCharInstanceEx, TextColors,
    TextRenderSpace,
};
use crate::retained::text_scene::WrappedTextSceneModel;
use crate::retained::ui::{
    apply_gpu_ui_runtime_update, build_gpu_ui_scene, GpuUiPrimitive, GpuUiRuntimeUpdate,
    GpuUiSceneData, UiRenderSpace,
};
use crate::retained::{NamedScrollSceneModel, Rect, RetainedScene, UiVisualRole};
use crate::text::{CharGridCell, VectorFontAtlas};
use crate::ui_physical_shader_bindings as retained_ui_physical_shader;
use bytemuck::{Pod, Zeroable};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use wgpu::util::DeviceExt;

static STORAGE_REVISION_SEED: AtomicU64 = AtomicU64::new(1u64 << 32);

fn next_storage_revision_seed() -> u64 {
    STORAGE_REVISION_SEED.fetch_add(1u64 << 32, Ordering::Relaxed)
}

pub struct UiPhysicalSceneBootstrap {
    pub text_data: FixedTextSceneData,
    pub ui_data: GpuUiSceneData,
    pub layout: UiPhysicalLayout,
}

pub type UiPhysicalRuntimeTextUpdate = FixedTextRuntimeUpdate;
pub type UiPhysicalRuntimeUiUpdate = GpuUiRuntimeUpdate;

pub struct UiPhysicalRuntimeUpdate {
    pub text: Option<UiPhysicalRuntimeTextUpdate>,
    pub ui: Option<UiPhysicalRuntimeUiUpdate>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiPhysicalGeometryMode {
    GenericCard,
    StackedCard,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiPhysicalLayout {
    pub center_px: [f32; 2],
    pub bounds_px: [f32; 4],
    pub corner_radius_px: f32,
    pub content_inset_px: f32,
    pub elevation_px: f32,
    pub depth_px: f32,
    pub fill_color: [f32; 4],
    pub accent_color: [f32; 4],
    pub detail_color: [f32; 4],
    pub outline_color: [f32; 4],
    pub outline_width_px: f32,
    pub shadow_color: [f32; 4],
    pub shadow_offset_px: [f32; 2],
    pub shadow_extra_size_px: [f32; 2],
    pub pixel_to_world: f32,
    pub geometry_mode: UiPhysicalGeometryMode,
}

pub struct FixedUiPhysicalSceneState {
    pub scene_state: FixedUi2dSceneState,
    atlas: Arc<VectorFontAtlas>,
    text_render_space: TextRenderSpace,
    ui_render_space: UiRenderSpace,
}

#[derive(Clone, Debug)]
pub struct UiPhysicalHostConfig {
    pub label: String,
    pub max_char_instances: usize,
    pub max_ui_primitives: usize,
    pub max_grid_indices: usize,
    pub grid_cell_capacity: usize,
}

impl UiPhysicalSceneBootstrap {
    fn fits_config(&self, config: &UiPhysicalHostConfig) -> bool {
        self.text_data.char_instances.len() <= config.max_char_instances
            && self.text_data.char_grid_indices.len() <= config.max_grid_indices
            && self.ui_data.primitive_count <= config.max_ui_primitives
    }
}

impl UiPhysicalLayout {
    pub const DEFAULT_PIXEL_TO_WORLD: f32 = 0.01;

    pub fn generic_from_scene(
        scene: &RetainedScene,
        text_data: &FixedTextSceneData,
        ui_data: &GpuUiSceneData,
    ) -> Self {
        let semantic_surface = scene
            .nodes()
            .values()
            .filter(|node| node.ui_visual_role == Some(UiVisualRole::FilledSurface))
            .filter_map(|node| scene.resolved_bounds(node.id).map(|bounds| (node, bounds)))
            .max_by(|a, b| {
                let a_area = a.1.width * a.1.height;
                let b_area = b.1.width * b.1.height;
                a_area.total_cmp(&b_area)
            });
        let semantic_outline = scene
            .nodes()
            .values()
            .filter(|node| node.ui_visual_role == Some(UiVisualRole::OutlineRect))
            .filter_map(|node| scene.resolved_bounds(node.id).map(|bounds| (node, bounds)))
            .max_by(|a, b| {
                let a_area = a.1.width * a.1.height;
                let b_area = b.1.width * b.1.height;
                a_area.total_cmp(&b_area)
            });
        let semantic_shadow = scene
            .nodes()
            .values()
            .filter(|node| node.ui_visual_role == Some(UiVisualRole::BoxShadow))
            .filter_map(|node| scene.resolved_bounds(node.id).map(|bounds| (node, bounds)))
            .max_by(|a, b| {
                let a_area = a.1.width * a.1.height;
                let b_area = b.1.width * b.1.height;
                a_area.total_cmp(&b_area)
            });

        let clip_bounds = scene
            .nodes()
            .values()
            .filter(|node| node.clip || node.element == crate::retained::ElementKind::Clip)
            .filter_map(|clip| scene.resolved_bounds(clip.id))
            .reduce(|acc, bounds| {
                Rect::new(
                    acc.x.min(bounds.x),
                    acc.y.min(bounds.y),
                    (acc.x + acc.width).max(bounds.x + bounds.width) - acc.x.min(bounds.x),
                    (acc.y + acc.height).max(bounds.y + bounds.height) - acc.y.min(bounds.y),
                )
            });
        let text_bounds = text_scene_bounds(text_data)
            .and_then(|bounds| {
                let rect = Rect::new(
                    bounds[0],
                    bounds[1],
                    bounds[2] - bounds[0],
                    bounds[3] - bounds[1],
                );
                match clip_bounds {
                    Some(clip) => rect.intersect(clip).or(Some(clip)),
                    None => Some(rect),
                }
            })
            .map(|bounds| {
                [
                    bounds.x,
                    bounds.y,
                    bounds.x + bounds.width,
                    bounds.y + bounds.height,
                ]
            });
        let base_bounds = semantic_surface
            .map(|(_, bounds)| bounds)
            .or_else(|| semantic_outline.map(|(_, bounds)| bounds))
            .or(clip_bounds)
            .or_else(|| {
                text_bounds.map(|bounds| {
                    Rect::new(
                        bounds[0],
                        bounds[1],
                        bounds[2] - bounds[0],
                        bounds[3] - bounds[1],
                    )
                })
            });
        let mut min_x = base_bounds.map(|b| b.x).unwrap_or(f32::INFINITY);
        let mut min_y = base_bounds.map(|b| b.y).unwrap_or(f32::INFINITY);
        let mut max_x = base_bounds
            .map(|b| b.x + b.width)
            .unwrap_or(f32::NEG_INFINITY);
        let mut max_y = base_bounds
            .map(|b| b.y + b.height)
            .unwrap_or(f32::NEG_INFINITY);
        let has_semantic_frame = semantic_surface.is_some() || semantic_outline.is_some();

        let mut corner_radius_px = 18.0;
        let mut content_inset_px: f32 = 0.0;
        let mut elevation_px = 6.0;
        let mut depth_px = 10.0;
        let mut fill_color = [248.0 / 255.0, 250.0 / 255.0, 252.0 / 255.0, 1.0];
        let mut accent_color = [0.0; 4];
        let mut detail_color = [0.0; 4];
        let mut outline_color = [203.0 / 255.0, 213.0 / 255.0, 225.0 / 255.0, 0.0];
        let mut outline_width_px = 0.0;
        if let Some((node, bounds)) = semantic_surface {
            min_x = min_x.min(bounds.x);
            min_y = min_y.min(bounds.y);
            max_x = max_x.max(bounds.x + bounds.width);
            max_y = max_y.max(bounds.y + bounds.height);
            if let Some(style) = node.ui_visual_style {
                corner_radius_px = style.corner_radius.max(0.0);
                fill_color = style.base_color;
                accent_color = style.accent_color;
                detail_color = style.detail_color;
            }
            elevation_px = node.elevation.max(0.0);
            depth_px = node.depth.max(0.0);
        }
        if let Some((node, bounds)) = semantic_outline {
            min_x = min_x.min(bounds.x);
            min_y = min_y.min(bounds.y);
            max_x = max_x.max(bounds.x + bounds.width);
            max_y = max_y.max(bounds.y + bounds.height);
            if let Some(style) = node.ui_visual_style {
                outline_color = style.base_color;
                outline_width_px = style.stroke_width.max(0.0);
                corner_radius_px = corner_radius_px.max(style.corner_radius.max(0.0));
            }
        }
        let semantic_alpha = fill_color[3].clamp(0.0, 1.0);
        let detail_mix = detail_color[3].clamp(0.0, 1.0) * 0.25;
        let accent_mix = accent_color[3].clamp(0.0, 1.0) * 0.12;
        let shadow_rgb = [
            fill_color[0] * 0.20 * (1.0 - detail_mix)
                + detail_color[0] * detail_mix
                + accent_color[0] * accent_mix,
            fill_color[1] * 0.22 * (1.0 - detail_mix)
                + detail_color[1] * detail_mix
                + accent_color[1] * accent_mix,
            fill_color[2] * 0.28 * (1.0 - detail_mix)
                + detail_color[2] * detail_mix
                + accent_color[2] * accent_mix,
        ];
        let mut shadow_color = [
            shadow_rgb[0],
            shadow_rgb[1],
            shadow_rgb[2],
            0.10 + semantic_alpha * 0.06,
        ];
        let mut shadow_offset_px = [0.0, elevation_px * 1.2 + depth_px * 0.35];
        let mut shadow_extra_size_px = [depth_px * 0.45, elevation_px * 0.25 + depth_px * 0.35];

        if let Some((node, _bounds)) = semantic_shadow {
            if let Some(style) = node.ui_visual_style {
                shadow_color = style.base_color;
                shadow_offset_px = style.offset;
                shadow_extra_size_px = [style.extra_size[0].max(0.0), style.extra_size[1].max(0.0)];
                corner_radius_px = corner_radius_px.max(style.corner_radius.max(0.0));
            }
        }

        if !has_semantic_frame {
            if let Some(bounds) = clip_bounds {
                min_x = min_x.min(bounds.x);
                min_y = min_y.min(bounds.y);
                max_x = max_x.max(bounds.x + bounds.width);
                max_y = max_y.max(bounds.y + bounds.height);
            }

            if let Some(bounds) = text_bounds {
                min_x = min_x.min(bounds[0]);
                min_y = min_y.min(bounds[1]);
                max_x = max_x.max(bounds[2]);
                max_y = max_y.max(bounds[3]);
            }

            for primitive in &ui_data.primitives {
                let [x0, y0, x1, y1] = gpu_ui_primitive_bounds(primitive);
                min_x = min_x.min(x0);
                min_y = min_y.min(y0);
                max_x = max_x.max(x1);
                max_y = max_y.max(y1);
            }
        } else {
            let mut update_inset_from_rect = |rect: Rect| {
                let left = (rect.x - min_x).max(0.0);
                let top = (rect.y - min_y).max(0.0);
                let right = (max_x - (rect.x + rect.width)).max(0.0);
                let bottom = (max_y - (rect.y + rect.height)).max(0.0);
                let semantic_inset = left.min(top).min(right).min(bottom);
                if semantic_inset.is_finite() {
                    content_inset_px = content_inset_px.max(semantic_inset.max(0.0));
                }
            };

            if let Some(bounds) = clip_bounds {
                update_inset_from_rect(bounds);
            }

            if let Some(bounds) = text_bounds {
                update_inset_from_rect(Rect::new(
                    bounds[0],
                    bounds[1],
                    bounds[2] - bounds[0],
                    bounds[3] - bounds[1],
                ));
            }

            for primitive in &ui_data.primitives {
                let [x0, y0, x1, y1] = gpu_ui_primitive_bounds(primitive);
                update_inset_from_rect(Rect::new(x0, y0, x1 - x0, y1 - y0));
            }

            if content_inset_px <= 0.0 {
                content_inset_px = (corner_radius_px * 0.16 + outline_width_px * 0.75).max(0.0);
            }
        }

        if !min_x.is_finite()
            || !min_y.is_finite()
            || !max_x.is_finite()
            || !max_y.is_finite()
            || min_x >= max_x
            || min_y >= max_y
        {
            min_x = 100.0;
            min_y = 100.0;
            max_x = 600.0;
            max_y = 600.0;
        }

        Self {
            center_px: [(min_x + max_x) * 0.5, (min_y + max_y) * 0.5],
            bounds_px: [min_x, min_y, max_x, max_y],
            corner_radius_px,
            content_inset_px,
            elevation_px,
            depth_px,
            fill_color,
            accent_color,
            detail_color,
            outline_color,
            outline_width_px,
            shadow_color,
            shadow_offset_px,
            shadow_extra_size_px,
            pixel_to_world: Self::DEFAULT_PIXEL_TO_WORLD,
            geometry_mode: UiPhysicalGeometryMode::GenericCard,
        }
    }
}

fn gpu_ui_primitive_bounds(primitive: &GpuUiPrimitive) -> [f32; 4] {
    let prim_type = primitive.params[3];
    if prim_type < 0.5 || (prim_type >= 4.5 && prim_type < 5.5) {
        let x = primitive.pos_size[0];
        let y = primitive.pos_size[1];
        let w = primitive.pos_size[2];
        let h = primitive.pos_size[3];
        [x, y, x + w, y + h]
    } else if prim_type < 1.5 {
        let x = primitive.pos_size[0];
        let y = primitive.pos_size[1];
        let w = primitive.pos_size[2];
        let h = primitive.pos_size[3];
        [x, y, x + w, y + h]
    } else if prim_type < 3.5 {
        let cx = primitive.pos_size[0];
        let cy = primitive.pos_size[1];
        let r = primitive.pos_size[2].abs();
        [cx - r, cy - r, cx + r, cy + r]
    } else if prim_type < 4.5 {
        let x0 = primitive.pos_size[0].min(primitive.pos_size[2]);
        let y0 = primitive.pos_size[1].min(primitive.pos_size[3]);
        let x1 = primitive.pos_size[0].max(primitive.pos_size[2]);
        let y1 = primitive.pos_size[1].max(primitive.pos_size[3]);
        [x0, y0, x1, y1]
    } else if prim_type < 6.5 {
        let x0 = primitive.pos_size[0]
            .min(primitive.pos_size[2])
            .min(primitive.extra[0]);
        let y0 = primitive.pos_size[1]
            .min(primitive.pos_size[3])
            .min(primitive.extra[1]);
        let x1 = primitive.pos_size[0]
            .max(primitive.pos_size[2])
            .max(primitive.extra[0]);
        let y1 = primitive.pos_size[1]
            .max(primitive.pos_size[3])
            .max(primitive.extra[1]);
        [x0, y0, x1, y1]
    } else {
        [0.0, 0.0, 0.0, 0.0]
    }
}

fn text_scene_bounds(text_data: &FixedTextSceneData) -> Option<[f32; 4]> {
    let count = (text_data.char_count as usize).min(text_data.char_instances.len());
    if count == 0 {
        return None;
    }

    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    let mut found = false;

    for inst in text_data.char_instances.iter().take(count) {
        let x = inst.pos_and_char[0];
        let y = inst.pos_and_char[1];
        let font_size = inst.pos_and_char[2].abs();

        if font_size <= 0.0 {
            continue;
        }

        // Generic physical cards should hug visible text, not the coarse fixed
        // grid bounds that often span the whole clip/viewport.
        let width = font_size * 0.7;
        let height = font_size * 1.15;

        min_x = min_x.min(x);
        min_y = min_y.min(y - height);
        max_x = max_x.max(x + width);
        max_y = max_y.max(y);
        found = true;
    }

    found.then_some([min_x, min_y, max_x, max_y])
}

impl UiPhysicalHostConfig {
    fn grown_to_fit(&self, bootstrap: &UiPhysicalSceneBootstrap) -> Self {
        Self {
            label: self.label.clone(),
            max_char_instances: self
                .max_char_instances
                .max(bootstrap.text_data.char_instances.len()),
            max_ui_primitives: self
                .max_ui_primitives
                .max(bootstrap.ui_data.primitive_count),
            max_grid_indices: self
                .max_grid_indices
                .max(bootstrap.text_data.char_grid_indices.len()),
            grid_cell_capacity: self
                .grid_cell_capacity
                .max(bootstrap.text_data.char_grid_cells.len()),
        }
    }
}

pub fn recolor_physical_char_instances(
    char_instances: &[GpuCharInstanceEx],
    colors: &TextColors,
) -> Vec<GpuCharInstanceEx> {
    char_instances
        .iter()
        .map(|inst| {
            let flags = inst.color_flags[3];
            let color = if flags == 2.0 {
                colors.heading
            } else if flags == 1.0 {
                colors.completed
            } else if flags == 3.0 {
                colors.placeholder
            } else if flags == 4.0 {
                colors.body
            } else if flags == 5.0 {
                colors.info
            } else {
                colors.active
            };
            GpuCharInstanceEx {
                pos_and_char: inst.pos_and_char,
                color_flags: [color[0], color[1], color[2], flags],
            }
        })
        .collect()
}

impl UiPhysicalRuntimeUpdate {
    fn needs_full_rebuild(&self) -> bool {
        matches!(self.text, Some(UiPhysicalRuntimeTextUpdate::Full(_)))
            || matches!(self.ui, Some(UiPhysicalRuntimeUiUpdate::Full(_)))
    }
}

impl From<FixedUi2dSceneUpdate> for UiPhysicalRuntimeUpdate {
    fn from(update: FixedUi2dSceneUpdate) -> Self {
        match update {
            FixedUi2dSceneUpdate::Full { text_data, ui_data } => Self {
                text: Some(UiPhysicalRuntimeTextUpdate::Full(text_data)),
                ui: Some(UiPhysicalRuntimeUiUpdate::Full(ui_data)),
            },
            FixedUi2dSceneUpdate::Partial {
                ui_patches,
                text_patch,
            } => Self {
                text: text_patch.map(UiPhysicalRuntimeTextUpdate::Partial),
                ui: (!ui_patches.is_empty())
                    .then_some(UiPhysicalRuntimeUiUpdate::Partial(ui_patches)),
            },
        }
    }
}

impl From<crate::demos::ui2d_runtime::Ui2dRuntimeUpdate> for UiPhysicalRuntimeUpdate {
    fn from(update: crate::demos::ui2d_runtime::Ui2dRuntimeUpdate) -> Self {
        Self {
            text: update.text,
            ui: update.ui,
        }
    }
}

pub trait UiPhysicalSceneState {
    fn atlas(&self) -> &VectorFontAtlas;
    fn text_state(&self) -> &FixedTextSceneState;
    fn build_ui_physical_bootstrap(&self, colors: &TextColors) -> UiPhysicalSceneBootstrap;
    fn take_ui_physical_resource_update(
        &mut self,
        colors: &TextColors,
    ) -> Option<UiPhysicalRuntimeUpdate>;
    fn mark_view_transform_dirty(&mut self);
    fn set_viewport_size(&mut self, width: u32, height: u32);
    fn scene(&self) -> &RetainedScene;
    fn scene_mut(&mut self) -> &mut RetainedScene;
    fn physical_layout(&self) -> UiPhysicalLayout;
}

pub struct StateBackedUiPhysicalHost<S> {
    state: S,
    text_colors: TextColors,
    storage_buffers: UiStorageBuffers,
    char_instances: Vec<GpuCharInstanceEx>,
    char_count: u32,
    ui_prim_count: u32,
    char_grid_params: [f32; 4],
    char_grid_bounds: [f32; 4],
    layout: UiPhysicalLayout,
    storage_revision: u64,
    config: UiPhysicalHostConfig,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

pub struct FixedUiPhysicalSceneHost {
    state_host: StateBackedUiPhysicalHost<FixedUiPhysicalSceneState>,
}

pub struct ModeledFixedUiPhysicalSceneHost<M> {
    model: M,
    host: FixedUiPhysicalSceneHost,
    viewport_size: [u32; 2],
    text_colors: TextColors,
    needs_rebuild: bool,
}

pub trait UiPhysicalDeckHost {
    fn needs_redraw(&self) -> bool;
    fn prepare_frame(&mut self, queue: &wgpu::Queue);
    fn resize(&mut self, width: u32, height: u32);
    fn resize_lazy(&mut self, width: u32, height: u32);
    fn ensure_ready(&mut self);
    fn set_text_colors(&mut self, text_colors: TextColors);
    fn storage_buffers(&self) -> &UiStorageBuffers;
    fn char_instances(&self) -> &[GpuCharInstanceEx];
}

pub trait UiPhysicalRuntimeScene {
    fn needs_redraw(&self) -> bool;
    fn prepare_frame_with_theme(&mut self, queue: &wgpu::Queue, theme_state: &UiPhysicalThemeState);
    fn sync_theme_state(
        &self,
        renderer: &UiPhysicalFullscreenRenderer,
        queue: &wgpu::Queue,
        theme_state: &mut UiPhysicalThemeState,
    );
    fn storage_buffers(&self) -> &UiStorageBuffers;
    fn storage_revision(&self) -> u64;
    fn char_count(&self) -> u32;
    fn ui_prim_count(&self) -> u32;
    fn char_grid_params(&self) -> [f32; 4];
    fn char_grid_bounds(&self) -> [f32; 4];
    fn physical_layout(&self) -> UiPhysicalLayout;
    fn mark_active_view_transform_dirty(&mut self);
    fn resize(&mut self, width: u32, height: u32);
}

pub struct UiPhysicalSceneDeck<H> {
    scenes: Vec<H>,
    active_scene: usize,
}

pub type ModeledFixedUiPhysicalSceneDeck<M> =
    UiPhysicalSceneDeck<ModeledFixedUiPhysicalSceneHost<M>>;
pub type ShowcaseUiPhysicalDeck = ModeledFixedUiPhysicalSceneDeck<ShowcaseSceneModel>;
pub type WrappedTextUiPhysicalDeck = ModeledFixedUiPhysicalSceneDeck<WrappedTextSceneModel>;
pub type StateBackedUiPhysicalSceneDeck<S> = UiPhysicalSceneDeck<StateBackedUiPhysicalHost<S>>;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct UiPhysicalUniforms {
    pub inv_view_proj: [[f32; 4]; 4],
    pub camera_pos_time: [f32; 4],
    pub light_dir_intensity: [f32; 4],
    pub render_params: [f32; 4],
    pub text_params: [f32; 4],
    pub char_grid_params: [f32; 4],
    pub char_grid_bounds: [f32; 4],
    pub layout_params0: [f32; 4],
    pub layout_params1: [f32; 4],
    pub layout_params2: [f32; 4],
    pub layout_params3: [f32; 4],
    pub layout_params4: [f32; 4],
    pub layout_params5: [f32; 4],
    pub layout_params6: [f32; 4],
    pub layout_params7: [f32; 4],
    pub layout_params8: [f32; 4],
    pub layout_params9: [f32; 4],
    pub layout_bounds: [f32; 4],
}

impl Default for UiPhysicalUniforms {
    fn default() -> Self {
        Self {
            inv_view_proj: [[0.0; 4]; 4],
            camera_pos_time: [0.0, 3.5, 3.5, 0.0],
            light_dir_intensity: [0.5, 0.8, 0.3, 1.5],
            render_params: [800.0, 600.0, 0.08, 1.0],
            text_params: [0.0; 4],
            char_grid_params: [0.0; 4],
            char_grid_bounds: [0.0; 4],
            layout_params0: [350.0, 398.0, UiPhysicalLayout::DEFAULT_PIXEL_TO_WORLD, 1.0],
            layout_params1: [12.0, 0.0, 8.0, 0.0],
            layout_params2: [248.0 / 255.0, 250.0 / 255.0, 252.0 / 255.0, 1.0],
            layout_params3: [203.0 / 255.0, 213.0 / 255.0, 225.0 / 255.0, 1.0],
            layout_params4: [15.0 / 255.0, 23.0 / 255.0, 42.0 / 255.0, 0.16],
            layout_params5: [0.0, 14.0, 0.0, 0.0],
            layout_params6: [12.0, 12.0, 0.0, 0.0],
            layout_params7: [1.0, 0.0, 0.0, 0.0],
            layout_params8: [0.0; 4],
            layout_params9: [0.0; 4],
            layout_bounds: [75.0, 225.8, 625.0, 570.0],
        }
    }
}

impl UiPhysicalUniforms {
    pub fn update_from_camera(&mut self, camera: &FlyCamera, width: u32, height: u32, time: f32) {
        let aspect = width as f32 / height as f32;
        self.inv_view_proj = camera.inv_view_projection_matrix(aspect).to_cols_array_2d();
        self.camera_pos_time = [
            camera.position().x,
            camera.position().y,
            camera.position().z,
            time,
        ];
        self.render_params[0] = width as f32;
        self.render_params[1] = height as f32;
    }
}

pub struct UiPhysicalPassHost {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    storage_revision: u64,
}

pub struct UiPhysicalFullscreenRenderer {
    pass_host: UiPhysicalPassHost,
    uniform_buffer: wgpu::Buffer,
    theme_buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    scale_factor: f32,
}

pub struct UiPhysicalRuntimeHost {
    renderer: UiPhysicalFullscreenRenderer,
    device: wgpu::Device,
    label: String,
}

pub struct ThemedUiPhysicalHost<S> {
    scene: S,
    runtime_host: UiPhysicalRuntimeHost,
    theme_state: UiPhysicalThemeState,
    classic_decal_prim_start: f32,
}

impl UiPhysicalPassHost {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        label_prefix: &str,
        uniform_buffer: &wgpu::Buffer,
        theme_buffer: &wgpu::Buffer,
        storage_buffers: &UiStorageBuffers,
        storage_revision: u64,
    ) -> Self {
        let bind_group_layout = create_bind_group_layout_with_storage(
            device,
            &format!("{label_prefix} Bind Group Layout"),
            &[
                (0, wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT),
                (1, wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT),
            ],
            &[2, 3, 4, 5, 6, 7, 8, 9],
            wgpu::ShaderStages::FRAGMENT,
        );
        let bind_group = create_bind_group_with_storage(
            device,
            &format!("{label_prefix} Bind Group"),
            &bind_group_layout,
            &[(0, uniform_buffer), (1, theme_buffer)],
            storage_buffers,
            &[2, 3, 4, 5, 6, 7, 8, 9],
        );
        let shader_module = retained_ui_physical_shader::create_shader_module_embed_source(device);
        let pipeline = create_fullscreen_pipeline(
            device,
            surface_format,
            &format!("{label_prefix} Pipeline"),
            &[&bind_group_layout],
            &shader_module,
        );
        Self {
            pipeline,
            bind_group_layout,
            bind_group,
            storage_revision,
        }
    }

    pub fn sync_storage_buffers_if_needed(
        &mut self,
        device: &wgpu::Device,
        label_prefix: &str,
        uniform_buffer: &wgpu::Buffer,
        theme_buffer: &wgpu::Buffer,
        storage_buffers: &UiStorageBuffers,
        storage_revision: u64,
    ) {
        if self.storage_revision == storage_revision {
            return;
        }

        self.bind_group = create_bind_group_with_storage(
            device,
            &format!("{label_prefix} Bind Group"),
            &self.bind_group_layout,
            &[(0, uniform_buffer), (1, theme_buffer)],
            storage_buffers,
            &[2, 3, 4, 5, 6, 7, 8, 9],
        );
        self.storage_revision = storage_revision;
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

impl UiPhysicalFullscreenRenderer {
    pub fn new<T: Pod>(
        ctx: &DemoContext,
        label_prefix: &str,
        initial_uniforms: &UiPhysicalUniforms,
        initial_theme_uniforms: &T,
        storage_buffers: &UiStorageBuffers,
        storage_revision: u64,
    ) -> Self {
        let uniform_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{label_prefix} Uniform Buffer")),
                contents: bytemuck::cast_slice(&[*initial_uniforms]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let theme_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{label_prefix} Theme Buffer")),
                contents: bytemuck::cast_slice(std::slice::from_ref(initial_theme_uniforms)),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let pass_host = UiPhysicalPassHost::new(
            ctx.device,
            ctx.surface_format,
            label_prefix,
            &uniform_buffer,
            &theme_buffer,
            storage_buffers,
            storage_revision,
        );
        Self {
            pass_host,
            uniform_buffer,
            theme_buffer,
            width: ctx.width,
            height: ctx.height,
            scale_factor: ctx.scale_factor,
        }
    }

    pub fn sync_storage_buffers_if_needed(
        &mut self,
        device: &wgpu::Device,
        label_prefix: &str,
        storage_buffers: &UiStorageBuffers,
        storage_revision: u64,
    ) {
        self.pass_host.sync_storage_buffers_if_needed(
            device,
            label_prefix,
            &self.uniform_buffer,
            &self.theme_buffer,
            storage_buffers,
            storage_revision,
        );
    }

    pub fn set_scale_factor(&mut self, scale_factor: f32) {
        self.scale_factor = scale_factor.max(0.5);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    pub fn write_uniforms(
        &self,
        queue: &wgpu::Queue,
        camera: &FlyCamera,
        time: f32,
        char_count: u32,
        ui_prim_count: u32,
        classic_decal_prim_start: f32,
        char_grid_params: [f32; 4],
        char_grid_bounds: [f32; 4],
        layout: UiPhysicalLayout,
        light_dir_intensity: [f32; 4],
    ) {
        let mut uniforms = UiPhysicalUniforms::default();
        uniforms.update_from_camera(camera, self.width, self.height, time);
        uniforms.text_params[0] = char_count as f32;
        uniforms.text_params[1] = ui_prim_count as f32;
        uniforms.text_params[2] = self.scale_factor;
        uniforms.text_params[3] = classic_decal_prim_start;
        uniforms.char_grid_params = char_grid_params;
        uniforms.char_grid_bounds = char_grid_bounds;
        uniforms.layout_params0 = [
            layout.center_px[0],
            layout.center_px[1],
            layout.pixel_to_world,
            match layout.geometry_mode {
                UiPhysicalGeometryMode::GenericCard => 0.0,
                UiPhysicalGeometryMode::StackedCard => 1.0,
            },
        ];
        uniforms.layout_params1 = [
            layout.corner_radius_px,
            layout.elevation_px,
            layout.depth_px,
            0.0,
        ];
        uniforms.layout_params2 = layout.fill_color;
        uniforms.layout_params8 = layout.accent_color;
        uniforms.layout_params9 = layout.detail_color;
        uniforms.layout_params3 = layout.outline_color;
        uniforms.layout_params4 = [
            layout.shadow_color[0],
            layout.shadow_color[1],
            layout.shadow_color[2],
            layout.shadow_color[3],
        ];
        uniforms.layout_params5 = [
            layout.shadow_offset_px[0],
            layout.shadow_offset_px[1],
            0.0,
            0.0,
        ];
        uniforms.layout_params6 = [
            layout.shadow_extra_size_px[0],
            layout.shadow_extra_size_px[1],
            0.0,
            0.0,
        ];
        uniforms.layout_params7 = [layout.outline_width_px, 0.0, 0.0, 0.0];
        uniforms.layout_params7[1] = layout.content_inset_px;
        uniforms.layout_bounds = layout.bounds_px;
        uniforms.light_dir_intensity = light_dir_intensity;
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    pub fn write_theme<T: Pod>(&self, queue: &wgpu::Queue, theme_uniforms: &T) {
        queue.write_buffer(
            &self.theme_buffer,
            0,
            bytemuck::cast_slice(std::slice::from_ref(theme_uniforms)),
        );
    }

    pub fn sync_theme_state(
        &self,
        queue: &wgpu::Queue,
        theme_state: &mut UiPhysicalThemeState,
        char_instances: &[GpuCharInstanceEx],
        char_instances_buffer: &wgpu::Buffer,
    ) {
        if !theme_state.is_dirty() {
            return;
        }

        let theme_uniforms: ThemeUniforms = theme_state.theme_uniforms();
        self.write_theme(queue, &theme_uniforms);

        let updated = recolor_physical_char_instances(char_instances, &theme_state.text_colors());
        queue.write_buffer(char_instances_buffer, 0, bytemuck::cast_slice(&updated));
        theme_state.clear_dirty();
    }

    pub fn uniform_buffer(&self) -> &wgpu::Buffer {
        &self.uniform_buffer
    }

    pub fn theme_buffer(&self) -> &wgpu::Buffer {
        &self.theme_buffer
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.pass_host.render(render_pass);
    }
}

impl UiPhysicalRuntimeHost {
    pub fn new<T: Pod, S: UiPhysicalRuntimeScene>(
        ctx: &DemoContext,
        label: &str,
        initial_theme_uniforms: &T,
        scene: &S,
        classic_decal_prim_start: f32,
        light_dir_intensity: [f32; 4],
    ) -> Self {
        let mut uniforms = UiPhysicalUniforms::default();
        uniforms.text_params[0] = scene.char_count() as f32;
        uniforms.text_params[1] = scene.ui_prim_count() as f32;
        uniforms.text_params[2] = ctx.scale_factor;
        uniforms.text_params[3] = classic_decal_prim_start;
        uniforms.char_grid_params = scene.char_grid_params();
        uniforms.char_grid_bounds = scene.char_grid_bounds();
        let layout = scene.physical_layout();
        uniforms.layout_params0 = [
            layout.center_px[0],
            layout.center_px[1],
            layout.pixel_to_world,
            match layout.geometry_mode {
                UiPhysicalGeometryMode::GenericCard => 0.0,
                UiPhysicalGeometryMode::StackedCard => 1.0,
            },
        ];
        uniforms.layout_params1 = [
            layout.corner_radius_px,
            layout.elevation_px,
            layout.depth_px,
            0.0,
        ];
        uniforms.layout_params2 = layout.fill_color;
        uniforms.layout_params8 = layout.accent_color;
        uniforms.layout_params9 = layout.detail_color;
        uniforms.layout_params3 = layout.outline_color;
        uniforms.layout_params4 = [
            layout.shadow_color[0],
            layout.shadow_color[1],
            layout.shadow_color[2],
            layout.shadow_color[3],
        ];
        uniforms.layout_params5 = [
            layout.shadow_offset_px[0],
            layout.shadow_offset_px[1],
            0.0,
            0.0,
        ];
        uniforms.layout_params6 = [
            layout.shadow_extra_size_px[0],
            layout.shadow_extra_size_px[1],
            0.0,
            0.0,
        ];
        uniforms.layout_params7 = [layout.outline_width_px, 0.0, 0.0, 0.0];
        uniforms.layout_params7[1] = layout.content_inset_px;
        uniforms.layout_bounds = layout.bounds_px;
        uniforms.light_dir_intensity = light_dir_intensity;

        let renderer = UiPhysicalFullscreenRenderer::new(
            ctx,
            label,
            &uniforms,
            initial_theme_uniforms,
            scene.storage_buffers(),
            scene.storage_revision(),
        );

        Self {
            renderer,
            device: ctx.device.clone(),
            label: label.to_string(),
        }
    }

    pub fn sync_scene_resources_if_needed<S: UiPhysicalRuntimeScene>(
        &mut self,
        queue: &wgpu::Queue,
        scene: &mut S,
        theme_state: &UiPhysicalThemeState,
    ) {
        scene.prepare_frame_with_theme(queue, theme_state);
        self.renderer.sync_storage_buffers_if_needed(
            &self.device,
            &self.label,
            scene.storage_buffers(),
            scene.storage_revision(),
        );
    }

    pub fn update_theme_state<S: UiPhysicalRuntimeScene>(
        &self,
        queue: &wgpu::Queue,
        scene: &S,
        theme_state: &mut UiPhysicalThemeState,
    ) {
        scene.sync_theme_state(&self.renderer, queue, theme_state);
    }

    pub fn update_uniforms<S: UiPhysicalRuntimeScene>(
        &self,
        queue: &wgpu::Queue,
        scene: &S,
        camera: &FlyCamera,
        time: f32,
        classic_decal_prim_start: f32,
        light_dir_intensity: [f32; 4],
    ) {
        self.renderer.write_uniforms(
            queue,
            camera,
            time,
            scene.char_count(),
            scene.ui_prim_count(),
            classic_decal_prim_start,
            scene.char_grid_params(),
            scene.char_grid_bounds(),
            scene.physical_layout(),
            light_dir_intensity,
        );
    }

    pub fn set_scale_factor<S: UiPhysicalRuntimeScene>(
        &mut self,
        scene: &mut S,
        scale_factor: f32,
    ) {
        self.renderer.set_scale_factor(scale_factor);
        scene.mark_active_view_transform_dirty();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.renderer.render(render_pass);
    }
}

impl<S: UiPhysicalRuntimeScene> ThemedUiPhysicalHost<S> {
    pub fn new<T: Pod>(
        ctx: &DemoContext,
        label: &str,
        scene: S,
        theme_state: UiPhysicalThemeState,
        classic_decal_prim_start: f32,
        initial_theme_uniforms: &T,
    ) -> Self {
        let runtime_host = UiPhysicalRuntimeHost::new(
            ctx,
            label,
            initial_theme_uniforms,
            &scene,
            classic_decal_prim_start,
            theme_state.light_dir_intensity(),
        );
        Self {
            scene,
            runtime_host,
            theme_state,
            classic_decal_prim_start,
        }
    }

    pub fn scene(&self) -> &S {
        &self.scene
    }

    pub fn scene_mut(&mut self) -> &mut S {
        &mut self.scene
    }

    pub fn theme_state(&self) -> &UiPhysicalThemeState {
        &self.theme_state
    }

    pub fn theme_state_mut(&mut self) -> &mut UiPhysicalThemeState {
        &mut self.theme_state
    }

    pub fn needs_redraw(&self) -> bool {
        self.scene.needs_redraw() || self.theme_state.is_dirty()
    }

    pub fn prepare_frame(&mut self, queue: &wgpu::Queue) {
        self.runtime_host
            .sync_scene_resources_if_needed(queue, &mut self.scene, &self.theme_state);
        self.runtime_host
            .update_theme_state(queue, &self.scene, &mut self.theme_state);
    }

    pub fn update_uniforms(&self, queue: &wgpu::Queue, camera: &FlyCamera, time: f32) {
        self.runtime_host.update_uniforms(
            queue,
            &self.scene,
            camera,
            time,
            self.classic_decal_prim_start,
            self.theme_state.light_dir_intensity(),
        );
    }

    pub fn set_scale_factor(&mut self, scale_factor: f32) {
        self.runtime_host
            .set_scale_factor(&mut self.scene, scale_factor);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.runtime_host.resize(width, height);
        self.scene.resize(width, height);
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.runtime_host.render(render_pass);
    }

    pub fn cycle_theme(&mut self) {
        self.theme_state.cycle_theme();
    }

    pub fn toggle_dark_mode(&mut self) {
        self.theme_state.toggle_dark_mode();
    }

    pub fn set_named_theme(
        &mut self,
        theme: &str,
        dark_mode: Option<bool>,
    ) -> Option<(&'static str, bool)> {
        self.theme_state.set_named_theme(theme, dark_mode)
    }
}

impl<S: SampleSceneDeckTarget + UiPhysicalRuntimeScene> SampleSceneDeckTarget
    for ThemedUiPhysicalHost<S>
{
    fn active_sample_scene_kind(&self) -> crate::retained::samples::SampleSceneKind {
        self.scene.active_sample_scene_kind()
    }

    fn cycle_sample_scene(&mut self) -> bool {
        self.scene.cycle_sample_scene()
    }

    fn apply_active_sample_scene_action(&mut self, action: SampleSceneAction) -> bool {
        self.scene.apply_active_sample_scene_action(action)
    }
}

impl<S: ShowcaseSceneDeckTarget + UiPhysicalRuntimeScene> ShowcaseSceneDeckTarget
    for ThemedUiPhysicalHost<S>
{
    fn cycle_showcase_scene(&mut self) -> bool {
        self.scene.cycle_showcase_scene()
    }

    fn apply_active_showcase_action(&mut self, action: ShowcaseSceneAction) -> bool {
        self.scene.apply_active_showcase_action(action)
    }
}

impl<S: UiPhysicalSceneState> StateBackedUiPhysicalHost<S> {
    fn rebuild_storage_buffers_from_bootstrap(&mut self, bootstrap: UiPhysicalSceneBootstrap) {
        let gpu_font_data = build_font_gpu_data(self.state.atlas());
        self.storage_buffers = create_storage_buffers(
            &self.device,
            &self.queue,
            &gpu_font_data,
            bytemuck::cast_slice(&bootstrap.text_data.char_instances),
            self.config.max_char_instances * std::mem::size_of::<GpuCharInstanceEx>(),
            &bootstrap.text_data.char_grid_cells,
            &bootstrap.text_data.char_grid_indices,
            self.config.max_grid_indices,
            bytemuck::cast_slice(&bootstrap.ui_data.primitives),
            self.config.max_ui_primitives * std::mem::size_of::<GpuUiPrimitive>(),
            &self.config.label,
        );
        self.char_instances = bootstrap.text_data.char_instances;
        self.char_count = bootstrap.text_data.char_count;
        self.ui_prim_count = bootstrap.ui_data.primitive_count as u32;
        self.char_grid_params = bootstrap.text_data.char_grid_params;
        self.char_grid_bounds = bootstrap.text_data.char_grid_bounds;
        self.layout = bootstrap.layout;
        self.storage_revision += 1;
    }

    fn sync_bootstrap_into_buffers(&mut self, bootstrap: UiPhysicalSceneBootstrap) {
        self.queue.write_buffer(
            &self.storage_buffers.char_instances_buffer,
            0,
            bytemuck::cast_slice(&bootstrap.text_data.char_instances),
        );
        self.queue.write_buffer(
            &self.storage_buffers.char_grid_cells_buffer,
            0,
            bytemuck::cast_slice(&bootstrap.text_data.char_grid_cells),
        );
        self.queue.write_buffer(
            &self.storage_buffers.char_grid_indices_buffer,
            0,
            bytemuck::cast_slice(&bootstrap.text_data.char_grid_indices),
        );
        self.queue.write_buffer(
            &self.storage_buffers.ui_primitives_buffer,
            0,
            bytemuck::cast_slice(&bootstrap.ui_data.primitives),
        );
        self.char_instances = bootstrap.text_data.char_instances;
        self.char_count = bootstrap.text_data.char_count;
        self.ui_prim_count = bootstrap.ui_data.primitive_count as u32;
        self.char_grid_params = bootstrap.text_data.char_grid_params;
        self.char_grid_bounds = bootstrap.text_data.char_grid_bounds;
        self.layout = bootstrap.layout;
    }

    pub fn from_bootstrap(
        ctx: &DemoContext,
        state: S,
        colors: TextColors,
        bootstrap: UiPhysicalSceneBootstrap,
        config: UiPhysicalHostConfig,
    ) -> Self {
        let config = config.grown_to_fit(&bootstrap);
        let gpu_font_data = build_font_gpu_data(state.atlas());
        let storage_buffers = create_storage_buffers(
            &ctx.device,
            &ctx.queue,
            &gpu_font_data,
            bytemuck::cast_slice(&bootstrap.text_data.char_instances),
            config.max_char_instances * std::mem::size_of::<GpuCharInstanceEx>(),
            &bootstrap.text_data.char_grid_cells,
            &bootstrap.text_data.char_grid_indices,
            config.max_grid_indices,
            bytemuck::cast_slice(&bootstrap.ui_data.primitives),
            config.max_ui_primitives * std::mem::size_of::<GpuUiPrimitive>(),
            &config.label,
        );

        Self {
            state,
            text_colors: colors,
            storage_buffers,
            char_instances: bootstrap.text_data.char_instances,
            char_count: bootstrap.text_data.char_count,
            ui_prim_count: bootstrap.ui_data.primitive_count as u32,
            char_grid_params: bootstrap.text_data.char_grid_params,
            char_grid_bounds: bootstrap.text_data.char_grid_bounds,
            layout: bootstrap.layout,
            storage_revision: next_storage_revision_seed(),
            config,
            device: ctx.device.clone(),
            queue: ctx.queue.clone(),
        }
    }

    pub fn new(
        ctx: &DemoContext,
        state: S,
        colors: &TextColors,
        config: UiPhysicalHostConfig,
    ) -> Self {
        let bootstrap = state.build_ui_physical_bootstrap(colors);
        Self::from_bootstrap(ctx, state, *colors, bootstrap, config)
    }

    fn rebuild_storage_buffers(&mut self) {
        let bootstrap = self.state.build_ui_physical_bootstrap(&self.text_colors);
        self.config = self.config.grown_to_fit(&bootstrap);
        self.rebuild_storage_buffers_from_bootstrap(bootstrap);
    }

    pub fn replace_state_and_bootstrap(&mut self, state: S, bootstrap: UiPhysicalSceneBootstrap) {
        self.state = state;
        if bootstrap.fits_config(&self.config) {
            self.sync_bootstrap_into_buffers(bootstrap);
        } else {
            self.config = self.config.grown_to_fit(&bootstrap);
            self.rebuild_storage_buffers_from_bootstrap(bootstrap);
        }
    }

    pub fn storage_buffers(&self) -> &UiStorageBuffers {
        &self.storage_buffers
    }

    pub fn state(&self) -> &S {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut S {
        &mut self.state
    }

    pub fn scene(&self) -> &RetainedScene {
        self.state.scene()
    }

    pub fn scene_mut(&mut self) -> &mut RetainedScene {
        self.state.scene_mut()
    }

    pub fn mutate_scene<F>(&mut self, mutate: F) -> bool
    where
        F: FnOnce(&mut RetainedScene) -> bool,
    {
        mutate(self.state.scene_mut())
    }

    pub fn char_instances(&self) -> &[GpuCharInstanceEx] {
        &self.char_instances
    }

    pub fn char_count(&self) -> u32 {
        self.char_count
    }

    pub fn ui_prim_count(&self) -> u32 {
        self.ui_prim_count
    }

    pub fn char_grid_params(&self) -> [f32; 4] {
        self.char_grid_params
    }

    pub fn char_grid_bounds(&self) -> [f32; 4] {
        self.char_grid_bounds
    }

    pub fn physical_layout(&self) -> UiPhysicalLayout {
        self.layout
    }

    pub fn storage_revision(&self) -> u64 {
        self.storage_revision
    }

    pub fn needs_redraw(&self) -> bool {
        !self.state.scene().dirty().is_empty()
    }

    pub fn prepare_frame(&mut self, queue: &wgpu::Queue) {
        let Some(updates) = self
            .state
            .take_ui_physical_resource_update(&self.text_colors)
        else {
            return;
        };

        if updates.needs_full_rebuild() {
            let bootstrap = self.state.build_ui_physical_bootstrap(&self.text_colors);
            if !bootstrap.fits_config(&self.config) {
                self.config = self.config.grown_to_fit(&bootstrap);
                self.rebuild_storage_buffers();
                return;
            }
        }

        if let Some(text_update) = updates.text {
            apply_fixed_text_runtime_update(
                text_update,
                self.state.text_state(),
                &mut self.char_instances,
                &mut self.char_count,
                &mut self.char_grid_params,
                &mut self.char_grid_bounds,
                self.config.max_char_instances,
                self.config.max_grid_indices,
                |instances| instances,
                |instances| {
                    queue.write_buffer(
                        &self.storage_buffers.char_instances_buffer,
                        0,
                        bytemuck::cast_slice(instances),
                    );
                },
                |offset, run_slots| {
                    queue.write_buffer(
                        &self.storage_buffers.char_instances_buffer,
                        (offset * std::mem::size_of::<GpuCharInstanceEx>()) as u64,
                        bytemuck::cast_slice(run_slots),
                    );
                },
                |cells, indices| {
                    queue.write_buffer(
                        &self.storage_buffers.char_grid_cells_buffer,
                        0,
                        bytemuck::cast_slice(cells),
                    );
                    queue.write_buffer(
                        &self.storage_buffers.char_grid_indices_buffer,
                        0,
                        bytemuck::cast_slice(indices),
                    );
                },
                |cell_idx, cell| {
                    queue.write_buffer(
                        &self.storage_buffers.char_grid_cells_buffer,
                        (cell_idx * std::mem::size_of::<CharGridCell>()) as u64,
                        bytemuck::bytes_of(cell),
                    );
                },
                |offset, indices| {
                    queue.write_buffer(
                        &self.storage_buffers.char_grid_indices_buffer,
                        (offset * std::mem::size_of::<u32>()) as u64,
                        bytemuck::cast_slice(indices),
                    );
                },
            );
        }

        if let Some(ui_update) = updates.ui {
            apply_gpu_ui_runtime_update(
                ui_update,
                &mut self.ui_prim_count,
                self.config.max_ui_primitives,
                |primitives| {
                    queue.write_buffer(
                        &self.storage_buffers.ui_primitives_buffer,
                        0,
                        bytemuck::cast_slice(primitives),
                    );
                },
                |offset, primitives| {
                    queue.write_buffer(
                        &self.storage_buffers.ui_primitives_buffer,
                        (offset * std::mem::size_of::<GpuUiPrimitive>()) as u64,
                        bytemuck::cast_slice(primitives),
                    );
                },
            );
        }
    }

    pub fn mark_view_transform_dirty(&mut self) {
        self.state.mark_view_transform_dirty();
    }

    pub fn set_text_colors(&mut self, text_colors: TextColors) {
        self.text_colors = text_colors;
    }

    pub fn set_viewport_size(&mut self, width: u32, height: u32) {
        self.state.set_viewport_size(width, height);
    }
}

impl FixedUiPhysicalSceneState {
    pub fn from_built_scene(
        built: BuiltFixedUi2dScene,
        atlas: Arc<VectorFontAtlas>,
        text_render_space: TextRenderSpace,
        ui_render_space: UiRenderSpace,
    ) -> (Self, UiPhysicalSceneBootstrap) {
        let layout = UiPhysicalLayout::generic_from_scene(
            &built.state.scene,
            &built.init.text_data,
            &built.init.ui_data,
        );
        let bootstrap = UiPhysicalSceneBootstrap {
            text_data: built.init.text_data,
            ui_data: built.init.ui_data,
            layout,
        };
        (
            Self {
                scene_state: built.state,
                atlas,
                text_render_space,
                ui_render_space,
            },
            bootstrap,
        )
    }
}

impl UiPhysicalSceneState for FixedUiPhysicalSceneState {
    fn atlas(&self) -> &VectorFontAtlas {
        &self.atlas
    }

    fn text_state(&self) -> &FixedTextSceneState {
        &self.scene_state.text_state
    }

    fn build_ui_physical_bootstrap(&self, colors: &TextColors) -> UiPhysicalSceneBootstrap {
        let (_, text_data) = build_fixed_text_scene_state_for_scene(
            &self.scene_state.scene,
            self.scene_state.text_state.layout(),
            &self.atlas,
            colors,
            self.text_render_space,
        );
        let ui_data = build_gpu_ui_scene(&self.scene_state.scene, self.ui_render_space);
        let layout =
            UiPhysicalLayout::generic_from_scene(&self.scene_state.scene, &text_data, &ui_data);
        UiPhysicalSceneBootstrap {
            text_data,
            ui_data,
            layout,
        }
    }

    fn take_ui_physical_resource_update(
        &mut self,
        colors: &TextColors,
    ) -> Option<UiPhysicalRuntimeUpdate> {
        self.scene_state
            .take_update(
                &self.atlas,
                colors,
                self.text_render_space,
                self.ui_render_space,
            )
            .map(Into::into)
    }

    fn mark_view_transform_dirty(&mut self) {
        let root = self.scene_state.scene.root();
        self.scene_state.scene.mark_node_dirty(root);
    }

    fn set_viewport_size(&mut self, width: u32, height: u32) {
        let _ = (width, height);
        let root = self.scene_state.scene.root();
        self.scene_state.scene.mark_node_dirty(root);
    }

    fn scene(&self) -> &RetainedScene {
        &self.scene_state.scene
    }

    fn scene_mut(&mut self) -> &mut RetainedScene {
        &mut self.scene_state.scene
    }

    fn physical_layout(&self) -> UiPhysicalLayout {
        let (_, text_data) = build_fixed_text_scene_state_for_scene(
            &self.scene_state.scene,
            self.scene_state.text_state.layout(),
            &self.atlas,
            &TextColors {
                heading: [0.0; 3],
                active: [0.0; 3],
                completed: [0.0; 3],
                placeholder: [0.0; 3],
                body: [0.0; 3],
                info: [0.0; 3],
            },
            self.text_render_space,
        );
        let ui_data = build_gpu_ui_scene(&self.scene_state.scene, self.ui_render_space);
        UiPhysicalLayout::generic_from_scene(&self.scene_state.scene, &text_data, &ui_data)
    }
}

impl FixedUiPhysicalSceneHost {
    pub fn from_built_scene(
        ctx: &DemoContext,
        built: BuiltFixedUi2dScene,
        atlas: Arc<VectorFontAtlas>,
        text_colors: TextColors,
        text_render_space: TextRenderSpace,
        ui_render_space: UiRenderSpace,
        config: UiPhysicalHostConfig,
    ) -> Self {
        let (state, bootstrap) = FixedUiPhysicalSceneState::from_built_scene(
            built,
            atlas,
            text_render_space,
            ui_render_space,
        );
        Self {
            state_host: StateBackedUiPhysicalHost::from_bootstrap(
                ctx,
                state,
                text_colors,
                bootstrap,
                config,
            ),
        }
    }

    pub fn scene(&self) -> &RetainedScene {
        self.state_host.scene()
    }

    pub fn scene_mut(&mut self) -> &mut RetainedScene {
        self.state_host.scene_mut()
    }

    pub fn storage_buffers(&self) -> &UiStorageBuffers {
        self.state_host.storage_buffers()
    }

    pub fn char_instances(&self) -> &[GpuCharInstanceEx] {
        self.state_host.char_instances()
    }

    pub fn char_count(&self) -> u32 {
        self.state_host.char_count()
    }

    pub fn ui_prim_count(&self) -> u32 {
        self.state_host.ui_prim_count()
    }

    pub fn char_grid_params(&self) -> [f32; 4] {
        self.state_host.char_grid_params()
    }

    pub fn char_grid_bounds(&self) -> [f32; 4] {
        self.state_host.char_grid_bounds()
    }

    pub fn physical_layout(&self) -> UiPhysicalLayout {
        self.state_host.physical_layout()
    }

    pub fn storage_revision(&self) -> u64 {
        self.state_host.storage_revision()
    }

    pub fn needs_redraw(&self) -> bool {
        self.state_host.needs_redraw()
    }

    pub fn prepare_frame(&mut self, queue: &wgpu::Queue) {
        self.state_host.prepare_frame(queue);
    }

    pub fn set_text_colors(&mut self, text_colors: TextColors) {
        self.state_host.set_text_colors(text_colors);
    }

    pub fn replace_built_scene(&mut self, built: BuiltFixedUi2dScene) {
        let atlas = self.state_host.state().atlas.clone();
        let text_render_space = self.state_host.state().text_render_space;
        let ui_render_space = self.state_host.state().ui_render_space;
        let (state, bootstrap) = FixedUiPhysicalSceneState::from_built_scene(
            built,
            atlas,
            text_render_space,
            ui_render_space,
        );
        self.state_host
            .replace_state_and_bootstrap(state, bootstrap);
    }

    pub fn mark_view_transform_dirty(&mut self) {
        self.state_host.mark_view_transform_dirty();
    }

    pub fn set_viewport_size(&mut self, width: u32, height: u32) {
        self.state_host.set_viewport_size(width, height);
    }
}

impl<M: FixedUi2dSceneModelBuilder> ModeledFixedUiPhysicalSceneHost<M> {
    pub fn new(
        ctx: &DemoContext,
        model: M,
        atlas: Arc<VectorFontAtlas>,
        text_colors: TextColors,
        text_render_space: TextRenderSpace,
        ui_render_space: UiRenderSpace,
        config: UiPhysicalHostConfig,
    ) -> Self {
        let built = model.build_fixed_ui2d_scene(
            [ctx.width, ctx.height],
            &atlas,
            &text_colors,
            text_render_space,
            ui_render_space,
        );
        let host = FixedUiPhysicalSceneHost::from_built_scene(
            ctx,
            built,
            atlas,
            text_colors,
            text_render_space,
            ui_render_space,
            config,
        );
        Self {
            model,
            host,
            viewport_size: [ctx.width, ctx.height],
            text_colors,
            needs_rebuild: false,
        }
    }

    pub fn scene(&self) -> &RetainedScene {
        self.host.scene()
    }

    pub fn scene_mut(&mut self) -> &mut RetainedScene {
        self.host.scene_mut()
    }

    pub fn model(&self) -> &M {
        &self.model
    }

    pub fn model_mut(&mut self) -> &mut M {
        &mut self.model
    }

    pub fn host(&self) -> &FixedUiPhysicalSceneHost {
        &self.host
    }

    pub fn host_mut(&mut self) -> &mut FixedUiPhysicalSceneHost {
        &mut self.host
    }

    pub fn resize_lazy(&mut self, width: u32, height: u32) {
        self.viewport_size = [width, height];
        self.needs_rebuild = true;
    }

    pub fn resize_and_rebuild(&mut self, width: u32, height: u32) {
        self.viewport_size = [width, height];
        self.rebuild();
    }

    pub fn replace_model_and_rebuild(&mut self, model: M) {
        self.model = model;
        self.rebuild();
    }

    pub fn rebuild(&mut self) {
        let state = self.host.state_host.state();
        let built = self.model.build_fixed_ui2d_scene(
            self.viewport_size,
            state.atlas(),
            &self.text_colors,
            state.text_render_space,
            state.ui_render_space,
        );
        self.host
            .set_viewport_size(self.viewport_size[0], self.viewport_size[1]);
        self.host.replace_built_scene(built);
        self.needs_rebuild = false;
    }

    pub fn set_text_colors(&mut self, text_colors: TextColors) {
        self.text_colors = text_colors;
        self.host.set_text_colors(text_colors);
    }

    pub fn mark_view_transform_dirty(&mut self) {
        self.host.mark_view_transform_dirty();
    }

    pub fn storage_buffers(&self) -> &UiStorageBuffers {
        self.host.storage_buffers()
    }

    pub fn char_instances(&self) -> &[GpuCharInstanceEx] {
        self.host.char_instances()
    }

    pub fn char_count(&self) -> u32 {
        self.host.char_count()
    }

    pub fn ui_prim_count(&self) -> u32 {
        self.host.ui_prim_count()
    }

    pub fn char_grid_params(&self) -> [f32; 4] {
        self.host.char_grid_params()
    }

    pub fn char_grid_bounds(&self) -> [f32; 4] {
        self.host.char_grid_bounds()
    }

    pub fn physical_layout(&self) -> UiPhysicalLayout {
        self.host.physical_layout()
    }

    pub fn storage_revision(&self) -> u64 {
        self.host.storage_revision()
    }

    pub fn needs_redraw(&self) -> bool {
        self.needs_rebuild || self.host.needs_redraw()
    }

    pub fn ensure_ready(&mut self) {
        if self.needs_rebuild {
            self.rebuild();
        }
    }

    pub fn prepare_frame(&mut self, queue: &wgpu::Queue) {
        self.ensure_ready();
        self.host.prepare_frame(queue);
    }
}

impl<M: FixedUi2dSceneModelBuilder> UiPhysicalDeckHost for ModeledFixedUiPhysicalSceneHost<M> {
    fn needs_redraw(&self) -> bool {
        ModeledFixedUiPhysicalSceneHost::needs_redraw(self)
    }

    fn prepare_frame(&mut self, queue: &wgpu::Queue) {
        ModeledFixedUiPhysicalSceneHost::prepare_frame(self, queue);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.resize_and_rebuild(width, height);
    }

    fn resize_lazy(&mut self, width: u32, height: u32) {
        ModeledFixedUiPhysicalSceneHost::resize_lazy(self, width, height);
    }

    fn ensure_ready(&mut self) {
        ModeledFixedUiPhysicalSceneHost::ensure_ready(self);
    }

    fn set_text_colors(&mut self, text_colors: TextColors) {
        ModeledFixedUiPhysicalSceneHost::set_text_colors(self, text_colors);
    }

    fn storage_buffers(&self) -> &UiStorageBuffers {
        ModeledFixedUiPhysicalSceneHost::storage_buffers(self)
    }

    fn char_instances(&self) -> &[GpuCharInstanceEx] {
        ModeledFixedUiPhysicalSceneHost::char_instances(self)
    }
}

impl<S: UiPhysicalSceneState> UiPhysicalDeckHost for StateBackedUiPhysicalHost<S> {
    fn needs_redraw(&self) -> bool {
        StateBackedUiPhysicalHost::needs_redraw(self)
    }

    fn prepare_frame(&mut self, queue: &wgpu::Queue) {
        StateBackedUiPhysicalHost::prepare_frame(self, queue);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.set_viewport_size(width, height);
    }

    fn resize_lazy(&mut self, width: u32, height: u32) {
        self.set_viewport_size(width, height);
    }

    fn ensure_ready(&mut self) {}

    fn set_text_colors(&mut self, text_colors: TextColors) {
        StateBackedUiPhysicalHost::set_text_colors(self, text_colors);
    }

    fn storage_buffers(&self) -> &UiStorageBuffers {
        StateBackedUiPhysicalHost::storage_buffers(self)
    }

    fn char_instances(&self) -> &[GpuCharInstanceEx] {
        StateBackedUiPhysicalHost::char_instances(self)
    }
}

impl<M: FixedUi2dSceneModelBuilder + FixedUi2dSceneModelCapture>
    ModeledFixedUiPhysicalSceneHost<M>
{
    pub fn capture_model_from_scene(&mut self) {
        self.model.capture_from_scene(self.host.scene());
    }

    pub fn mutate_scene_and_capture<F>(&mut self, mutate: F) -> bool
    where
        F: FnOnce(&mut RetainedScene) -> bool,
    {
        let changed = mutate(self.host.scene_mut());
        if changed {
            self.capture_model_from_scene();
        }
        changed
    }
}

impl<H: UiPhysicalDeckHost> UiPhysicalSceneDeck<H> {
    pub fn new(scenes: Vec<H>) -> Self {
        assert!(
            !scenes.is_empty(),
            "UiPhysical scene deck requires at least one scene"
        );
        Self {
            scenes,
            active_scene: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.scenes.len()
    }

    pub fn active_index(&self) -> usize {
        self.active_scene
    }

    pub fn active_host(&self) -> &H {
        &self.scenes[self.active_scene]
    }

    pub fn needs_redraw(&self) -> bool {
        self.active_host().needs_redraw()
    }

    pub fn active_host_mut(&mut self) -> &mut H {
        &mut self.scenes[self.active_scene]
    }

    pub fn cycle_next(&mut self) -> bool {
        let next_scene = (self.active_scene + 1) % self.scenes.len();
        self.set_active_scene(next_scene)
    }

    pub fn set_active_scene(&mut self, index: usize) -> bool {
        if index >= self.scenes.len() || index == self.active_scene {
            return false;
        }

        self.active_scene = index;
        self.ensure_active_scene_ready();
        true
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        for (index, scene) in self.scenes.iter_mut().enumerate() {
            if index == self.active_scene {
                scene.resize(width, height);
            } else {
                scene.resize_lazy(width, height);
            }
        }
    }

    pub fn ensure_active_scene_ready(&mut self) {
        self.active_host_mut().ensure_ready();
    }

    pub fn prepare_frame(&mut self, queue: &wgpu::Queue) {
        self.ensure_active_scene_ready();
        self.active_host_mut().prepare_frame(queue);
    }

    pub fn prepare_frame_with_theme(
        &mut self,
        queue: &wgpu::Queue,
        theme_state: &UiPhysicalThemeState,
    ) {
        self.set_text_colors_if_dirty(theme_state);
        self.prepare_frame(queue);
    }

    pub fn set_text_colors(&mut self, text_colors: TextColors) {
        for scene in &mut self.scenes {
            scene.set_text_colors(text_colors);
        }
    }

    pub fn set_text_colors_if_dirty(&mut self, theme_state: &UiPhysicalThemeState) {
        if theme_state.is_dirty() {
            self.set_text_colors(theme_state.text_colors());
        }
    }

    pub fn sync_theme_state(
        &self,
        renderer: &UiPhysicalFullscreenRenderer,
        queue: &wgpu::Queue,
        theme_state: &mut UiPhysicalThemeState,
    ) {
        renderer.sync_theme_state(
            queue,
            theme_state,
            self.active_host().char_instances(),
            &self.active_host().storage_buffers().char_instances_buffer,
        );
    }
}

impl<M: FixedUi2dSceneModelBuilder> UiPhysicalSceneDeck<ModeledFixedUiPhysicalSceneHost<M>> {
    pub fn model(&self) -> &M {
        self.active_host().model()
    }

    pub fn model_mut(&mut self) -> &mut M {
        self.active_host_mut().model_mut()
    }

    pub fn scene(&self) -> &RetainedScene {
        self.active_host().scene()
    }

    pub fn scene_mut(&mut self) -> &mut RetainedScene {
        self.active_host_mut().scene_mut()
    }

    pub fn storage_buffers(&self) -> &UiStorageBuffers {
        self.active_host().storage_buffers()
    }

    pub fn char_instances(&self) -> &[GpuCharInstanceEx] {
        self.active_host().char_instances()
    }

    pub fn char_count(&self) -> u32 {
        self.active_host().char_count()
    }

    pub fn ui_prim_count(&self) -> u32 {
        self.active_host().ui_prim_count()
    }

    pub fn char_grid_params(&self) -> [f32; 4] {
        self.active_host().char_grid_params()
    }

    pub fn char_grid_bounds(&self) -> [f32; 4] {
        self.active_host().char_grid_bounds()
    }

    pub fn physical_layout(&self) -> UiPhysicalLayout {
        self.active_host().physical_layout()
    }

    pub fn storage_revision(&self) -> u64 {
        self.active_host().storage_revision()
    }

    pub fn mark_active_view_transform_dirty(&mut self) {
        self.active_host_mut().mark_view_transform_dirty();
    }
}

impl<M: FixedUi2dSceneModelBuilder> UiPhysicalRuntimeScene
    for UiPhysicalSceneDeck<ModeledFixedUiPhysicalSceneHost<M>>
{
    fn needs_redraw(&self) -> bool {
        self.active_host().needs_redraw()
    }

    fn prepare_frame_with_theme(
        &mut self,
        queue: &wgpu::Queue,
        theme_state: &UiPhysicalThemeState,
    ) {
        UiPhysicalSceneDeck::prepare_frame_with_theme(self, queue, theme_state);
    }

    fn sync_theme_state(
        &self,
        renderer: &UiPhysicalFullscreenRenderer,
        queue: &wgpu::Queue,
        theme_state: &mut UiPhysicalThemeState,
    ) {
        UiPhysicalSceneDeck::sync_theme_state(self, renderer, queue, theme_state);
    }

    fn storage_buffers(&self) -> &UiStorageBuffers {
        self.active_host().storage_buffers()
    }

    fn storage_revision(&self) -> u64 {
        self.active_host().storage_revision()
    }

    fn char_count(&self) -> u32 {
        self.active_host().char_count()
    }

    fn ui_prim_count(&self) -> u32 {
        self.active_host().ui_prim_count()
    }

    fn char_grid_params(&self) -> [f32; 4] {
        self.active_host().char_grid_params()
    }

    fn char_grid_bounds(&self) -> [f32; 4] {
        self.active_host().char_grid_bounds()
    }

    fn physical_layout(&self) -> UiPhysicalLayout {
        self.active_host().physical_layout()
    }

    fn mark_active_view_transform_dirty(&mut self) {
        self.active_host_mut().mark_view_transform_dirty();
    }

    fn resize(&mut self, width: u32, height: u32) {
        UiPhysicalSceneDeck::resize(self, width, height);
    }
}

impl<M: FixedUi2dSceneModelBuilder + FixedUi2dSceneModelCapture>
    UiPhysicalSceneDeck<ModeledFixedUiPhysicalSceneHost<M>>
{
    pub fn capture_active_model_from_scene(&mut self) {
        self.active_host_mut().capture_model_from_scene();
    }

    pub fn mutate_active_scene_and_capture<F>(&mut self, mutate: F) -> bool
    where
        F: FnOnce(&mut RetainedScene) -> bool,
    {
        self.active_host_mut().mutate_scene_and_capture(mutate)
    }
}

impl SampleSceneDeckTarget
    for UiPhysicalSceneDeck<ModeledFixedUiPhysicalSceneHost<SampleSceneModel>>
{
    fn active_sample_scene_kind(&self) -> crate::retained::samples::SampleSceneKind {
        self.model().kind
    }

    fn cycle_sample_scene(&mut self) -> bool {
        self.cycle_next()
    }

    fn apply_active_sample_scene_action(&mut self, action: SampleSceneAction) -> bool {
        let model = *self.model();
        self.mutate_active_scene_and_capture(|scene| model.apply_action(scene, action))
    }
}

impl ShowcaseSceneDeckTarget
    for UiPhysicalSceneDeck<ModeledFixedUiPhysicalSceneHost<ShowcaseSceneModel>>
{
    fn cycle_showcase_scene(&mut self) -> bool {
        self.cycle_next()
    }

    fn apply_active_showcase_action(&mut self, action: ShowcaseSceneAction) -> bool {
        let model = self.model().clone();
        self.mutate_active_scene_and_capture(|scene| model.apply_action(scene, action))
    }
}

impl<S: UiPhysicalSceneState> UiPhysicalSceneDeck<StateBackedUiPhysicalHost<S>> {
    pub fn active_state(&self) -> &S {
        self.active_host().state()
    }

    pub fn active_state_mut(&mut self) -> &mut S {
        self.active_host_mut().state_mut()
    }

    pub fn active_scene(&self) -> &RetainedScene {
        self.active_host().scene()
    }

    pub fn active_scene_mut(&mut self) -> &mut RetainedScene {
        self.active_host_mut().scene_mut()
    }

    pub fn storage_buffers(&self) -> &UiStorageBuffers {
        self.active_host().storage_buffers()
    }

    pub fn char_instances(&self) -> &[GpuCharInstanceEx] {
        self.active_host().char_instances()
    }

    pub fn char_count(&self) -> u32 {
        self.active_host().char_count()
    }

    pub fn ui_prim_count(&self) -> u32 {
        self.active_host().ui_prim_count()
    }

    pub fn char_grid_params(&self) -> [f32; 4] {
        self.active_host().char_grid_params()
    }

    pub fn char_grid_bounds(&self) -> [f32; 4] {
        self.active_host().char_grid_bounds()
    }

    pub fn physical_layout(&self) -> UiPhysicalLayout {
        self.active_host().physical_layout()
    }

    pub fn storage_revision(&self) -> u64 {
        self.active_host().storage_revision()
    }

    pub fn mark_active_view_transform_dirty(&mut self) {
        self.active_host_mut().mark_view_transform_dirty();
    }

    pub fn mutate_active_scene<F>(&mut self, mutate: F) -> bool
    where
        F: FnOnce(&mut RetainedScene) -> bool,
    {
        self.active_host_mut().mutate_scene(mutate)
    }
}

impl<S: UiPhysicalSceneState> UiPhysicalRuntimeScene
    for UiPhysicalSceneDeck<StateBackedUiPhysicalHost<S>>
{
    fn needs_redraw(&self) -> bool {
        self.active_host().needs_redraw()
    }

    fn prepare_frame_with_theme(
        &mut self,
        queue: &wgpu::Queue,
        theme_state: &UiPhysicalThemeState,
    ) {
        UiPhysicalSceneDeck::prepare_frame_with_theme(self, queue, theme_state);
    }

    fn sync_theme_state(
        &self,
        renderer: &UiPhysicalFullscreenRenderer,
        queue: &wgpu::Queue,
        theme_state: &mut UiPhysicalThemeState,
    ) {
        UiPhysicalSceneDeck::sync_theme_state(self, renderer, queue, theme_state);
    }

    fn storage_buffers(&self) -> &UiStorageBuffers {
        self.active_host().storage_buffers()
    }

    fn storage_revision(&self) -> u64 {
        self.active_host().storage_revision()
    }

    fn char_count(&self) -> u32 {
        self.active_host().char_count()
    }

    fn ui_prim_count(&self) -> u32 {
        self.active_host().ui_prim_count()
    }

    fn char_grid_params(&self) -> [f32; 4] {
        self.active_host().char_grid_params()
    }

    fn char_grid_bounds(&self) -> [f32; 4] {
        self.active_host().char_grid_bounds()
    }

    fn physical_layout(&self) -> UiPhysicalLayout {
        self.active_host().physical_layout()
    }

    fn mark_active_view_transform_dirty(&mut self) {
        self.active_host_mut().mark_view_transform_dirty();
    }

    fn resize(&mut self, width: u32, height: u32) {
        UiPhysicalSceneDeck::resize(self, width, height);
    }
}

impl<M> NamedScrollTarget for ModeledFixedUiPhysicalSceneHost<M>
where
    M: FixedUi2dSceneModelBuilder + FixedUi2dSceneModelCapture + NamedScrollSceneModel,
{
    fn set_named_scroll_offset(&mut self, name: &str, offset_y: f32) -> bool {
        let changed = {
            let model = &self.model;
            let scene = self.host.scene_mut();
            model.set_named_scroll_offset(scene, name, offset_y)
        };
        if changed {
            self.capture_model_from_scene();
        }
        changed
    }
}

impl<S: UiPhysicalSceneState + NamedScrollTarget> NamedScrollTarget
    for StateBackedUiPhysicalHost<S>
{
    fn set_named_scroll_offset(&mut self, name: &str, offset_y: f32) -> bool {
        self.state_mut().set_named_scroll_offset(name, offset_y)
    }
}

impl<S: UiPhysicalSceneState + ListCommandTarget> ListCommandTarget
    for StateBackedUiPhysicalHost<S>
{
    fn toggle_item(&mut self, index: usize) -> bool {
        self.state_mut().toggle_item(index)
    }

    fn set_item_completed(&mut self, index: usize, completed: bool) -> bool {
        self.state_mut().set_item_completed(index, completed)
    }

    fn set_item_label(&mut self, index: usize, label: &str) -> bool {
        self.state_mut().set_item_label(index, label)
    }

    fn set_filter(&mut self, filter: ListFilter) -> bool {
        self.state_mut().set_filter(filter)
    }

    fn set_scroll_offset(&mut self, offset_y: f32) {
        self.state_mut().set_scroll_offset(offset_y);
    }
}

impl<H: UiPhysicalDeckHost + NamedScrollTarget> NamedScrollTarget for UiPhysicalSceneDeck<H> {
    fn set_named_scroll_offset(&mut self, name: &str, offset_y: f32) -> bool {
        self.active_host_mut()
            .set_named_scroll_offset(name, offset_y)
    }
}

impl<H: UiPhysicalDeckHost + ListCommandTarget> ListCommandTarget for UiPhysicalSceneDeck<H> {
    fn toggle_item(&mut self, index: usize) -> bool {
        self.active_host_mut().toggle_item(index)
    }

    fn set_item_completed(&mut self, index: usize, completed: bool) -> bool {
        self.active_host_mut().set_item_completed(index, completed)
    }

    fn set_item_label(&mut self, index: usize, label: &str) -> bool {
        self.active_host_mut().set_item_label(index, label)
    }

    fn set_filter(&mut self, filter: ListFilter) -> bool {
        self.active_host_mut().set_filter(filter)
    }

    fn set_scroll_offset(&mut self, offset_y: f32) {
        self.active_host_mut().set_scroll_offset(offset_y);
    }
}

impl<S: NamedScrollTarget + UiPhysicalRuntimeScene> NamedScrollTarget for ThemedUiPhysicalHost<S> {
    fn set_named_scroll_offset(&mut self, name: &str, offset_y: f32) -> bool {
        self.scene.set_named_scroll_offset(name, offset_y)
    }
}

impl<S: ListCommandTarget + UiPhysicalRuntimeScene> ListCommandTarget for ThemedUiPhysicalHost<S> {
    fn toggle_item(&mut self, index: usize) -> bool {
        self.scene.toggle_item(index)
    }

    fn set_item_completed(&mut self, index: usize, completed: bool) -> bool {
        self.scene.set_item_completed(index, completed)
    }

    fn set_item_label(&mut self, index: usize, label: &str) -> bool {
        self.scene.set_item_label(index, label)
    }

    fn set_filter(&mut self, filter: ListFilter) -> bool {
        self.scene.set_filter(filter)
    }

    fn set_scroll_offset(&mut self, offset_y: f32) {
        self.scene.set_scroll_offset(offset_y);
    }
}

pub fn create_showcase_ui_physical_deck(
    ctx: &DemoContext,
    atlas: Arc<VectorFontAtlas>,
    text_colors: TextColors,
    text_render_space: TextRenderSpace,
    ui_render_space: UiRenderSpace,
    config: UiPhysicalHostConfig,
) -> ShowcaseUiPhysicalDeck {
    let scenes = ShowcaseSceneModel::default_deck_models(crate::retained::SceneMode::UiPhysical)
        .into_iter()
        .map(|model| {
            ModeledFixedUiPhysicalSceneHost::new(
                ctx,
                model,
                atlas.clone(),
                text_colors,
                text_render_space,
                ui_render_space,
                config.clone(),
            )
        })
        .collect();
    UiPhysicalSceneDeck::new(scenes)
}

pub fn create_showcase_ui_physical_host(
    ctx: &DemoContext,
    label: &str,
    atlas: Arc<VectorFontAtlas>,
    theme_state: UiPhysicalThemeState,
    text_render_space: TextRenderSpace,
    ui_render_space: UiRenderSpace,
    config: UiPhysicalHostConfig,
    classic_decal_prim_start: f32,
) -> ThemedUiPhysicalHost<ShowcaseUiPhysicalDeck> {
    let text_colors = tune_generic_ui_physical_text_colors(theme_state.text_colors());
    let theme_uniforms = theme_state.theme_uniforms();
    let deck = create_showcase_ui_physical_deck(
        ctx,
        atlas,
        text_colors,
        text_render_space,
        ui_render_space,
        config,
    );
    ThemedUiPhysicalHost::new(
        ctx,
        label,
        deck,
        theme_state,
        classic_decal_prim_start,
        &theme_uniforms,
    )
}

pub fn create_wrapped_text_ui_physical_deck(
    ctx: &DemoContext,
    model: WrappedTextSceneModel,
    atlas: Arc<VectorFontAtlas>,
    text_colors: TextColors,
    text_render_space: TextRenderSpace,
    ui_render_space: UiRenderSpace,
    config: UiPhysicalHostConfig,
) -> WrappedTextUiPhysicalDeck {
    UiPhysicalSceneDeck::new(vec![ModeledFixedUiPhysicalSceneHost::new(
        ctx,
        model,
        atlas,
        text_colors,
        text_render_space,
        ui_render_space,
        config,
    )])
}

pub fn create_wrapped_text_ui_physical_host(
    ctx: &DemoContext,
    label: &str,
    model: WrappedTextSceneModel,
    atlas: Arc<VectorFontAtlas>,
    theme_state: UiPhysicalThemeState,
    text_render_space: TextRenderSpace,
    ui_render_space: UiRenderSpace,
    config: UiPhysicalHostConfig,
    classic_decal_prim_start: f32,
) -> ThemedUiPhysicalHost<WrappedTextUiPhysicalDeck> {
    let text_colors = tune_generic_ui_physical_text_colors(theme_state.text_colors());
    let theme_uniforms = theme_state.theme_uniforms();
    let deck = create_wrapped_text_ui_physical_deck(
        ctx,
        model,
        atlas,
        text_colors,
        text_render_space,
        ui_render_space,
        config,
    );
    ThemedUiPhysicalHost::new(
        ctx,
        label,
        deck,
        theme_state,
        classic_decal_prim_start,
        &theme_uniforms,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retained::samples::{
        build_scrolling_feed_scene, build_settings_panel_scene, sample_text_run_layout,
        scrolling_feed_text_run_layout, toggle_settings_panel_state,
    };
    use crate::retained::text::FixedTextScenePatch;
    use crate::retained::text_scene::{
        build_wrapped_text_scene, TextSceneBlock, WrappedTextSceneConfig,
    };
    use crate::text::{VectorFont, VectorFontAtlas};
    use std::sync::Arc;

    fn load_test_atlas() -> Arc<VectorFontAtlas> {
        let font_data = std::fs::read("assets/fonts/DejaVuSans.ttf").expect("load test font");
        let font = VectorFont::from_ttf(&font_data).expect("parse test font");
        Arc::new(VectorFontAtlas::from_font(&font, 32))
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
    fn physical_bootstrap_fit_check_uses_config_limits() {
        let bootstrap = UiPhysicalSceneBootstrap {
            text_data: FixedTextSceneData {
                char_instances: vec![
                    GpuCharInstanceEx {
                        pos_and_char: [0.0; 4],
                        color_flags: [0.0; 4],
                    };
                    8
                ],
                char_count: 8,
                char_grid_params: [0.0; 4],
                char_grid_bounds: [0.0; 4],
                char_grid_cells: vec![CharGridCell {
                    offset: 0,
                    count: 0,
                }],
                char_grid_indices: vec![0; 16],
            },
            ui_data: GpuUiSceneData {
                primitives: vec![
                    GpuUiPrimitive {
                        pos_size: [0.0; 4],
                        color: [0.0; 4],
                        params: [0.0; 4],
                        extra: [0.0; 4],
                    };
                    4
                ],
                primitive_count: 4,
            },
            layout: UiPhysicalLayout {
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
            },
        };

        let fit = UiPhysicalHostConfig {
            label: "fit".to_string(),
            max_char_instances: 8,
            max_ui_primitives: 4,
            max_grid_indices: 16,
            grid_cell_capacity: 4,
        };
        let overflow = UiPhysicalHostConfig {
            label: "overflow".to_string(),
            max_char_instances: 7,
            max_ui_primitives: 4,
            max_grid_indices: 16,
            grid_cell_capacity: 4,
        };

        assert!(bootstrap.fits_config(&fit));
        assert!(!bootstrap.fits_config(&overflow));
    }

    #[test]
    fn generic_physical_layout_prefers_clip_frame_bounds() {
        let atlas = load_test_atlas();
        let built = build_wrapped_text_scene(
            &atlas,
            WrappedTextSceneConfig {
                scene_mode: crate::retained::SceneMode::UiPhysical,
                heading: Some(TextSceneBlock {
                    text: "Header",
                    font_size: 28.0,
                    role: crate::retained::TextRole::Heading,
                }),
                body: TextSceneBlock {
                    text: "Short body text",
                    font_size: 18.0,
                    role: crate::retained::TextRole::Body,
                },
                width: 900.0,
                height: 700.0,
                frame_size: Some([360.0, 240.0]),
                margin: 28.0,
                body_line_height: 24.0,
                body_top_padding: 18.0,
                scroll_offset: 0.0,
                grid_dims: [128, 128],
                grid_cell_capacity: 32,
                clip_name: "clip",
                scroll_name: "scroll",
                heading_name: "heading",
                line_name_prefix: "line_",
            },
        );
        let colors = colors();
        let (_text_state, text_data) = build_fixed_text_scene_state_for_scene(
            &built.scene,
            built.layout(),
            &atlas,
            &colors,
            text_space(),
        );
        let ui_data = build_gpu_ui_scene(&built.scene, ui_space());

        let layout = UiPhysicalLayout::generic_from_scene(&built.scene, &text_data, &ui_data);

        let width = layout.bounds_px[2] - layout.bounds_px[0];
        let height = layout.bounds_px[3] - layout.bounds_px[1];
        assert!(width >= 359.0);
        assert!(height >= 239.0);
        assert!(width <= 362.0);
        assert!(height <= 242.0);
        assert!(layout.content_inset_px > 0.0);
    }

    #[test]
    fn generic_physical_layout_has_no_outline_without_semantic_outline() {
        use crate::retained::{
            ElementKind, RenderNodeDescriptor, RenderNodeKind, RetainedScene, SceneMode,
            UiVisualStyle,
        };

        let mut scene = RetainedScene::new(SceneMode::UiPhysical);
        let root = scene.root();
        scene.append_node(
            root,
            RenderNodeDescriptor::new(
                RenderNodeKind::Group,
                ElementKind::Group,
                Rect::new(100.0, 120.0, 320.0, 220.0),
            )
            .named("surface")
            .with_ui_visual_role(UiVisualRole::FilledSurface)
            .with_ui_visual_style(UiVisualStyle {
                base_color: [0.9, 0.92, 0.95, 1.0],
                accent_color: [0.0; 4],
                detail_color: [0.0; 4],
                stroke_width: 0.0,
                corner_radius: 20.0,
                offset: [0.0, 0.0],
                extra_size: [0.0, 0.0],
            })
            .with_material(6.0, 10.0, 18.0),
        );

        let text_data = FixedTextSceneData {
            char_instances: Vec::new(),
            char_count: 0,
            char_grid_params: [0.0; 4],
            char_grid_bounds: [0.0; 4],
            char_grid_cells: Vec::new(),
            char_grid_indices: Vec::new(),
        };
        let ui_data = GpuUiSceneData {
            primitives: Vec::new(),
            primitive_count: 0,
        };

        let layout = UiPhysicalLayout::generic_from_scene(&scene, &text_data, &ui_data);
        assert_eq!(layout.outline_width_px, 0.0);
        assert_eq!(layout.outline_color[3], 0.0);
        assert!(layout.content_inset_px > 0.0);
    }

    #[test]
    fn generic_physical_layout_uses_inner_ui_primitives_for_content_inset() {
        use crate::retained::{
            ElementKind, RenderNodeDescriptor, RenderNodeKind, RetainedScene, SceneMode,
            UiVisualStyle,
        };

        let mut scene = RetainedScene::new(SceneMode::UiPhysical);
        let root = scene.root();
        scene.append_node(
            root,
            RenderNodeDescriptor::new(
                RenderNodeKind::Group,
                ElementKind::Group,
                Rect::new(100.0, 120.0, 320.0, 220.0),
            )
            .named("surface")
            .with_ui_visual_role(UiVisualRole::FilledSurface)
            .with_ui_visual_style(UiVisualStyle {
                base_color: [0.9, 0.92, 0.95, 1.0],
                accent_color: [0.0; 4],
                detail_color: [0.0; 4],
                stroke_width: 0.0,
                corner_radius: 20.0,
                offset: [0.0, 0.0],
                extra_size: [0.0, 0.0],
            })
            .with_material(6.0, 10.0, 18.0),
        );

        let text_data = FixedTextSceneData {
            char_instances: Vec::new(),
            char_count: 0,
            char_grid_params: [0.0; 4],
            char_grid_bounds: [0.0; 4],
            char_grid_cells: Vec::new(),
            char_grid_indices: Vec::new(),
        };
        let ui_data = GpuUiSceneData {
            primitives: vec![GpuUiPrimitive {
                pos_size: [140.0, 165.0, 240.0, 30.0],
                color: [0.0; 4],
                params: [0.0; 4],
                extra: [0.0; 4],
            }],
            primitive_count: 1,
        };

        let layout = UiPhysicalLayout::generic_from_scene(&scene, &text_data, &ui_data);
        assert!(layout.content_inset_px >= 39.0);
        assert!(layout.content_inset_px <= 46.0);
    }

    #[test]
    fn physical_runtime_update_converts_partial_updates_without_full_rebuild() {
        let update = crate::demos::ui2d_runtime::Ui2dRuntimeUpdate {
            text: Some(crate::demos::ui2d_runtime::Ui2dRuntimeTextUpdate::Partial(
                FixedTextScenePatch {
                    run_updates: vec![(0, Vec::<GpuCharInstanceEx>::new())],
                    char_count: 3,
                    changed_cells: vec![1, 2],
                },
            )),
            ui: Some(crate::demos::ui2d_runtime::Ui2dRuntimeUiUpdate::Partial(
                vec![crate::retained::ui::GpuUiPatch {
                    offset: 4,
                    primitives: vec![GpuUiPrimitive {
                        pos_size: [0.0; 4],
                        color: [0.0; 4],
                        params: [0.0; 4],
                        extra: [0.0; 4],
                    }],
                }],
            )),
        };

        let runtime_update = UiPhysicalRuntimeUpdate::from(update);
        assert!(!runtime_update.needs_full_rebuild());

        match runtime_update.text {
            Some(UiPhysicalRuntimeTextUpdate::Partial(text_patch)) => {
                assert_eq!(text_patch.char_count, 3);
                assert_eq!(text_patch.changed_cells, vec![1, 2]);
            }
            _ => panic!("expected partial text update"),
        }

        match runtime_update.ui {
            Some(UiPhysicalRuntimeUiUpdate::Partial(ui_patches)) => {
                assert_eq!(ui_patches.len(), 1);
                assert_eq!(ui_patches[0].offset, 4);
            }
            _ => panic!("expected partial ui update"),
        }
    }

    #[test]
    fn fixed_physical_scene_state_builds_bootstrap_from_built_scene() {
        let atlas = load_test_atlas();
        let layout = scrolling_feed_text_run_layout();
        let built = crate::retained::fixed_scene::build_fixed_ui2d_scene(
            build_scrolling_feed_scene(),
            layout.layout(),
            &atlas,
            &colors(),
            text_space(),
            ui_space(),
        );

        let (_state, bootstrap) =
            FixedUiPhysicalSceneState::from_built_scene(built, atlas, text_space(), ui_space());

        assert!(bootstrap.text_data.char_count > 0);
        assert!(bootstrap.ui_data.primitive_count > 0);
    }

    #[test]
    fn fixed_physical_scene_state_produces_partial_update_for_scene_mutation() {
        let atlas = load_test_atlas();
        let layout = sample_text_run_layout();
        let (mut scene_state, _) = FixedUi2dSceneState::new(
            build_settings_panel_scene(),
            layout.layout(),
            &atlas,
            &colors(),
            text_space(),
            ui_space(),
        );
        scene_state.clear_dirty();

        let mut state = FixedUiPhysicalSceneState {
            scene_state,
            atlas,
            text_render_space: text_space(),
            ui_render_space: ui_space(),
        };

        assert!(toggle_settings_panel_state(state.scene_mut()));
        let Some(update) = state.take_ui_physical_resource_update(&colors()) else {
            panic!("expected physical retained update");
        };

        match update {
            UiPhysicalRuntimeUpdate {
                text: Some(UiPhysicalRuntimeTextUpdate::Partial(_)),
                ui: Some(UiPhysicalRuntimeUiUpdate::Partial(ui_patches)),
            } => assert!(!ui_patches.is_empty()),
            _ => panic!("expected partial physical update"),
        }
    }

    #[test]
    fn fixed_physical_scene_state_reuses_scroll_local_text_updates() {
        let atlas = load_test_atlas();
        let layout = scrolling_feed_text_run_layout();
        let (mut scene_state, _) = FixedUi2dSceneState::new(
            build_scrolling_feed_scene(),
            layout.layout(),
            &atlas,
            &colors(),
            text_space(),
            ui_space(),
        );
        scene_state.clear_dirty();

        let mut state = FixedUiPhysicalSceneState {
            scene_state,
            atlas,
            text_render_space: text_space(),
            ui_render_space: ui_space(),
        };

        assert!(crate::retained::samples::adjust_scrolling_feed_offset(
            state.scene_mut(),
            24.0,
        ));
        let expected_slots = state.scene().classify_resource_dirty(state.scene().dirty());
        let Some(update) = state.take_ui_physical_resource_update(&colors()) else {
            panic!("expected physical retained update");
        };

        match update {
            UiPhysicalRuntimeUpdate {
                text: Some(UiPhysicalRuntimeTextUpdate::Partial(text_patch)),
                ui: None,
            } => {
                assert_eq!(
                    text_patch.run_updates.len(),
                    expected_slots.text_slots.len()
                );
                assert!(text_patch.run_updates.len() < layout.run_capacities().len());
            }
            _ => panic!("expected scroll-local physical text update"),
        }
    }

    #[derive(Default)]
    struct MockPhysicalDeckHost {
        ready_calls: usize,
        prepare_calls: usize,
        resize_calls: Vec<(u32, u32)>,
        resize_lazy_calls: Vec<(u32, u32)>,
        colors: Vec<TextColors>,
        needs_redraw: bool,
    }

    impl UiPhysicalDeckHost for MockPhysicalDeckHost {
        fn needs_redraw(&self) -> bool {
            self.needs_redraw
        }

        fn prepare_frame(&mut self, _queue: &wgpu::Queue) {
            self.prepare_calls += 1;
        }

        fn resize(&mut self, width: u32, height: u32) {
            self.resize_calls.push((width, height));
        }

        fn resize_lazy(&mut self, width: u32, height: u32) {
            self.resize_lazy_calls.push((width, height));
        }

        fn ensure_ready(&mut self) {
            self.ready_calls += 1;
        }

        fn set_text_colors(&mut self, text_colors: TextColors) {
            self.colors.push(text_colors);
        }

        fn storage_buffers(&self) -> &UiStorageBuffers {
            panic!("mock storage buffers are not used in these deck tests");
        }

        fn char_instances(&self) -> &[GpuCharInstanceEx] {
            panic!("mock char instances are not used in these deck tests");
        }
    }

    #[test]
    fn physical_scene_deck_cycles_and_resizes_active_scene_eagerly() {
        let mut deck = UiPhysicalSceneDeck::new(vec![
            MockPhysicalDeckHost::default(),
            MockPhysicalDeckHost::default(),
        ]);

        assert_eq!(deck.active_index(), 0);
        assert!(deck.cycle_next());
        assert_eq!(deck.active_index(), 1);
        assert_eq!(deck.active_host().ready_calls, 1);

        deck.resize(800, 600);
        assert_eq!(deck.scenes[0].resize_lazy_calls, vec![(800, 600)]);
        assert_eq!(deck.scenes[1].resize_calls, vec![(800, 600)]);
    }

    #[test]
    fn physical_scene_deck_sets_text_colors_for_all_scenes() {
        let mut deck = UiPhysicalSceneDeck::new(vec![
            MockPhysicalDeckHost::default(),
            MockPhysicalDeckHost::default(),
        ]);
        let colors = TextColors {
            heading: [1.0, 0.0, 0.0],
            active: [0.0, 1.0, 0.0],
            completed: [0.0, 0.0, 1.0],
            placeholder: [1.0, 1.0, 0.0],
            body: [1.0, 0.0, 1.0],
            info: [0.0, 1.0, 1.0],
        };

        deck.set_text_colors(colors);

        assert_eq!(deck.scenes[0].colors.len(), 1);
        assert_eq!(deck.scenes[1].colors.len(), 1);
        assert_eq!(deck.scenes[0].colors[0].heading, [1.0, 0.0, 0.0]);
        assert_eq!(deck.scenes[1].colors[0].heading, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn physical_scene_deck_reports_pending_redraw_from_active_host() {
        let mut deck = UiPhysicalSceneDeck::new(vec![
            MockPhysicalDeckHost::default(),
            MockPhysicalDeckHost {
                needs_redraw: true,
                ..Default::default()
            },
        ]);

        assert!(!deck.needs_redraw());
        assert!(deck.cycle_next());
        assert!(deck.needs_redraw());
    }
}
