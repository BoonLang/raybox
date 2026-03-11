use crate::demo_core::ListFilter;
use crate::retained::{
    ElementKind, NodeId, Rect, RenderNode, RenderNodeDescriptor, RenderNodeKind, RetainedScene,
    SceneMode, ScrollState, SelectionState, TextNode, TextRole, ToggleState, UiVisualRole,
    UiVisualStyle,
};
use crate::todomvc_shared::{
    todo_item_ui_slot_for_index, TODO_TEXT_RUN_CLEAR_COMPLETED, TODO_TEXT_RUN_FILTER_ACTIVE,
    TODO_TEXT_RUN_FILTER_ALL, TODO_TEXT_RUN_FILTER_COMPLETED, TODO_TEXT_RUN_HEADING,
    TODO_TEXT_RUN_INFO_AUTHOR, TODO_TEXT_RUN_INFO_BRAND, TODO_TEXT_RUN_INFO_EDIT,
    TODO_TEXT_RUN_ITEMS_LEFT_COUNT, TODO_TEXT_RUN_ITEMS_LEFT_SUFFIX, TODO_TEXT_RUN_ITEM_BUY_LABEL,
    TODO_TEXT_RUN_ITEM_FINISH_LABEL, TODO_TEXT_RUN_ITEM_READ_LABEL, TODO_TEXT_RUN_ITEM_WALK_LABEL,
    TODO_TEXT_RUN_PLACEHOLDER, TODO_UI_PRIMS_PER_ITEM, TODO_UI_SLOT_FILTER_ACTIVE,
    TODO_UI_SLOT_FILTER_ALL, TODO_UI_SLOT_FILTER_COMPLETED, TODO_UI_SLOT_ITEM_BUY,
    TODO_UI_SLOT_ITEM_FINISH, TODO_UI_SLOT_ITEM_READ, TODO_UI_SLOT_ITEM_WALK,
};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

const CARD_X: f32 = 75.0;
const CARD_Y: f32 = 130.0;
const CARD_W: f32 = 550.0;
const CARD_H: f32 = 344.2;
const INPUT_H: f32 = 65.8;
const LIST_TOP: f32 = 195.8;
const LIST_BOTTOM: f32 = 433.4;
const FOOTER_H: f32 = CARD_H - (LIST_BOTTOM - CARD_Y);
const ITEM_H: f32 = 59.6;
const CHECKBOX_X: f32 = CARD_X + 9.0;
const CHECKBOX_Y_OFFSET: f32 = 12.8;
const LABEL_X: f32 = 135.0;
const LABEL_Y_OFFSET: f32 = 15.8;
const HIDDEN_ITEM_Y_OFFSET: f32 = 1000.0;

fn rgb(r: u8, g: u8, b: u8) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

fn rgba(r: u8, g: u8, b: u8, a: f32) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a]
}

fn todo_checkbox_style() -> UiVisualStyle {
    UiVisualStyle {
        base_color: rgb(0x94, 0x94, 0x94),
        accent_color: rgb(0x59, 0xA1, 0x93),
        detail_color: rgb(0x3E, 0xA3, 0x90),
        stroke_width: 1.2,
        corner_radius: 0.0,
        offset: [0.0, 0.0],
        extra_size: [0.0, 0.0],
    }
}

fn todo_completed_text_style() -> UiVisualStyle {
    UiVisualStyle {
        base_color: rgb(0x94, 0x94, 0x94),
        accent_color: [0.0; 4],
        detail_color: [0.0; 4],
        stroke_width: 2.0,
        corner_radius: 0.0,
        offset: [0.0, 0.0],
        extra_size: [0.0, 0.0],
    }
}

fn todo_filter_outline_style() -> UiVisualStyle {
    UiVisualStyle {
        base_color: rgba(0xCE, 0x46, 0x46, 1.0),
        accent_color: [0.0; 4],
        detail_color: [0.0; 4],
        stroke_width: 0.8,
        corner_radius: 3.0,
        offset: [-7.8, -3.8],
        extra_size: [8.3, 9.4],
    }
}

fn todo_box_shadow_style(alpha: f32, blur: f32, offset_y: f32) -> UiVisualStyle {
    UiVisualStyle {
        base_color: rgba(0, 0, 0, alpha),
        accent_color: [0.0; 4],
        detail_color: [0.0; 4],
        stroke_width: 0.0,
        corner_radius: blur,
        offset: [0.0, offset_y],
        extra_size: [0.0, 0.0],
    }
}

fn todo_fill_style(r: u8, g: u8, b: u8, a: f32) -> UiVisualStyle {
    UiVisualStyle {
        base_color: rgba(r, g, b, a),
        accent_color: [0.0; 4],
        detail_color: [0.0; 4],
        stroke_width: 0.0,
        corner_radius: 0.0,
        offset: [0.0, 0.0],
        extra_size: [0.0, 0.0],
    }
}

fn todo_separator_style(r: u8, g: u8, b: u8) -> UiVisualStyle {
    UiVisualStyle {
        base_color: rgb(r, g, b),
        accent_color: [0.0; 4],
        detail_color: [0.0; 4],
        stroke_width: 0.8,
        corner_radius: 0.0,
        offset: [0.0, 0.0],
        extra_size: [0.0, 0.0],
    }
}

fn todo_chevron_style() -> UiVisualStyle {
    UiVisualStyle {
        base_color: rgb(0x94, 0x94, 0x94),
        accent_color: [0.0; 4],
        detail_color: [0.0; 4],
        stroke_width: 4.0,
        corner_radius: 0.0,
        offset: [0.0, 0.0],
        extra_size: [0.0, 0.0],
    }
}

fn todo_input_outline_style() -> UiVisualStyle {
    UiVisualStyle {
        base_color: rgba(0xCE, 0x46, 0x46, 0.6),
        accent_color: [0.0; 4],
        detail_color: [0.0; 4],
        stroke_width: 0.8,
        corner_radius: 0.0,
        offset: [0.0, 0.0],
        extra_size: [0.0, 0.0],
    }
}

pub struct TodoMvcRetainedScene {
    scene: RetainedScene,
    viewport: NodeId,
    list_scroll: NodeId,
    items_left_count: NodeId,
    filter_all: NodeId,
    filter_active: NodeId,
    filter_completed: NodeId,
    items: Vec<TodoItemNodeIds>,
    item_node_indices: BTreeMap<NodeId, usize>,
    filter: ListFilter,
    pending_changes: PendingResourceChanges,
}

#[derive(Debug, Clone)]
struct TodoItemNodeIds {
    item: NodeId,
    checkbox: NodeId,
    label: NodeId,
    label_text: Cow<'static, str>,
    completed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct PendingResourceChanges {
    ui_slots: BTreeSet<u16>,
    text_style_nodes: BTreeSet<NodeId>,
    text_layout_nodes: BTreeSet<NodeId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TodoMvcResourceDirty {
    pub ui: bool,
    pub text: bool,
    pub full_ui: bool,
    pub full_text: bool,
    pub ui_slots: BTreeSet<u16>,
    pub text_style_nodes: BTreeSet<NodeId>,
    pub text_layout_nodes: BTreeSet<NodeId>,
}

impl TodoMvcResourceDirty {
    pub fn none() -> Self {
        Self {
            ui: false,
            text: false,
            full_ui: false,
            full_text: false,
            ui_slots: BTreeSet::new(),
            text_style_nodes: BTreeSet::new(),
            text_layout_nodes: BTreeSet::new(),
        }
    }

    pub fn all() -> Self {
        Self {
            ui: true,
            text: true,
            full_ui: true,
            full_text: true,
            ui_slots: BTreeSet::new(),
            text_style_nodes: BTreeSet::new(),
            text_layout_nodes: BTreeSet::new(),
        }
    }

    pub const fn any(&self) -> bool {
        self.ui || self.text
    }
}

impl TodoMvcRetainedScene {
    pub fn new(mode: SceneMode, width: u32, height: u32) -> Self {
        let mut scene = RetainedScene::new(mode);
        let root = scene.root();

        let viewport = scene
            .append_node(
                root,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Viewport,
                    ElementKind::Viewport,
                    Rect::new(0.0, 0.0, width as f32, height as f32),
                )
                .named("viewport")
                .with_key("viewport"),
            )
            .expect("root viewport");

        let heading = scene
            .append_node(
                viewport,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Heading,
                    Rect::new(252.1, 8.4, 195.7, 89.6),
                )
                .named("heading")
                .with_key("heading")
                .with_text_slot(TODO_TEXT_RUN_HEADING)
                .with_text("todos", 80.0)
                .with_text_role(TextRole::Heading),
            )
            .expect("heading");

        let card_shadow = scene
            .append_node(
                viewport,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Shadow,
                    Rect::new(CARD_X, CARD_Y, CARD_W, CARD_H),
                )
                .named("card_shadow")
                .with_key("card_shadow")
                .with_ui_visual_role(UiVisualRole::BoxShadow)
                .with_ui_visual_style(todo_box_shadow_style(0.1, 50.0, -25.0))
                .with_material(0.0, 0.0, 0.0),
            )
            .expect("card shadow");

        let _stack_back_shadow = scene
            .append_node(
                viewport,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Shadow,
                    Rect::new(CARD_X + 4.0, CARD_Y + CARD_H, CARD_W - 8.0, 8.0),
                )
                .named("stack_back_shadow")
                .with_key("stack_back_shadow")
                .with_ui_visual_role(UiVisualRole::BoxShadow)
                .with_ui_visual_style(todo_box_shadow_style(0.2, 1.0, -1.0)),
            )
            .expect("stack back shadow");

        let _stack_back_fill = scene
            .append_node(
                viewport,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Panel,
                    Rect::new(CARD_X + 4.0, CARD_Y + CARD_H, CARD_W - 8.0, 8.0),
                )
                .named("stack_back_fill")
                .with_key("stack_back_fill")
                .with_ui_visual_role(UiVisualRole::FilledSurface)
                .with_ui_visual_style(todo_fill_style(252, 252, 252, 1.0)),
            )
            .expect("stack back fill");

        let _stack_mid_shadow = scene
            .append_node(
                viewport,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Shadow,
                    Rect::new(CARD_X + 2.0, CARD_Y + CARD_H, CARD_W - 4.0, 4.0),
                )
                .named("stack_mid_shadow")
                .with_key("stack_mid_shadow")
                .with_ui_visual_role(UiVisualRole::BoxShadow)
                .with_ui_visual_style(todo_box_shadow_style(0.2, 1.0, -1.0)),
            )
            .expect("stack mid shadow");

        let _stack_mid_fill = scene
            .append_node(
                viewport,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Panel,
                    Rect::new(CARD_X + 2.0, CARD_Y + CARD_H, CARD_W - 4.0, 4.0),
                )
                .named("stack_mid_fill")
                .with_key("stack_mid_fill")
                .with_ui_visual_role(UiVisualRole::FilledSurface)
                .with_ui_visual_style(todo_fill_style(252, 252, 252, 1.0)),
            )
            .expect("stack mid fill");

        let card = scene
            .append_node(
                viewport,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Panel,
                    Rect::new(CARD_X, CARD_Y, CARD_W, CARD_H),
                )
                .named("card")
                .with_key("card")
                .with_ui_visual_role(UiVisualRole::FilledSurface)
                .with_ui_visual_style(todo_fill_style(255, 255, 255, 1.0))
                .with_material(
                    if mode == SceneMode::UiPhysical {
                        10.0
                    } else {
                        0.0
                    },
                    0.0,
                    0.0,
                ),
            )
            .expect("card");

        let _card_edge_shadow = scene
            .append_node(
                viewport,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Shadow,
                    Rect::new(CARD_X, CARD_Y, CARD_W, CARD_H),
                )
                .named("card_edge_shadow")
                .with_key("card_edge_shadow")
                .with_ui_visual_role(UiVisualRole::BoxShadow)
                .with_ui_visual_style(todo_box_shadow_style(0.2, 1.0, -1.0)),
            )
            .expect("card edge shadow");

        let _card_top_highlight = scene
            .append_node(
                card,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Panel,
                    Rect::new(CARD_X, CARD_Y, CARD_W, 2.0),
                )
                .named("card_top_highlight")
                .with_key("card_top_highlight")
                .with_ui_visual_role(UiVisualRole::FilledSurface)
                .with_ui_visual_style(todo_fill_style(0, 0, 0, 0.03)),
            )
            .expect("card top highlight");

        let input = scene
            .append_node(
                card,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Input,
                    Rect::new(CARD_X, CARD_Y, CARD_W, INPUT_H),
                )
                .named("input")
                .with_key("input")
                .with_ui_visual_role(UiVisualRole::OutlineRect)
                .with_ui_visual_style(todo_input_outline_style()),
            )
            .expect("input");

        let _chevron = scene
            .append_node(
                input,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Separator,
                    Rect::new(CARD_X + 16.0, 158.8, 20.0, 9.0),
                )
                .named("input_chevron")
                .with_key("input_chevron")
                .with_ui_visual_role(UiVisualRole::ChevronMark)
                .with_ui_visual_style(todo_chevron_style()),
            )
            .expect("input chevron");

        let _placeholder = scene
            .append_node(
                input,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Text,
                    Rect::new(135.0, 148.4, 268.0, 33.6),
                )
                .named("placeholder")
                .with_key("placeholder")
                .with_text_slot(TODO_TEXT_RUN_PLACEHOLDER)
                .with_text("What needs to be done?", 24.0)
                .with_text_role(TextRole::Placeholder),
            )
            .expect("placeholder");

        let list_clip = scene
            .append_node(
                card,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Clip,
                    ElementKind::Clip,
                    Rect::new(CARD_X, LIST_TOP, CARD_W, LIST_BOTTOM - LIST_TOP),
                )
                .named("list_clip")
                .with_key("list_clip")
                .with_clip(),
            )
            .expect("list clip");

        let list_scroll = scene
            .append_node(
                list_clip,
                RenderNodeDescriptor::new(
                    RenderNodeKind::ScrollRoot,
                    ElementKind::ScrollContainer,
                    Rect::new(CARD_X, LIST_TOP, CARD_W, LIST_BOTTOM - LIST_TOP),
                )
                .named("list_scroll")
                .with_key("list_scroll")
                .with_scroll(ScrollState::new(
                    [CARD_W, LIST_BOTTOM - LIST_TOP],
                    [CARD_W, LIST_BOTTOM - LIST_TOP],
                )),
            )
            .expect("list scroll");

        for (name, y, style) in [
            ("sep_finish", 255.4, todo_separator_style(237, 237, 237)),
            ("sep_walk", 315.0, todo_separator_style(237, 237, 237)),
            ("sep_buy", 374.6, todo_separator_style(237, 237, 237)),
            ("sep_footer", 433.4, todo_separator_style(230, 230, 230)),
            ("sep_input", 195.8, todo_separator_style(237, 237, 237)),
        ] {
            scene
                .append_node(
                    card,
                    RenderNodeDescriptor::new(
                        RenderNodeKind::Primitive,
                        ElementKind::Separator,
                        Rect::new(CARD_X, y, CARD_W, 0.0),
                    )
                    .named(name)
                    .with_key(name)
                    .with_ui_visual_role(UiVisualRole::SeparatorLine)
                    .with_ui_visual_style(style),
                )
                .expect("separator");
        }

        let items = [
            (
                "item_read",
                "Read documentation",
                195.8,
                false,
                TextRole::Active,
            ),
            (
                "item_finish",
                "Finish TodoMVC renderer",
                255.4,
                true,
                TextRole::Completed,
            ),
            ("item_walk", "Walk the dog", 315.0, false, TextRole::Active),
            ("item_buy", "Buy groceries", 374.6, false, TextRole::Active),
        ];

        let mut retained_items = Vec::with_capacity(items.len());
        let mut item_node_indices = BTreeMap::new();

        for (index, (name, item_label, y, completed, text_role)) in items.into_iter().enumerate() {
            let item_ui_slot = match index {
                0 => TODO_UI_SLOT_ITEM_READ,
                1 => TODO_UI_SLOT_ITEM_FINISH,
                2 => TODO_UI_SLOT_ITEM_WALK,
                3 => TODO_UI_SLOT_ITEM_BUY,
                _ => unreachable!("unexpected TodoMVC item ui slot"),
            };
            let label_slot = match index {
                0 => TODO_TEXT_RUN_ITEM_READ_LABEL,
                1 => TODO_TEXT_RUN_ITEM_FINISH_LABEL,
                2 => TODO_TEXT_RUN_ITEM_WALK_LABEL,
                3 => TODO_TEXT_RUN_ITEM_BUY_LABEL,
                _ => unreachable!("unexpected TodoMVC item slot"),
            };
            let item = scene
                .append_node(
                    list_scroll,
                    RenderNodeDescriptor::new(
                        RenderNodeKind::Group,
                        ElementKind::Group,
                        Rect::new(CARD_X, y, CARD_W, ITEM_H),
                    )
                    .named(name)
                    .with_key(name)
                    .with_ui_slot(item_ui_slot)
                    .with_ui_primitive_count(TODO_UI_PRIMS_PER_ITEM as u16)
                    .with_material(
                        if mode == SceneMode::UiPhysical {
                            2.0
                        } else {
                            0.0
                        },
                        0.0,
                        0.0,
                    ),
                )
                .expect("todo item");

            let checkbox_name = format!("{name}_checkbox");
            let label_name = format!("{name}_label");
            let checkbox_label = if completed {
                "checkbox_checked"
            } else {
                "checkbox"
            };

            let checkbox = scene
                .append_node(
                    item,
                    RenderNodeDescriptor::new(
                        RenderNodeKind::Primitive,
                        ElementKind::Checkbox,
                        Rect::new(CHECKBOX_X, y + CHECKBOX_Y_OFFSET, 34.0, 34.0),
                    )
                    .named(checkbox_name.clone())
                    .with_key(checkbox_name)
                    .with_text(checkbox_label, 0.0)
                    .with_ui_visual_role(UiVisualRole::CheckboxControl)
                    .with_ui_visual_style(todo_checkbox_style())
                    .with_toggle_state(if completed {
                        ToggleState::On
                    } else {
                        ToggleState::Off
                    }),
                )
                .expect("checkbox");

            let label = scene
                .append_node(
                    item,
                    RenderNodeDescriptor::new(
                        RenderNodeKind::Primitive,
                        ElementKind::Text,
                        Rect::new(LABEL_X, y + LABEL_Y_OFFSET, 280.0, 28.0),
                    )
                    .named(label_name.clone())
                    .with_key(label_name)
                    .with_text_slot(label_slot)
                    .with_text(item_label, 24.0)
                    .with_ui_visual_role(UiVisualRole::CompletedTextDecoration)
                    .with_ui_visual_style(todo_completed_text_style())
                    .with_text_role(text_role),
                )
                .expect("todo text");

            item_node_indices.insert(item, index);
            item_node_indices.insert(checkbox, index);
            item_node_indices.insert(label, index);
            retained_items.push(TodoItemNodeIds {
                item,
                checkbox,
                label,
                label_text: Cow::Owned(item_label.to_string()),
                completed,
            });
        }

        let footer = scene
            .append_node(
                card,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Footer,
                    Rect::new(CARD_X, LIST_BOTTOM, CARD_W, FOOTER_H),
                )
                .named("footer")
                .with_key("footer"),
            )
            .expect("footer");

        let footer_texts = [
            (
                "items_left_count",
                "3",
                Rect::new(90.0, 445.0, 8.3, 15.0),
                15.0,
                ElementKind::Text,
                TextRole::Body,
                SelectionState::Unselected,
            ),
            (
                "items_left_suffix",
                " items left",
                Rect::new(98.3, 445.0, 79.7, 15.0),
                15.0,
                ElementKind::Text,
                TextRole::Body,
                SelectionState::Unselected,
            ),
            (
                "filter_all",
                "All",
                Rect::new(263.4, 445.0, 24.0, 15.0),
                15.0,
                ElementKind::Button,
                TextRole::Body,
                SelectionState::Selected,
            ),
            (
                "filter_active",
                "Active",
                Rect::new(301.6, 445.0, 44.0, 15.0),
                15.0,
                ElementKind::Button,
                TextRole::Body,
                SelectionState::Unselected,
            ),
            (
                "filter_completed",
                "Completed",
                Rect::new(364.1, 445.0, 75.0, 15.0),
                15.0,
                ElementKind::Button,
                TextRole::Body,
                SelectionState::Unselected,
            ),
            (
                "clear_completed",
                "Clear completed",
                Rect::new(500.8, 445.0, 103.0, 15.0),
                15.0,
                ElementKind::Button,
                TextRole::Body,
                SelectionState::Unselected,
            ),
        ];

        let mut items_left_count = None;
        let mut filter_all = None;
        let mut filter_active = None;
        let mut filter_completed = None;
        for (name, text, bounds, font_size, element, text_role, selection_state) in footer_texts {
            let ui_slot = match name {
                "filter_all" => Some(TODO_UI_SLOT_FILTER_ALL),
                "filter_active" => Some(TODO_UI_SLOT_FILTER_ACTIVE),
                "filter_completed" => Some(TODO_UI_SLOT_FILTER_COMPLETED),
                _ => None,
            };
            let text_slot = match name {
                "items_left_count" => TODO_TEXT_RUN_ITEMS_LEFT_COUNT,
                "items_left_suffix" => TODO_TEXT_RUN_ITEMS_LEFT_SUFFIX,
                "filter_all" => TODO_TEXT_RUN_FILTER_ALL,
                "filter_active" => TODO_TEXT_RUN_FILTER_ACTIVE,
                "filter_completed" => TODO_TEXT_RUN_FILTER_COMPLETED,
                "clear_completed" => TODO_TEXT_RUN_CLEAR_COMPLETED,
                _ => unreachable!("unexpected footer text slot"),
            };
            let mut descriptor =
                RenderNodeDescriptor::new(RenderNodeKind::Primitive, element, bounds)
                    .named(name)
                    .with_key(name)
                    .with_text_slot(text_slot)
                    .with_text(text, font_size)
                    .with_text_role(text_role)
                    .with_selection_state(selection_state);
            if let Some(ui_slot) = ui_slot {
                descriptor = descriptor
                    .with_ui_slot(ui_slot)
                    .with_ui_primitive_count(1)
                    .with_ui_visual_role(UiVisualRole::SelectionOutline)
                    .with_ui_visual_style(todo_filter_outline_style());
            }
            let id = scene.append_node(footer, descriptor).expect("footer node");

            match name {
                "items_left_count" => items_left_count = Some(id),
                "filter_all" => filter_all = Some(id),
                "filter_active" => filter_active = Some(id),
                "filter_completed" => filter_completed = Some(id),
                _ => {}
            }
        }

        let info = scene
            .append_node(
                viewport,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Group,
                    ElementKind::Info,
                    Rect::new(286.7, 538.4, 140.0, 55.0),
                )
                .named("info")
                .with_key("info"),
            )
            .expect("info");

        let info_lines = [
            (
                "info_edit",
                "Double-click to edit a todo",
                Rect::new(286.7, 538.4, 160.0, 11.0),
            ),
            (
                "info_author",
                "Created by Martin Kav\u{00ED}k",
                Rect::new(291.5, 560.4, 140.0, 11.0),
            ),
            (
                "info_brand",
                "Part of TodoMVC",
                Rect::new(308.1, 582.4, 90.0, 11.0),
            ),
        ];

        for (name, text, bounds) in info_lines {
            let text_slot = match name {
                "info_edit" => TODO_TEXT_RUN_INFO_EDIT,
                "info_author" => TODO_TEXT_RUN_INFO_AUTHOR,
                "info_brand" => TODO_TEXT_RUN_INFO_BRAND,
                _ => unreachable!("unexpected info text slot"),
            };
            scene
                .append_node(
                    info,
                    RenderNodeDescriptor::new(RenderNodeKind::Primitive, ElementKind::Text, bounds)
                        .named(name)
                        .with_key(name)
                        .with_text_slot(text_slot)
                        .with_text(text, 11.0)
                        .with_text_role(TextRole::Info),
                )
                .expect("info line");
        }

        scene.mark_node_dirty(heading);
        scene.mark_node_dirty(card_shadow);
        scene.mark_node_dirty(card);

        Self {
            scene,
            viewport,
            list_scroll,
            items_left_count: items_left_count.expect("items left count"),
            filter_all: filter_all.expect("filter all"),
            filter_active: filter_active.expect("filter active"),
            filter_completed: filter_completed.expect("filter completed"),
            items: retained_items,
            item_node_indices,
            filter: ListFilter::All,
            pending_changes: PendingResourceChanges::default(),
        }
    }

    pub fn scene(&self) -> &RetainedScene {
        &self.scene
    }

    pub fn scene_mut(&mut self) -> &mut RetainedScene {
        &mut self.scene
    }

    pub fn take_resource_dirty(&mut self) -> TodoMvcResourceDirty {
        let dirty = self.scene.take_dirty();

        if dirty.full_scene || !dirty.layers.is_empty() {
            self.pending_changes = PendingResourceChanges::default();
            return TodoMvcResourceDirty::all();
        }

        let mut resources = TodoMvcResourceDirty::none();
        for id in dirty.nodes {
            if id == self.viewport {
                continue;
            }

            let Some(node) = self.scene.node(id) else {
                self.pending_changes = PendingResourceChanges::default();
                return TodoMvcResourceDirty::all();
            };

            self.classify_resource_dirty(id, node, &mut resources);
            if resources.ui && resources.text {
                break;
            }
        }

        if !self.pending_changes.ui_slots.is_empty() {
            resources.ui = true;
            resources
                .ui_slots
                .extend(self.pending_changes.ui_slots.iter().copied());
        }
        if !self.pending_changes.text_style_nodes.is_empty() {
            resources.text = true;
            resources
                .text_style_nodes
                .extend(self.pending_changes.text_style_nodes.iter().copied());
        }
        if !self.pending_changes.text_layout_nodes.is_empty() {
            resources.text = true;
            resources
                .text_layout_nodes
                .extend(self.pending_changes.text_layout_nodes.iter().copied());
        }
        self.pending_changes = PendingResourceChanges::default();

        resources
    }

    pub fn set_viewport_size(&mut self, width: u32, height: u32) {
        self.scene.set_bounds(
            self.viewport,
            Rect::new(0.0, 0.0, width as f32, height as f32),
        );
    }

    pub fn mark_view_transform_dirty(&mut self) {
        self.scene.mark_node_dirty(self.viewport);
    }

    pub fn mark_visual_change(&mut self) {
        self.scene.mark_full_scene_dirty();
    }

    pub fn todo_count(&self) -> usize {
        self.items.len()
    }

    pub fn filter(&self) -> ListFilter {
        self.filter
    }

    pub fn set_item_label(&mut self, index: usize, label: impl Into<String>) -> bool {
        let Some(item) = self.items.get_mut(index) else {
            return false;
        };

        let label: Cow<'static, str> = Cow::Owned(label.into());
        if item.label_text == label {
            return false;
        }

        item.label_text = label.clone();
        self.pending_changes.text_layout_nodes.insert(item.label);
        self.scene
            .set_text(item.label, Some(TextNode::new(label, 24.0)));
        true
    }

    pub fn set_item_completed(&mut self, index: usize, completed: bool) -> bool {
        let Some(item) = self.items.get_mut(index) else {
            return false;
        };

        if item.completed == completed {
            return false;
        }

        item.completed = completed;
        self.scene.set_toggle_state(
            item.checkbox,
            Some(if completed {
                ToggleState::On
            } else {
                ToggleState::Off
            }),
        );
        self.scene.set_text_role(
            item.label,
            Some(if completed {
                TextRole::Completed
            } else {
                TextRole::Active
            }),
        );
        self.pending_changes
            .ui_slots
            .insert(todo_item_ui_slot_for_index(index));
        self.pending_changes.text_style_nodes.insert(item.label);
        self.update_items_left_count();
        self.relayout_items();
        true
    }

    pub fn toggle_item(&mut self, index: usize) -> bool {
        let Some(completed) = self.items.get(index).map(|item| item.completed) else {
            return false;
        };

        self.set_item_completed(index, !completed)
    }

    pub fn set_filter(&mut self, filter: ListFilter) -> bool {
        if self.filter == filter {
            return false;
        }

        self.filter = filter;
        self.scene.set_selection_state(
            self.filter_all,
            Some(if matches!(filter, ListFilter::All) {
                SelectionState::Selected
            } else {
                SelectionState::Unselected
            }),
        );
        self.scene.set_selection_state(
            self.filter_active,
            Some(if matches!(filter, ListFilter::Active) {
                SelectionState::Selected
            } else {
                SelectionState::Unselected
            }),
        );
        self.scene.set_selection_state(
            self.filter_completed,
            Some(if matches!(filter, ListFilter::Completed) {
                SelectionState::Selected
            } else {
                SelectionState::Unselected
            }),
        );
        self.pending_changes.ui_slots.extend([
            TODO_UI_SLOT_FILTER_ALL,
            TODO_UI_SLOT_FILTER_ACTIVE,
            TODO_UI_SLOT_FILTER_COMPLETED,
        ]);
        self.relayout_items();
        true
    }

    pub fn set_scroll_offset(&mut self, y: f32) {
        if let Some(scroll) = self
            .scene
            .node(self.list_scroll)
            .and_then(|node| node.scroll)
        {
            let old_visible = self.visible_item_indices_for_scroll_offset(scroll.offset[1]);
            let new_visible = self.visible_item_indices_for_scroll_offset(y);

            for index in old_visible.union(&new_visible).copied() {
                let item = &self.items[index];
                self.pending_changes
                    .ui_slots
                    .insert(todo_item_ui_slot_for_index(index));
                self.pending_changes.text_layout_nodes.insert(item.label);
            }
            self.scene.set_scroll_state(
                self.list_scroll,
                Some(ScrollState {
                    offset: [0.0, y],
                    ..scroll
                }),
            );
        }
    }

    fn visible_item_indices_for_scroll_offset(&self, scroll_y: f32) -> BTreeSet<usize> {
        self.scene
            .visible_descendants_in_scroll_root_sorted_by_resolved_position(
                self.list_scroll,
                [0.0, scroll_y],
            )
            .into_iter()
            .filter_map(|node| self.item_index_for_node(node.id))
            .collect()
    }

    fn update_items_left_count(&mut self) {
        let active_count = self.items.iter().filter(|item| !item.completed).count();
        self.scene.set_text(
            self.items_left_count,
            Some(TextNode::new(active_count.to_string(), 15.0)),
        );
        self.pending_changes
            .text_layout_nodes
            .insert(self.items_left_count);
    }

    fn item_matches_filter(&self, item: &TodoItemNodeIds) -> bool {
        match self.filter {
            ListFilter::All => true,
            ListFilter::Active => !item.completed,
            ListFilter::Completed => item.completed,
        }
    }

    fn relayout_items(&mut self) {
        let mut visible_index = 0usize;

        for index in 0..self.items.len() {
            let (item_id, checkbox_id, label_id) = {
                let item = &self.items[index];
                (item.item, item.checkbox, item.label)
            };
            let is_visible = self.item_matches_filter(&self.items[index]);

            let row_y = if is_visible {
                let y = LIST_TOP + visible_index as f32 * ITEM_H;
                visible_index += 1;
                y
            } else {
                LIST_BOTTOM + HIDDEN_ITEM_Y_OFFSET + index as f32 * ITEM_H
            };

            let item_changed = self
                .scene
                .set_bounds(item_id, Rect::new(CARD_X, row_y, CARD_W, ITEM_H));
            let checkbox_changed = self.scene.set_bounds(
                checkbox_id,
                Rect::new(CHECKBOX_X, row_y + CHECKBOX_Y_OFFSET, 34.0, 34.0),
            );
            let label_changed = self.scene.set_bounds(
                label_id,
                Rect::new(LABEL_X, row_y + LABEL_Y_OFFSET, 280.0, 28.0),
            );

            if item_changed || checkbox_changed || label_changed {
                self.pending_changes
                    .ui_slots
                    .insert(todo_item_ui_slot_for_index(index));
                self.pending_changes.text_layout_nodes.insert(label_id);
            }
        }

        if let Some(mut scroll) = self
            .scene
            .node(self.list_scroll)
            .and_then(|node| node.scroll)
        {
            let viewport_h = scroll.viewport_size[1];
            let content_h = (visible_index as f32 * ITEM_H).max(viewport_h);
            let max_scroll = (content_h - viewport_h).max(0.0);
            scroll.content_size[1] = content_h;
            scroll.offset[1] = scroll.offset[1].clamp(0.0, max_scroll);
            self.scene.set_scroll_state(self.list_scroll, Some(scroll));
        }
    }

    fn classify_resource_dirty(
        &self,
        id: NodeId,
        node: &RenderNode,
        resources: &mut TodoMvcResourceDirty,
    ) {
        if id == self.list_scroll {
            resources.ui = true;
            resources.text = true;
            return;
        }

        if node.kind == RenderNodeKind::ScrollRoot || node.clip || node.scroll.is_some() {
            *resources = TodoMvcResourceDirty::all();
            return;
        }

        if let Some(index) = self.item_index_for_node(id) {
            if id == self.items[index].checkbox || id == self.items[index].item {
                resources.ui = true;
                resources
                    .ui_slots
                    .insert(todo_item_ui_slot_for_index(index));
            }
            if id == self.items[index].label {
                resources.text = true;
                if !self.pending_changes.text_style_nodes.contains(&id)
                    && !self.pending_changes.text_layout_nodes.contains(&id)
                {
                    resources.full_text = true;
                }
            }
        }

        if id == self.filter_all || id == self.filter_active || id == self.filter_completed {
            resources.ui = true;
            resources.ui_slots.extend([
                TODO_UI_SLOT_FILTER_ALL,
                TODO_UI_SLOT_FILTER_ACTIVE,
                TODO_UI_SLOT_FILTER_COMPLETED,
            ]);
        }

        if node.text.is_some() {
            resources.text = true;
            if !self.pending_changes.text_style_nodes.contains(&id)
                && !self.pending_changes.text_layout_nodes.contains(&id)
            {
                resources.full_text = true;
            }
        }

        if node.toggle_state.is_some() || node.selection_state.is_some() {
            resources.ui = true;
        }

        match node.element {
            ElementKind::Text | ElementKind::Heading | ElementKind::Info => {
                resources.text = true;
                if !self.pending_changes.text_style_nodes.contains(&id)
                    && !self.pending_changes.text_layout_nodes.contains(&id)
                {
                    resources.full_text = true;
                }
            }
            ElementKind::Checkbox | ElementKind::Shadow | ElementKind::Separator => {
                resources.ui = true;
            }
            ElementKind::Button => {
                resources.ui = true;
            }
            ElementKind::Panel
            | ElementKind::Input
            | ElementKind::Footer
            | ElementKind::Clip
            | ElementKind::ScrollContainer
            | ElementKind::Viewport
            | ElementKind::Group
            | ElementKind::Root => {
                resources.ui = true;
                resources.full_ui = true;
            }
        }
    }

    fn item_index_for_node(&self, id: NodeId) -> Option<usize> {
        self.item_node_indices.get(&id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Rect, TodoMvcResourceDirty, TodoMvcRetainedScene, CARD_W, CARD_X, ITEM_H, LIST_BOTTOM,
        LIST_TOP,
    };
    use crate::retained::{ElementKind, NodeId, SceneMode, TextNode, ToggleState};
    use crate::todomvc_shared::{
        todo_item_ui_slot_for_index, TODO_UI_SLOT_FILTER_ACTIVE, TODO_UI_SLOT_FILTER_ALL,
        TODO_UI_SLOT_FILTER_COMPLETED, TODO_UI_SLOT_ITEM_BUY,
    };
    use std::collections::BTreeSet;

    fn ui_slots(slots: impl IntoIterator<Item = u16>) -> BTreeSet<u16> {
        slots.into_iter().collect()
    }

    fn node_text<'a>(scene: &'a TodoMvcRetainedScene, name: &str) -> &'a str {
        scene
            .scene()
            .node_named(name)
            .and_then(|node| node.text.as_ref())
            .map(|text| text.text.as_ref())
            .expect("text node")
    }

    fn node_id(scene: &TodoMvcRetainedScene, name: &str) -> NodeId {
        scene
            .scene()
            .node_named(name)
            .map(|node| node.id)
            .expect("named node")
    }

    #[test]
    fn creates_scroll_aware_todomvc_scene() {
        let scene = TodoMvcRetainedScene::new(SceneMode::Ui2D, 800, 600);
        let nodes = scene.scene().nodes();

        assert!(nodes
            .values()
            .any(|node| node.element == ElementKind::ScrollContainer));
        assert!(nodes
            .values()
            .any(|node| node.name.as_deref() == Some("card")));
        assert!(nodes.values().any(|node| {
            node.text.as_ref().map(|text| text.text.as_ref()) == Some("Finish TodoMVC renderer")
        }));
    }

    #[test]
    fn resizing_marks_viewport_dirty() {
        let mut scene = TodoMvcRetainedScene::new(SceneMode::UiPhysical, 700, 700);
        scene.scene_mut().clear_dirty();

        scene.set_viewport_size(1024, 768);

        assert!(!scene.scene().dirty().is_empty());
    }

    #[test]
    fn viewport_only_dirty_does_not_require_resource_rebuild() {
        let mut scene = TodoMvcRetainedScene::new(SceneMode::Ui2D, 700, 700);
        scene.scene_mut().clear_dirty();

        scene.set_viewport_size(800, 600);

        assert!(!scene.scene().dirty().is_empty());
        assert_eq!(scene.take_resource_dirty(), TodoMvcResourceDirty::none());
    }

    #[test]
    fn scroll_dirty_requires_resource_rebuild() {
        let mut scene = TodoMvcRetainedScene::new(SceneMode::Ui2D, 700, 700);
        scene.scene_mut().clear_dirty();
        let expected_text_layout_nodes = scene
            .items
            .iter()
            .map(|item| item.label)
            .collect::<BTreeSet<_>>();

        scene.set_scroll_offset(48.0);

        assert_eq!(
            scene.take_resource_dirty(),
            TodoMvcResourceDirty {
                ui: true,
                text: true,
                full_ui: false,
                full_text: false,
                ui_slots: ui_slots(
                    scene
                        .items
                        .iter()
                        .enumerate()
                        .map(|(index, _)| todo_item_ui_slot_for_index(index)),
                ),
                text_style_nodes: BTreeSet::new(),
                text_layout_nodes: expected_text_layout_nodes,
            }
        );
    }

    #[test]
    fn scroll_dirty_tracks_union_of_old_and_new_visible_items() {
        let mut scene = TodoMvcRetainedScene::new(SceneMode::Ui2D, 700, 700);
        scene.scene_mut().clear_dirty();

        scene.set_scroll_offset(120.0);
        let _ = scene.take_resource_dirty();

        scene.set_scroll_offset(160.0);

        assert_eq!(
            scene.take_resource_dirty(),
            TodoMvcResourceDirty {
                ui: true,
                text: true,
                full_ui: false,
                full_text: false,
                ui_slots: ui_slots([2, 3].into_iter().map(todo_item_ui_slot_for_index),),
                text_style_nodes: BTreeSet::new(),
                text_layout_nodes: scene
                    .items
                    .iter()
                    .enumerate()
                    .filter_map(|(index, item)| [2, 3].contains(&index).then_some(item.label))
                    .collect(),
            }
        );
    }

    #[test]
    fn checkbox_toggle_marks_checkbox_slot_dirty() {
        let mut scene = TodoMvcRetainedScene::new(SceneMode::Ui2D, 700, 700);
        scene.scene_mut().clear_dirty();

        let checkbox = scene
            .scene()
            .nodes()
            .values()
            .find(|node| node.name.as_deref() == Some("item_read_checkbox"))
            .map(|node| node.id)
            .expect("checkbox node");
        scene
            .scene_mut()
            .set_toggle_state(checkbox, Some(ToggleState::On));

        let dirty = scene.take_resource_dirty();
        assert!(dirty.ui);
        assert!(!dirty.full_ui);
        assert_eq!(dirty.ui_slots, ui_slots([todo_item_ui_slot_for_index(0)]));
    }

    #[test]
    fn text_update_requires_only_text_rebuild() {
        let mut scene = TodoMvcRetainedScene::new(SceneMode::Ui2D, 700, 700);
        scene.scene_mut().clear_dirty();

        let info = scene
            .scene()
            .nodes()
            .values()
            .find(|node| node.name.as_deref() == Some("info_author"))
            .map(|node| node.id)
            .expect("info text node");
        scene
            .scene_mut()
            .set_text(info, Some(TextNode::new("Created by retained", 11.0)));

        assert_eq!(
            scene.take_resource_dirty(),
            TodoMvcResourceDirty {
                ui: false,
                text: true,
                full_ui: false,
                full_text: true,
                ui_slots: BTreeSet::new(),
                text_style_nodes: BTreeSet::new(),
                text_layout_nodes: BTreeSet::new(),
            }
        );
    }

    #[test]
    fn toggling_item_updates_footer_count_and_text_role() {
        let mut scene = TodoMvcRetainedScene::new(SceneMode::Ui2D, 700, 700);
        scene.scene_mut().clear_dirty();
        let item_label = node_id(&scene, "item_read_label");
        let items_left_count = node_id(&scene, "items_left_count");

        assert!(scene.toggle_item(0));

        assert_eq!(node_text(&scene, "items_left_count"), "2");
        let dirty = scene.take_resource_dirty();
        assert!(dirty.ui);
        assert!(dirty.text);
        assert!(!dirty.full_ui);
        assert!(!dirty.full_text);
        assert_eq!(
            dirty.ui_slots,
            ui_slots([todo_item_ui_slot_for_index(0), TODO_UI_SLOT_ITEM_BUY])
        );
        assert_eq!(dirty.text_style_nodes, [item_label].into_iter().collect());
        assert!(dirty.text_layout_nodes.contains(&items_left_count));
    }

    #[test]
    fn selecting_filter_reflows_items_and_text() {
        let mut scene = TodoMvcRetainedScene::new(SceneMode::Ui2D, 700, 700);
        scene.scene_mut().clear_dirty();

        assert!(scene.set_filter(crate::demo_core::ListFilter::Completed));

        assert_eq!(
            scene.take_resource_dirty(),
            TodoMvcResourceDirty {
                ui: true,
                text: true,
                full_ui: true,
                full_text: true,
                ui_slots: ui_slots(
                    scene
                        .items
                        .iter()
                        .enumerate()
                        .map(|(index, _)| todo_item_ui_slot_for_index(index))
                        .chain([
                            TODO_UI_SLOT_FILTER_ALL,
                            TODO_UI_SLOT_FILTER_ACTIVE,
                            TODO_UI_SLOT_FILTER_COMPLETED,
                        ]),
                ),
                text_style_nodes: BTreeSet::new(),
                text_layout_nodes: scene.items.iter().map(|item| item.label).collect(),
            }
        );
    }

    #[test]
    fn completed_filter_compacts_visible_item_to_top_row() {
        let mut scene = TodoMvcRetainedScene::new(SceneMode::Ui2D, 700, 700);
        scene.scene_mut().clear_dirty();

        assert!(scene.set_filter(crate::demo_core::ListFilter::Completed));

        let visible_item_bounds = scene
            .scene()
            .node(scene.items[1].item)
            .expect("visible item")
            .bounds;
        let hidden_item_bounds = scene
            .scene()
            .node(scene.items[0].item)
            .expect("hidden item")
            .bounds;

        assert_eq!(
            visible_item_bounds,
            Rect::new(CARD_X, LIST_TOP, CARD_W, ITEM_H)
        );
        assert!(hidden_item_bounds.y > LIST_BOTTOM + 900.0);
    }
}
