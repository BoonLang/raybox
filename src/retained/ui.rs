use super::{
    ElementKind, RenderNode, RetainedScene, TextRole, ToggleState, UiVisualRole, UiVisualStyle,
};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UiShape {
    FilledRect,
    StrokedRect {
        corner_radius: f32,
        stroke_width: f32,
    },
    StrokedCircle {
        radius: f32,
        stroke_width: f32,
    },
    Line {
        stroke_width: f32,
    },
    BoxShadow {
        blur_radius: f32,
        offset: [f32; 2],
    },
    Checkmark {
        stroke_width: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiPrimitive {
    pub pos_size: [f32; 4],
    pub color: [f32; 4],
    pub shape: UiShape,
    pub extra: [f32; 4],
}

pub type GpuUiPrimitive = crate::ui2d_shader_bindings::UiPrimitive_std430_0;

pub struct UiSceneData {
    pub primitives: Vec<UiPrimitive>,
    pub primitive_count: usize,
}

pub struct UiPatch {
    pub offset: usize,
    pub primitives: Vec<UiPrimitive>,
}

pub struct GpuUiSceneData {
    pub primitives: Vec<GpuUiPrimitive>,
    pub primitive_count: usize,
}

pub struct GpuUiPatch {
    pub offset: usize,
    pub primitives: Vec<GpuUiPrimitive>,
}

pub enum GpuUiRuntimeUpdate {
    Full(GpuUiSceneData),
    Partial(Vec<GpuUiPatch>),
}

#[derive(Debug, Clone, Copy)]
pub struct UiRenderSpace {
    pub x_offset: f32,
    pub screen_height: f32,
}

impl UiRenderSpace {
    fn fy(self, y: f32) -> f32 {
        self.screen_height - y
    }

    fn rect_yu(self, x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
        [x, self.fy(y + h), w, h]
    }

    fn hline_yu(self, x1: f32, y: f32, x2: f32) -> [f32; 4] {
        [x1, self.fy(y), x2, self.fy(y)]
    }
}

fn default_checkbox_style() -> UiVisualStyle {
    UiVisualStyle {
        base_color: [
            0x94 as f32 / 255.0,
            0x94 as f32 / 255.0,
            0x94 as f32 / 255.0,
            1.0,
        ],
        accent_color: [
            0x59 as f32 / 255.0,
            0xA1 as f32 / 255.0,
            0x93 as f32 / 255.0,
            1.0,
        ],
        detail_color: [
            0x3E as f32 / 255.0,
            0xA3 as f32 / 255.0,
            0x90 as f32 / 255.0,
            1.0,
        ],
        stroke_width: 1.2,
        corner_radius: 0.0,
        offset: [0.0, 0.0],
        extra_size: [0.0, 0.0],
    }
}

fn default_completed_text_style() -> UiVisualStyle {
    UiVisualStyle {
        base_color: [
            0x94 as f32 / 255.0,
            0x94 as f32 / 255.0,
            0x94 as f32 / 255.0,
            1.0,
        ],
        accent_color: [0.0; 4],
        detail_color: [0.0; 4],
        stroke_width: 2.0,
        corner_radius: 0.0,
        offset: [0.0, 0.0],
        extra_size: [0.0, 0.0],
    }
}

fn default_selection_outline_style() -> UiVisualStyle {
    UiVisualStyle {
        base_color: [
            0xCE as f32 / 255.0,
            0x46 as f32 / 255.0,
            0x46 as f32 / 255.0,
            1.0,
        ],
        accent_color: [0.0; 4],
        detail_color: [0.0; 4],
        stroke_width: 0.8,
        corner_radius: 3.0,
        offset: [-7.8, -3.8],
        extra_size: [8.3, 9.4],
    }
}

pub const PRIM_FILLED_RECT: f32 = 0.0;
pub const PRIM_STROKED_RECT: f32 = 1.0;
pub const PRIM_FILLED_CIRCLE: f32 = 2.0;
pub const PRIM_STROKED_CIRCLE: f32 = 3.0;
pub const PRIM_LINE: f32 = 4.0;
pub const PRIM_BOX_SHADOW: f32 = 5.0;
pub const PRIM_CHECKMARK_V: f32 = 6.0;

pub fn noop_primitive() -> UiPrimitive {
    UiPrimitive {
        pos_size: [0.0; 4],
        color: [0.0; 4],
        shape: UiShape::FilledRect,
        extra: [0.0; 4],
    }
}

pub fn pack_gpu_ui_primitive(primitive: UiPrimitive) -> GpuUiPrimitive {
    let params = match primitive.shape {
        UiShape::FilledRect => [0.0, 0.0, 0.0, PRIM_FILLED_RECT],
        UiShape::StrokedRect {
            corner_radius,
            stroke_width,
        } => [corner_radius, stroke_width, 0.0, PRIM_STROKED_RECT],
        UiShape::StrokedCircle {
            radius: _,
            stroke_width,
        } => [0.0, stroke_width, 0.0, PRIM_STROKED_CIRCLE],
        UiShape::Line { stroke_width } => [0.0, stroke_width, 0.0, PRIM_LINE],
        UiShape::BoxShadow {
            blur_radius,
            offset: _,
        } => [0.0, 0.0, blur_radius, PRIM_BOX_SHADOW],
        UiShape::Checkmark { stroke_width } => [0.0, stroke_width, 0.0, PRIM_CHECKMARK_V],
    };
    let extra = match primitive.shape {
        UiShape::BoxShadow { offset, .. } => [offset[0], offset[1], 0.0, 0.0],
        _ => primitive.extra,
    };

    GpuUiPrimitive::new(primitive.pos_size, primitive.color, params, extra)
}

fn build_static_ui_primitive(
    scene: &RetainedScene,
    node: &RenderNode,
    space: UiRenderSpace,
) -> Option<UiPrimitive> {
    let bounds = scene.resolved_bounds(node.id)?;
    let style = node.ui_visual_style?;

    match node.ui_visual_role? {
        UiVisualRole::BoxShadow => Some(UiPrimitive {
            pos_size: space.rect_yu(
                bounds.x + space.x_offset,
                bounds.y,
                bounds.width,
                bounds.height,
            ),
            color: style.base_color,
            shape: UiShape::BoxShadow {
                blur_radius: style.corner_radius,
                offset: style.offset,
            },
            extra: [0.0; 4],
        }),
        UiVisualRole::FilledSurface => Some(UiPrimitive {
            pos_size: space.rect_yu(
                bounds.x + space.x_offset,
                bounds.y,
                bounds.width,
                bounds.height,
            ),
            color: style.base_color,
            shape: UiShape::FilledRect,
            extra: [0.0; 4],
        }),
        UiVisualRole::SeparatorLine => Some(UiPrimitive {
            pos_size: space.hline_yu(
                bounds.x + space.x_offset,
                bounds.y,
                bounds.x + space.x_offset + bounds.width,
            ),
            color: style.base_color,
            shape: UiShape::Line {
                stroke_width: style.stroke_width,
            },
            extra: [0.0; 4],
        }),
        UiVisualRole::ChevronMark => {
            let cx = bounds.x + space.x_offset + bounds.width * 0.5;
            let cy = bounds.y + bounds.height * 0.5;
            let half_w = bounds.width * 0.5;
            let half_h = bounds.height * 0.5;
            let left = (cx - half_w, cy - half_h);
            let bottom = (cx, cy + half_h);
            let right = (cx + half_w, cy - half_h);
            Some(UiPrimitive {
                pos_size: [left.0, space.fy(left.1), bottom.0, space.fy(bottom.1)],
                color: style.base_color,
                shape: UiShape::Checkmark {
                    stroke_width: style.stroke_width,
                },
                extra: [right.0, space.fy(right.1), 0.0, 0.0],
            })
        }
        UiVisualRole::OutlineRect => Some(UiPrimitive {
            pos_size: space.rect_yu(
                bounds.x + space.x_offset + style.offset[0],
                bounds.y + style.offset[1],
                bounds.width + style.extra_size[0],
                bounds.height + style.extra_size[1],
            ),
            color: style.base_color,
            shape: UiShape::StrokedRect {
                corner_radius: style.corner_radius,
                stroke_width: style.stroke_width,
            },
            extra: [0.0; 4],
        }),
        _ => None,
    }
}

fn build_checkbox_ui_primitives(
    scene: &RetainedScene,
    checkbox: &RenderNode,
    space: UiRenderSpace,
) -> [UiPrimitive; 2] {
    let style = checkbox
        .ui_visual_style
        .unwrap_or_else(default_checkbox_style);
    let checkbox_bounds = scene
        .resolved_bounds(checkbox.id)
        .expect("resolved checkbox bounds");
    let cx = checkbox_bounds.x + checkbox_bounds.width * 0.5;
    let cy = space.fy(checkbox_bounds.y + checkbox_bounds.height * 0.5);
    let r = 17.0;
    let checked = checkbox.toggle_state == Some(ToggleState::On);

    let checkbox_circle = if checked {
        UiPrimitive {
            pos_size: [cx, cy, r, 0.0],
            color: style.accent_color,
            shape: UiShape::StrokedCircle {
                radius: r,
                stroke_width: style.stroke_width,
            },
            extra: [0.0; 4],
        }
    } else {
        UiPrimitive {
            pos_size: [cx, cy, r, 0.0],
            color: style.base_color,
            shape: UiShape::StrokedCircle {
                radius: r,
                stroke_width: style.stroke_width,
            },
            extra: [0.0; 4],
        }
    };

    let checkmark = if checked {
        let s = r / 50.0;
        let map = |sx: f32, sy: f32| -> (f32, f32) { (cx + (sx - 50.0) * s, cy - (sy - 50.0) * s) };
        let (ax, ay) = map(27.0, 56.0);
        let (bx, by) = map(42.0, 71.0);
        let (cx2, cy2) = map(72.0, 25.0);
        UiPrimitive {
            pos_size: [ax, ay, bx, by],
            color: style.detail_color,
            shape: UiShape::Checkmark { stroke_width: 2.0 },
            extra: [cx2, cy2, 0.0, 0.0],
        }
    } else {
        noop_primitive()
    };

    [checkbox_circle, checkmark]
}

fn build_completed_text_decoration(
    scene: &RetainedScene,
    label: &RenderNode,
    space: UiRenderSpace,
) -> UiPrimitive {
    let style = label
        .ui_visual_style
        .unwrap_or_else(default_completed_text_style);
    let bounds = scene
        .resolved_bounds(label.id)
        .expect("resolved label bounds");
    if label.text_role != Some(TextRole::Completed)
        || !scene.is_rect_visible_for_node(label.id, bounds)
    {
        return noop_primitive();
    }

    let strike_y = space.fy(bounds.y + bounds.height * 0.5);
    UiPrimitive {
        pos_size: [
            bounds.x + space.x_offset,
            strike_y,
            bounds.x + bounds.width + space.x_offset,
            strike_y,
        ],
        color: style.base_color,
        shape: UiShape::Line {
            stroke_width: style.stroke_width,
        },
        extra: [0.0; 4],
    }
}

fn build_selected_button_ui_primitive(
    scene: &RetainedScene,
    button: &RenderNode,
    space: UiRenderSpace,
) -> UiPrimitive {
    let style = button
        .ui_visual_style
        .unwrap_or_else(default_selection_outline_style);
    if button.selection_state != Some(super::SelectionState::Selected) {
        return noop_primitive();
    }

    let bounds = scene
        .resolved_bounds(button.id)
        .expect("resolved button bounds");
    UiPrimitive {
        pos_size: space.rect_yu(
            bounds.x + space.x_offset + style.offset[0],
            bounds.y + style.offset[1],
            bounds.width + style.extra_size[0],
            bounds.height + style.extra_size[1],
        ),
        color: style.base_color,
        shape: UiShape::StrokedRect {
            corner_radius: style.corner_radius,
            stroke_width: style.stroke_width,
        },
        extra: [0.0; 4],
    }
}

fn build_composite_ui_primitives(
    scene: &RetainedScene,
    node: &RenderNode,
    space: UiRenderSpace,
) -> Option<Vec<UiPrimitive>> {
    let bounds = scene.resolved_bounds(node.id)?;
    if !scene.is_rect_visible_for_node(node.id, bounds) {
        return Some(vec![noop_primitive(); node.ui_primitive_count as usize]);
    }

    let checkbox = scene.first_child_with_ui_visual_role(node.id, UiVisualRole::CheckboxControl);
    let completed_text =
        scene.first_child_with_ui_visual_role(node.id, UiVisualRole::CompletedTextDecoration);
    let selection_outline =
        scene.first_child_with_ui_visual_role(node.id, UiVisualRole::SelectionOutline);

    if checkbox.is_none() && completed_text.is_none() && selection_outline.is_none() {
        return None;
    }

    let mut primitives = Vec::with_capacity(node.ui_primitive_count as usize);

    if let Some(checkbox) = checkbox {
        primitives.extend(build_checkbox_ui_primitives(scene, checkbox, space));
    }

    if let Some(label) = completed_text {
        primitives.push(build_completed_text_decoration(scene, label, space));
    }

    if let Some(button) = selection_outline {
        primitives.push(build_selected_button_ui_primitive(scene, button, space));
    }

    primitives.resize(node.ui_primitive_count as usize, noop_primitive());
    Some(primitives)
}

pub fn build_ui_primitives_for_node(
    scene: &RetainedScene,
    node: &RenderNode,
    space: UiRenderSpace,
) -> Option<Vec<UiPrimitive>> {
    match (node.element, node.ui_visual_role) {
        (ElementKind::Checkbox, Some(UiVisualRole::CheckboxControl)) => {
            Some(build_checkbox_ui_primitives(scene, node, space).into())
        }
        (ElementKind::Text, Some(UiVisualRole::CompletedTextDecoration)) => {
            Some(vec![build_completed_text_decoration(scene, node, space)])
        }
        (ElementKind::Button, Some(UiVisualRole::SelectionOutline)) => {
            Some(vec![build_selected_button_ui_primitive(scene, node, space)])
        }
        _ => build_composite_ui_primitives(scene, node, space),
    }
}

pub fn build_static_ui_primitives(scene: &RetainedScene, space: UiRenderSpace) -> Vec<UiPrimitive> {
    let mut prims = Vec::new();
    for role in [
        UiVisualRole::BoxShadow,
        UiVisualRole::FilledSurface,
        UiVisualRole::SeparatorLine,
        UiVisualRole::ChevronMark,
        UiVisualRole::OutlineRect,
    ] {
        prims.extend(
            scene
                .descendants_with_ui_visual_role(scene.root(), role)
                .into_iter()
                .filter_map(|node| build_static_ui_primitive(scene, node, space)),
        );
    }
    prims
}

pub fn build_ui_patch_for_slot(
    scene: &RetainedScene,
    ui_slot: u16,
    space: UiRenderSpace,
) -> Option<UiPatch> {
    let static_count = build_static_ui_primitives(scene, space).len();
    let range = scene.ui_slot_range(ui_slot, static_count)?;
    let node = scene.node(range.node_id)?;
    let primitives = build_ui_primitives_for_node(scene, node, space)
        .unwrap_or_else(|| vec![noop_primitive(); range.primitive_count]);
    Some(UiPatch {
        offset: range.offset,
        primitives,
    })
}

pub fn build_ui_patches_for_slots(
    scene: &RetainedScene,
    ui_slots: &BTreeSet<u16>,
    space: UiRenderSpace,
) -> Vec<UiPatch> {
    ui_slots
        .iter()
        .filter_map(|ui_slot| build_ui_patch_for_slot(scene, *ui_slot, space))
        .collect()
}

pub fn build_ui_scene(scene: &RetainedScene, space: UiRenderSpace) -> UiSceneData {
    let static_primitives = build_static_ui_primitives(scene, space);
    let static_count = static_primitives.len();
    let mut primitives = static_primitives;
    primitives.reserve(scene.total_ui_primitive_count(static_count) - static_count);

    for range in scene.ui_slot_ranges(static_count) {
        let Some(node) = scene.node(range.node_id) else {
            primitives.extend(vec![noop_primitive(); range.primitive_count]);
            continue;
        };
        primitives.extend(
            build_ui_primitives_for_node(scene, node, space)
                .unwrap_or_else(|| vec![noop_primitive(); node.ui_primitive_count as usize]),
        );
    }

    UiSceneData {
        primitive_count: primitives.len(),
        primitives,
    }
}

pub fn build_gpu_ui_patch_for_slot(
    scene: &RetainedScene,
    ui_slot: u16,
    space: UiRenderSpace,
) -> Option<GpuUiPatch> {
    let patch = build_ui_patch_for_slot(scene, ui_slot, space)?;
    Some(GpuUiPatch {
        offset: patch.offset,
        primitives: patch
            .primitives
            .into_iter()
            .map(pack_gpu_ui_primitive)
            .collect(),
    })
}

pub fn build_gpu_ui_patches_for_slots(
    scene: &RetainedScene,
    ui_slots: &BTreeSet<u16>,
    space: UiRenderSpace,
) -> Vec<GpuUiPatch> {
    build_ui_patches_for_slots(scene, ui_slots, space)
        .into_iter()
        .map(|patch| GpuUiPatch {
            offset: patch.offset,
            primitives: patch
                .primitives
                .into_iter()
                .map(pack_gpu_ui_primitive)
                .collect(),
        })
        .collect()
}

pub fn build_gpu_ui_scene(scene: &RetainedScene, space: UiRenderSpace) -> GpuUiSceneData {
    let scene_data = build_ui_scene(scene, space);
    GpuUiSceneData {
        primitive_count: scene_data.primitive_count,
        primitives: scene_data
            .primitives
            .into_iter()
            .map(pack_gpu_ui_primitive)
            .collect(),
    }
}

pub fn apply_gpu_ui_runtime_update(
    update: GpuUiRuntimeUpdate,
    ui_prim_count: &mut u32,
    max_ui_primitives: usize,
    mut write_full: impl FnMut(&[GpuUiPrimitive]),
    mut write_patch: impl FnMut(usize, &[GpuUiPrimitive]),
) {
    match update {
        GpuUiRuntimeUpdate::Full(ui_data) => {
            assert!(ui_data.primitive_count <= max_ui_primitives);
            write_full(&ui_data.primitives);
            *ui_prim_count = ui_data.primitive_count as u32;
        }
        GpuUiRuntimeUpdate::Partial(patches) => {
            for patch in patches {
                write_patch(patch.offset, &patch.primitives);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_gpu_ui_patch_for_slot, build_gpu_ui_scene, UiRenderSpace, PRIM_CHECKMARK_V,
        PRIM_LINE, PRIM_STROKED_CIRCLE, PRIM_STROKED_RECT,
    };
    use crate::retained::{
        ElementKind, Rect, RenderNodeDescriptor, RenderNodeKind, RetainedScene, SceneMode,
        SelectionState, UiVisualRole, UiVisualStyle,
    };

    fn rgba(r: u8, g: u8, b: u8, a: f32) -> [f32; 4] {
        [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a]
    }

    #[test]
    fn gpu_ui_scene_and_patch_share_same_slot_encoding() {
        let mut scene = RetainedScene::new(SceneMode::Ui2D);
        let root = scene.root();
        assert!(scene.set_bounds(root, Rect::new(0.0, 0.0, 400.0, 300.0)));

        scene
            .append_node(
                root,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Button,
                    Rect::new(20.0, 20.0, 120.0, 48.0),
                )
                .with_ui_slot(7)
                .with_ui_primitive_count(1)
                .with_selection_state(SelectionState::Selected)
                .with_ui_visual_role(UiVisualRole::SelectionOutline)
                .with_ui_visual_style(UiVisualStyle {
                    base_color: rgba(0xCE, 0x46, 0x46, 1.0),
                    accent_color: [0.0; 4],
                    detail_color: [0.0; 4],
                    stroke_width: 0.8,
                    corner_radius: 3.0,
                    offset: [-7.8, -3.8],
                    extra_size: [8.3, 9.4],
                }),
            )
            .expect("insert ui node");

        let space = UiRenderSpace {
            x_offset: 0.0,
            screen_height: 300.0,
        };

        let full = build_gpu_ui_scene(&scene, space);
        let patch = build_gpu_ui_patch_for_slot(&scene, 7, space).expect("ui patch");

        assert_eq!(full.primitive_count, 1);
        assert_eq!(patch.offset, 0);
        assert_eq!(patch.primitives, full.primitives);
        assert_eq!(patch.primitives[0].params_0[3], PRIM_STROKED_RECT);
    }

    #[test]
    fn composite_slot_uses_descendant_visual_roles_without_todo_item_kind() {
        let mut scene = RetainedScene::new(SceneMode::Ui2D);
        let root = scene.root();
        assert!(scene.set_bounds(root, Rect::new(0.0, 0.0, 400.0, 300.0)));

        let item = scene
            .append_node(
                root,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Group,
                    ElementKind::Group,
                    Rect::new(20.0, 40.0, 220.0, 40.0),
                )
                .named("composite")
                .with_ui_slot(9)
                .with_ui_primitive_count(3),
            )
            .expect("composite item");

        scene
            .append_node(
                item,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Checkbox,
                    Rect::new(28.0, 44.0, 32.0, 32.0),
                )
                .with_ui_visual_role(UiVisualRole::CheckboxControl)
                .with_toggle_state(crate::retained::ToggleState::On),
            )
            .expect("checkbox");

        scene
            .append_node(
                item,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Text,
                    Rect::new(80.0, 48.0, 120.0, 20.0),
                )
                .with_text("Done", 18.0)
                .with_text_role(crate::retained::TextRole::Completed)
                .with_ui_visual_role(UiVisualRole::CompletedTextDecoration),
            )
            .expect("completed text");

        let gpu_scene = build_gpu_ui_scene(
            &scene,
            UiRenderSpace {
                x_offset: 0.0,
                screen_height: 300.0,
            },
        );
        assert_eq!(gpu_scene.primitive_count, 3);
        assert_eq!(gpu_scene.primitives[0].params_0[3], PRIM_STROKED_CIRCLE);
        assert_eq!(gpu_scene.primitives[1].params_0[3], PRIM_CHECKMARK_V);
        assert_eq!(gpu_scene.primitives[2].params_0[3], PRIM_LINE);

        let patch = build_gpu_ui_patch_for_slot(
            &scene,
            9,
            UiRenderSpace {
                x_offset: 0.0,
                screen_height: 300.0,
            },
        )
        .expect("composite slot patch");
        assert_eq!(patch.primitives.len(), 3);
        assert_eq!(patch.primitives[0].params_0[3], PRIM_STROKED_CIRCLE);
        assert_eq!(patch.primitives[1].params_0[3], PRIM_CHECKMARK_V);
        assert_eq!(patch.primitives[2].params_0[3], PRIM_LINE);
    }
}
