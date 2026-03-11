//! Retained scene scaffolding for future UI renderers.
//!
//! This starts with stable identity, dirty tracking, and a lightweight semantic
//! payload so real UI scenes can lower into a shared retained structure before
//! the renderer rewrite lands.

pub mod fixed_scene;
pub mod samples;
pub mod showcase;
pub mod text;
pub mod text_scene;
pub mod ui;

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

pub trait NamedScrollSceneModel {
    fn set_named_scroll_offset(&self, scene: &mut RetainedScene, name: &str, offset_y: f32)
        -> bool;
}

/// Rendering family selected for a semantic scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneMode {
    Ui2D,
    UiPhysical,
    World3D,
}

/// Stable retained node identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(u64);

impl NodeId {
    pub const ROOT: Self = Self(1);

    pub fn get(self) -> u64 {
        self.0
    }
}

/// Optional layer identifier for cached retained subtrees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayerId(u32);

impl LayerId {
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    pub fn get(self) -> u32 {
        self.0
    }
}

/// Coarse retained node categories used by early scene lowering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderNodeKind {
    Group,
    Primitive,
    Clip,
    ScrollRoot,
    Viewport,
}

/// Semantic element category attached to a retained node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementKind {
    Root,
    Group,
    Panel,
    Shadow,
    Heading,
    Input,
    Checkbox,
    Text,
    Button,
    Footer,
    Info,
    Separator,
    Clip,
    ScrollContainer,
    Viewport,
}

/// Semantic text styling role for retained text nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRole {
    Heading,
    Placeholder,
    Active,
    Completed,
    Body,
    Info,
}

/// Binary toggle state for semantic controls such as checkboxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleState {
    On,
    Off,
}

/// Selection state for buttons, filters, and similar controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionState {
    Selected,
    Unselected,
}

/// Renderer-facing UI decoration semantics for retained nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiVisualRole {
    BoxShadow,
    FilledSurface,
    SeparatorLine,
    ChevronMark,
    OutlineRect,
    CheckboxControl,
    CompletedTextDecoration,
    SelectionOutline,
}

/// Renderer-facing style parameters for retained UI decorations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiVisualStyle {
    pub base_color: [f32; 4],
    pub accent_color: [f32; 4],
    pub detail_color: [f32; 4],
    pub stroke_width: f32,
    pub corner_radius: f32,
    pub offset: [f32; 2],
    pub extra_size: [f32; 2],
}

/// Lightweight axis-aligned rectangle for retained layout/culling.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn translate(self, dx: f32, dy: f32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            width: self.width,
            height: self.height,
        }
    }

    pub fn intersect(self, other: Self) -> Option<Self> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.width).min(other.x + other.width);
        let y2 = (self.y + self.height).min(other.y + other.height);

        if x2 <= x1 || y2 <= y1 {
            None
        } else {
            Some(Self::new(x1, y1, x2 - x1, y2 - y1))
        }
    }

    pub fn intersects(self, other: Self) -> bool {
        self.intersect(other).is_some()
    }
}

/// Semantic text payload attached to a retained node.
#[derive(Debug, Clone, PartialEq)]
pub struct TextNode {
    pub text: Cow<'static, str>,
    pub font_size: f32,
}

impl TextNode {
    pub fn new(text: impl Into<Cow<'static, str>>, font_size: f32) -> Self {
        Self {
            text: text.into(),
            font_size,
        }
    }
}

/// Scroll metadata for retained scroll roots.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollState {
    pub offset: [f32; 2],
    pub viewport_size: [f32; 2],
    pub content_size: [f32; 2],
}

impl ScrollState {
    pub fn new(viewport_size: [f32; 2], content_size: [f32; 2]) -> Self {
        Self {
            offset: [0.0, 0.0],
            viewport_size,
            content_size,
        }
    }
}

/// Builder-style node descriptor used when lowering semantic scenes.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderNodeDescriptor {
    pub kind: RenderNodeKind,
    pub element: ElementKind,
    pub bounds: Rect,
    pub name: Option<Cow<'static, str>>,
    pub key: Option<Cow<'static, str>>,
    pub ui_slot: Option<u16>,
    pub ui_primitive_count: u16,
    pub text_slot: Option<u16>,
    pub text: Option<TextNode>,
    pub clip: bool,
    pub scroll: Option<ScrollState>,
    pub ui_visual_role: Option<UiVisualRole>,
    pub ui_visual_style: Option<UiVisualStyle>,
    pub text_role: Option<TextRole>,
    pub toggle_state: Option<ToggleState>,
    pub selection_state: Option<SelectionState>,
    pub elevation: f32,
    pub depth: f32,
    pub corner_radius: f32,
}

impl RenderNodeDescriptor {
    pub fn new(kind: RenderNodeKind, element: ElementKind, bounds: Rect) -> Self {
        Self {
            kind,
            element,
            bounds,
            name: None,
            key: None,
            ui_slot: None,
            ui_primitive_count: 0,
            text_slot: None,
            text: None,
            clip: false,
            scroll: None,
            ui_visual_role: None,
            ui_visual_style: None,
            text_role: None,
            toggle_state: None,
            selection_state: None,
            elevation: 0.0,
            depth: 0.0,
            corner_radius: 0.0,
        }
    }

    pub fn named(mut self, name: impl Into<Cow<'static, str>>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_key(mut self, key: impl Into<Cow<'static, str>>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn with_ui_slot(mut self, ui_slot: u16) -> Self {
        self.ui_slot = Some(ui_slot);
        self
    }

    pub fn with_ui_primitive_count(mut self, ui_primitive_count: u16) -> Self {
        self.ui_primitive_count = ui_primitive_count;
        self
    }

    pub fn with_text_slot(mut self, text_slot: u16) -> Self {
        self.text_slot = Some(text_slot);
        self
    }

    pub fn with_text(mut self, text: impl Into<Cow<'static, str>>, font_size: f32) -> Self {
        self.text = Some(TextNode::new(text, font_size));
        self
    }

    pub fn with_ui_visual_role(mut self, role: UiVisualRole) -> Self {
        self.ui_visual_role = Some(role);
        self
    }

    pub fn with_ui_visual_style(mut self, style: UiVisualStyle) -> Self {
        self.ui_visual_style = Some(style);
        self
    }

    pub fn with_text_role(mut self, role: TextRole) -> Self {
        self.text_role = Some(role);
        self
    }

    pub fn with_clip(mut self) -> Self {
        self.clip = true;
        self
    }

    pub fn with_scroll(mut self, scroll: ScrollState) -> Self {
        self.scroll = Some(scroll);
        self
    }

    pub fn with_toggle_state(mut self, state: ToggleState) -> Self {
        self.toggle_state = Some(state);
        self
    }

    pub fn with_selection_state(mut self, state: SelectionState) -> Self {
        self.selection_state = Some(state);
        self
    }

    pub fn with_material(mut self, elevation: f32, depth: f32, corner_radius: f32) -> Self {
        self.elevation = elevation;
        self.depth = depth;
        self.corner_radius = corner_radius;
        self
    }
}

/// A retained render node with stable identity and parent/child links.
#[derive(Debug, Clone)]
pub struct RenderNode {
    pub id: NodeId,
    pub kind: RenderNodeKind,
    pub element: ElementKind,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub bounds: Rect,
    pub name: Option<Cow<'static, str>>,
    pub key: Option<Cow<'static, str>>,
    pub ui_slot: Option<u16>,
    pub ui_primitive_count: u16,
    pub text_slot: Option<u16>,
    pub text: Option<TextNode>,
    pub clip: bool,
    pub scroll: Option<ScrollState>,
    pub ui_visual_role: Option<UiVisualRole>,
    pub ui_visual_style: Option<UiVisualStyle>,
    pub text_role: Option<TextRole>,
    pub toggle_state: Option<ToggleState>,
    pub selection_state: Option<SelectionState>,
    pub elevation: f32,
    pub depth: f32,
    pub corner_radius: f32,
    pub layer: Option<LayerId>,
}

impl RenderNode {
    fn new(id: NodeId, parent: Option<NodeId>, descriptor: RenderNodeDescriptor) -> Self {
        Self {
            id,
            kind: descriptor.kind,
            element: descriptor.element,
            parent,
            children: Vec::new(),
            bounds: descriptor.bounds,
            name: descriptor.name,
            key: descriptor.key,
            ui_slot: descriptor.ui_slot,
            ui_primitive_count: descriptor.ui_primitive_count,
            text_slot: descriptor.text_slot,
            text: descriptor.text,
            clip: descriptor.clip,
            scroll: descriptor.scroll,
            ui_visual_role: descriptor.ui_visual_role,
            ui_visual_style: descriptor.ui_visual_style,
            text_role: descriptor.text_role,
            toggle_state: descriptor.toggle_state,
            selection_state: descriptor.selection_state,
            elevation: descriptor.elevation,
            depth: descriptor.depth,
            corner_radius: descriptor.corner_radius,
            layer: None,
        }
    }
}

/// Incremental invalidation state for retained rendering work.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirtySet {
    pub full_scene: bool,
    pub nodes: BTreeSet<NodeId>,
    pub layers: BTreeSet<LayerId>,
    pub scroll_resource_dirty: BTreeMap<NodeId, ResourceDirtySet>,
}

impl DirtySet {
    pub fn mark_full_scene(&mut self) {
        self.full_scene = true;
    }

    pub fn mark_node(&mut self, id: NodeId) {
        self.nodes.insert(id);
    }

    pub fn mark_layer(&mut self, id: LayerId) {
        self.layers.insert(id);
    }

    pub fn mark_scroll_resources(&mut self, id: NodeId, resources: ResourceDirtySet) {
        if resources.is_empty() {
            self.scroll_resource_dirty.remove(&id);
        } else {
            self.scroll_resource_dirty.insert(id, resources);
        }
    }

    pub fn is_empty(&self) -> bool {
        !self.full_scene
            && self.nodes.is_empty()
            && self.layers.is_empty()
            && self.scroll_resource_dirty.is_empty()
    }

    fn clear(&mut self) {
        self.full_scene = false;
        self.nodes.clear();
        self.layers.clear();
        self.scroll_resource_dirty.clear();
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResourceDirtySet {
    pub full_text: bool,
    pub full_ui: bool,
    pub text_slots: BTreeSet<u16>,
    pub ui_slots: BTreeSet<u16>,
}

impl ResourceDirtySet {
    pub fn is_empty(&self) -> bool {
        !self.full_text && !self.full_ui && self.text_slots.is_empty() && self.ui_slots.is_empty()
    }

    pub fn merge_from(&mut self, other: &Self) {
        self.full_text |= other.full_text;
        self.full_ui |= other.full_ui;
        self.text_slots.extend(other.text_slots.iter().copied());
        self.ui_slots.extend(other.ui_slots.iter().copied());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiSlotRange {
    pub node_id: NodeId,
    pub ui_slot: u16,
    pub offset: usize,
    pub primitive_count: usize,
}

/// Shared retained scene state for semantic UI and world modes.
#[derive(Debug, Clone)]
pub struct RetainedScene {
    mode: SceneMode,
    root: NodeId,
    next_node_id: u64,
    nodes: BTreeMap<NodeId, RenderNode>,
    dirty: DirtySet,
}

impl RetainedScene {
    pub fn new(mode: SceneMode) -> Self {
        let root = NodeId::ROOT;
        let mut nodes = BTreeMap::new();
        nodes.insert(
            root,
            RenderNode::new(
                root,
                None,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Group,
                    ElementKind::Root,
                    Rect::default(),
                )
                .named("root"),
            ),
        );

        let mut dirty = DirtySet::default();
        dirty.mark_full_scene();
        dirty.mark_node(root);

        Self {
            mode,
            root,
            next_node_id: root.get() + 1,
            nodes,
            dirty,
        }
    }

    pub fn mode(&self) -> SceneMode {
        self.mode
    }

    pub fn root(&self) -> NodeId {
        self.root
    }

    pub fn node(&self, id: NodeId) -> Option<&RenderNode> {
        self.nodes.get(&id)
    }

    pub fn node_named(&self, name: &str) -> Option<&RenderNode> {
        self.nodes
            .values()
            .find(|node| node.name.as_deref() == Some(name))
    }

    pub fn node_keyed(&self, key: &str) -> Option<&RenderNode> {
        self.nodes
            .values()
            .find(|node| node.key.as_deref() == Some(key))
    }

    pub fn node_with_text_slot(&self, text_slot: u16) -> Option<&RenderNode> {
        self.nodes
            .values()
            .find(|node| node.text_slot == Some(text_slot))
    }

    pub fn node_with_ui_slot(&self, ui_slot: u16) -> Option<&RenderNode> {
        self.nodes
            .values()
            .find(|node| node.ui_slot == Some(ui_slot))
    }

    pub fn nodes_with_ui_slots_sorted(&self) -> Vec<&RenderNode> {
        let mut nodes = self
            .nodes
            .values()
            .filter(|node| node.ui_slot.is_some())
            .collect::<Vec<_>>();
        nodes.sort_by(|a, b| {
            a.ui_slot.cmp(&b.ui_slot).then_with(|| {
                self.resolved_bounds(a.id)
                    .map(|bounds| (bounds.y, bounds.x))
                    .unwrap_or((a.bounds.y, a.bounds.x))
                    .partial_cmp(
                        &self
                            .resolved_bounds(b.id)
                            .map(|bounds| (bounds.y, bounds.x))
                            .unwrap_or((b.bounds.y, b.bounds.x)),
                    )
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });
        nodes
    }

    pub fn ui_slot_ranges(&self, base_offset: usize) -> Vec<UiSlotRange> {
        let mut offset = base_offset;
        let mut ranges = Vec::new();

        for node in self.nodes_with_ui_slots_sorted() {
            let Some(ui_slot) = node.ui_slot else {
                continue;
            };
            let primitive_count = node.ui_primitive_count as usize;
            ranges.push(UiSlotRange {
                node_id: node.id,
                ui_slot,
                offset,
                primitive_count,
            });
            offset += primitive_count;
        }

        ranges
    }

    pub fn ui_slot_range(&self, ui_slot: u16, base_offset: usize) -> Option<UiSlotRange> {
        self.ui_slot_ranges(base_offset)
            .into_iter()
            .find(|range| range.ui_slot == ui_slot)
    }

    pub fn total_ui_primitive_count(&self, base_offset: usize) -> usize {
        self.ui_slot_ranges(base_offset)
            .into_iter()
            .map(|range| range.primitive_count)
            .sum::<usize>()
            + base_offset
    }

    pub fn nodes(&self) -> &BTreeMap<NodeId, RenderNode> {
        &self.nodes
    }

    pub fn children_of(&self, parent: NodeId) -> Vec<&RenderNode> {
        let Some(parent_node) = self.node(parent) else {
            return Vec::new();
        };

        parent_node
            .children
            .iter()
            .filter_map(|child| self.node(*child))
            .collect()
    }

    pub fn first_child_with_element(
        &self,
        parent: NodeId,
        element: ElementKind,
    ) -> Option<&RenderNode> {
        self.children_of(parent)
            .into_iter()
            .find(|node| node.element == element)
    }

    pub fn children_with_element(&self, parent: NodeId, element: ElementKind) -> Vec<&RenderNode> {
        self.children_of(parent)
            .into_iter()
            .filter(|node| node.element == element)
            .collect()
    }

    pub fn first_child_with_ui_visual_role(
        &self,
        parent: NodeId,
        role: UiVisualRole,
    ) -> Option<&RenderNode> {
        self.children_of(parent)
            .into_iter()
            .find(|node| node.ui_visual_role == Some(role))
    }

    pub fn descendants_with_ui_visual_role(
        &self,
        root: NodeId,
        role: UiVisualRole,
    ) -> Vec<&RenderNode> {
        self.descendants_of(root)
            .into_iter()
            .filter(|node| node.ui_visual_role == Some(role))
            .collect()
    }

    pub fn nodes_with_element(&self, element: ElementKind) -> Vec<&RenderNode> {
        self.nodes
            .values()
            .filter(|node| node.element == element)
            .collect()
    }

    pub fn descendants_of(&self, parent: NodeId) -> Vec<&RenderNode> {
        let mut out = Vec::new();
        self.collect_descendants(parent, &mut out);
        out
    }

    pub fn descendants_with_element(
        &self,
        parent: NodeId,
        element: ElementKind,
    ) -> Vec<&RenderNode> {
        self.descendants_of(parent)
            .into_iter()
            .filter(|node| node.element == element)
            .collect()
    }

    pub fn visible_descendants_of(&self, parent: NodeId) -> Vec<&RenderNode> {
        self.descendants_of(parent)
            .into_iter()
            .filter(|node| self.is_node_visible(node.id))
            .collect()
    }

    pub fn descendants_with_text(&self, parent: NodeId) -> Vec<&RenderNode> {
        self.descendants_of(parent)
            .into_iter()
            .filter(|node| node.text.is_some())
            .collect()
    }

    pub fn visible_descendants_with_text(&self, parent: NodeId) -> Vec<&RenderNode> {
        self.visible_descendants_of(parent)
            .into_iter()
            .filter(|node| node.text.is_some())
            .collect()
    }

    pub fn descendants_sorted_by_resolved_position(&self, parent: NodeId) -> Vec<&RenderNode> {
        let mut nodes = self.descendants_of(parent);
        self.sort_nodes_by_resolved_position(&mut nodes);
        nodes
    }

    pub fn descendants_with_element_sorted_by_resolved_position(
        &self,
        parent: NodeId,
        element: ElementKind,
    ) -> Vec<&RenderNode> {
        let mut nodes = self.descendants_with_element(parent, element);
        self.sort_nodes_by_resolved_position(&mut nodes);
        nodes
    }

    pub fn visible_descendants_with_text_sorted_by_resolved_position(
        &self,
        parent: NodeId,
    ) -> Vec<&RenderNode> {
        let mut nodes = self.visible_descendants_with_text(parent);
        self.sort_nodes_by_resolved_position(&mut nodes);
        nodes
    }

    pub fn dirty(&self) -> &DirtySet {
        &self.dirty
    }

    pub fn take_dirty(&mut self) -> DirtySet {
        let mut dirty = DirtySet::default();
        std::mem::swap(&mut dirty, &mut self.dirty);
        dirty
    }

    pub fn classify_resource_dirty(&self, dirty: &DirtySet) -> ResourceDirtySet {
        let mut resources = ResourceDirtySet::default();
        if dirty.full_scene {
            resources.full_text = true;
            resources.full_ui = true;
            return resources;
        }

        for &id in &dirty.nodes {
            let Some(node) = self.node(id) else {
                resources.full_text = true;
                resources.full_ui = true;
                continue;
            };

            if let Some(text_slot) = node.text_slot {
                resources.text_slots.insert(text_slot);
            }
            if let Some(ui_slot) = node.ui_slot {
                resources.ui_slots.insert(ui_slot);
            }
            if node.ui_visual_role.is_some() && node.ui_slot.is_none() {
                resources.full_ui = true;
            }
            if let Some(scroll_resources) = dirty.scroll_resource_dirty.get(&id) {
                resources.merge_from(scroll_resources);
                continue;
            }
            if node.clip || node.scroll.is_some() {
                for descendant in self.descendants_of(id) {
                    if let Some(text_slot) = descendant.text_slot {
                        resources.text_slots.insert(text_slot);
                    }
                    if let Some(ui_slot) = descendant.ui_slot {
                        resources.ui_slots.insert(ui_slot);
                    }
                    if descendant.ui_visual_role.is_some() && descendant.ui_slot.is_none() {
                        resources.full_ui = true;
                    }
                }
            }
        }

        resources
    }

    pub fn clear_dirty(&mut self) {
        self.dirty.clear();
    }

    pub fn append_child(
        &mut self,
        parent: NodeId,
        kind: RenderNodeKind,
        bounds: Rect,
    ) -> Option<NodeId> {
        self.append_node(
            parent,
            RenderNodeDescriptor::new(kind, ElementKind::Group, bounds),
        )
    }

    pub fn append_node(
        &mut self,
        parent: NodeId,
        descriptor: RenderNodeDescriptor,
    ) -> Option<NodeId> {
        let parent_node = self.nodes.get_mut(&parent)?;
        let id = NodeId(self.next_node_id);
        self.next_node_id += 1;

        parent_node.children.push(id);
        self.nodes
            .insert(id, RenderNode::new(id, Some(parent), descriptor));

        self.dirty.mark_node(parent);
        self.dirty.mark_node(id);
        Some(id)
    }

    pub fn set_bounds(&mut self, id: NodeId, bounds: Rect) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };

        if node.bounds == bounds {
            return false;
        }

        node.bounds = bounds;
        self.dirty.mark_node(id);
        true
    }

    pub fn set_layer(&mut self, id: NodeId, layer: Option<LayerId>) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };

        if node.layer == layer {
            return false;
        }

        if let Some(existing) = node.layer {
            self.dirty.mark_layer(existing);
        }
        if let Some(new_layer) = layer {
            self.dirty.mark_layer(new_layer);
        }

        node.layer = layer;
        self.dirty.mark_node(id);
        true
    }

    pub fn set_scroll_state(&mut self, id: NodeId, scroll: Option<ScrollState>) -> bool {
        let Some(previous_scroll) = self.nodes.get(&id).map(|node| node.scroll) else {
            return false;
        };

        if previous_scroll == scroll {
            return false;
        }

        if let (Some(previous), Some(next)) = (previous_scroll, scroll) {
            self.dirty.mark_scroll_resources(
                id,
                self.collect_visible_scroll_resource_dirty(id, previous.offset, next.offset),
            );
        }

        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };
        node.scroll = scroll;
        self.dirty.mark_node(id);
        true
    }

    pub fn set_text(&mut self, id: NodeId, text: Option<TextNode>) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };

        if node.text == text {
            return false;
        }

        node.text = text;
        self.dirty.mark_node(id);
        true
    }

    pub fn set_ui_slot(&mut self, id: NodeId, ui_slot: Option<u16>) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };

        if node.ui_slot == ui_slot {
            return false;
        }

        node.ui_slot = ui_slot;
        self.dirty.mark_node(id);
        true
    }

    pub fn set_text_slot(&mut self, id: NodeId, text_slot: Option<u16>) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };

        if node.text_slot == text_slot {
            return false;
        }

        node.text_slot = text_slot;
        self.dirty.mark_node(id);
        true
    }

    pub fn set_text_role(&mut self, id: NodeId, role: Option<TextRole>) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };

        if node.text_role == role {
            return false;
        }

        node.text_role = role;
        self.dirty.mark_node(id);
        true
    }

    pub fn set_toggle_state(&mut self, id: NodeId, state: Option<ToggleState>) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };

        if node.toggle_state == state {
            return false;
        }

        node.toggle_state = state;
        self.dirty.mark_node(id);
        true
    }

    pub fn set_selection_state(&mut self, id: NodeId, state: Option<SelectionState>) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };

        if node.selection_state == state {
            return false;
        }

        node.selection_state = state;
        self.dirty.mark_node(id);
        true
    }

    pub fn mark_node_dirty(&mut self, id: NodeId) {
        if self.nodes.contains_key(&id) {
            self.dirty.mark_node(id);
        }
    }

    pub fn mark_full_scene_dirty(&mut self) {
        self.dirty.mark_full_scene();
        self.dirty.mark_node(self.root);
    }

    pub fn mark_layer_dirty(&mut self, id: LayerId) {
        self.dirty.mark_layer(id);
    }

    pub fn remove_subtree(&mut self, id: NodeId) -> Vec<NodeId> {
        if id == self.root || !self.nodes.contains_key(&id) {
            return Vec::new();
        }

        let parent = self.nodes.get(&id).and_then(|node| node.parent);
        let mut removed = Vec::new();
        self.collect_subtree_ids(id, &mut removed);

        if let Some(parent_id) = parent {
            if let Some(parent_node) = self.nodes.get_mut(&parent_id) {
                parent_node.children.retain(|child| *child != id);
            }
            self.dirty.mark_node(parent_id);
        }

        for removed_id in &removed {
            if let Some(node) = self.nodes.remove(removed_id) {
                if let Some(layer) = node.layer {
                    self.dirty.mark_layer(layer);
                }
                self.dirty.mark_node(*removed_id);
            }
        }

        removed
    }

    fn collect_subtree_ids(&self, id: NodeId, out: &mut Vec<NodeId>) {
        let Some(node) = self.nodes.get(&id) else {
            return;
        };

        out.push(id);
        for child in &node.children {
            self.collect_subtree_ids(*child, out);
        }
    }

    fn collect_descendants<'a>(&'a self, id: NodeId, out: &mut Vec<&'a RenderNode>) {
        let Some(node) = self.nodes.get(&id) else {
            return;
        };

        for child in &node.children {
            let Some(child_node) = self.nodes.get(child) else {
                continue;
            };
            out.push(child_node);
            self.collect_descendants(*child, out);
        }
    }

    fn sort_nodes_by_resolved_position(&self, nodes: &mut Vec<&RenderNode>) {
        nodes.sort_by(|a, b| {
            let a_bounds = self.resolved_bounds(a.id).unwrap_or(a.bounds);
            let b_bounds = self.resolved_bounds(b.id).unwrap_or(b.bounds);
            a_bounds
                .y
                .total_cmp(&b_bounds.y)
                .then_with(|| a_bounds.x.total_cmp(&b_bounds.x))
                .then_with(|| a.id.cmp(&b.id))
        });
    }

    fn collect_resource_dirty_for_nodes(
        &self,
        node_ids: impl IntoIterator<Item = NodeId>,
    ) -> ResourceDirtySet {
        let mut resources = ResourceDirtySet::default();

        for id in node_ids {
            let Some(node) = self.node(id) else {
                resources.full_text = true;
                resources.full_ui = true;
                continue;
            };

            if let Some(text_slot) = node.text_slot {
                resources.text_slots.insert(text_slot);
            }
            if let Some(ui_slot) = node.ui_slot {
                resources.ui_slots.insert(ui_slot);
            }
            if node.ui_visual_role.is_some() && node.ui_slot.is_none() {
                resources.full_ui = true;
            }
        }

        resources
    }

    fn collect_visible_scroll_resource_dirty(
        &self,
        scroll_root: NodeId,
        previous_offset: [f32; 2],
        next_offset: [f32; 2],
    ) -> ResourceDirtySet {
        let mut visible_ids = BTreeSet::new();
        visible_ids.extend(
            self.visible_descendants_in_scroll_root(scroll_root, previous_offset)
                .into_iter()
                .map(|node| node.id),
        );
        visible_ids.extend(
            self.visible_descendants_in_scroll_root(scroll_root, next_offset)
                .into_iter()
                .map(|node| node.id),
        );
        self.collect_resource_dirty_for_nodes(visible_ids)
    }

    pub fn resolved_bounds(&self, id: NodeId) -> Option<Rect> {
        self.resolved_bounds_with_scroll_override(id, None)
    }

    pub fn resolved_bounds_with_scroll_override(
        &self,
        id: NodeId,
        scroll_override: Option<(NodeId, [f32; 2])>,
    ) -> Option<Rect> {
        let mut bounds = self.node(id)?.bounds;
        let mut current = self.node(id)?.parent;

        while let Some(parent_id) = current {
            let parent = self.node(parent_id)?;
            if let Some(scroll) = parent.scroll {
                let offset = match scroll_override {
                    Some((override_id, override_offset)) if override_id == parent_id => {
                        override_offset
                    }
                    _ => scroll.offset,
                };
                bounds = bounds.translate(-offset[0], -offset[1]);
            }
            current = parent.parent;
        }

        Some(bounds)
    }

    pub fn clip_bounds(&self, id: NodeId) -> Option<Rect> {
        self.clip_bounds_with_scroll_override(id, None)
    }

    pub fn clip_bounds_with_scroll_override(
        &self,
        id: NodeId,
        scroll_override: Option<(NodeId, [f32; 2])>,
    ) -> Option<Rect> {
        let mut current = Some(id);
        let mut clip_bounds: Option<Rect> = None;

        while let Some(node_id) = current {
            let node = self.node(node_id)?;
            if node.clip {
                let bounds = self.resolved_bounds_with_scroll_override(node_id, scroll_override)?;
                clip_bounds = match clip_bounds {
                    Some(existing) => existing.intersect(bounds),
                    None => Some(bounds),
                };
            }
            current = node.parent;
        }

        clip_bounds
    }

    pub fn is_rect_visible_for_node(&self, id: NodeId, bounds: Rect) -> bool {
        self.is_rect_visible_for_node_with_scroll_override(id, bounds, None)
    }

    pub fn is_rect_visible_for_node_with_scroll_override(
        &self,
        id: NodeId,
        bounds: Rect,
        scroll_override: Option<(NodeId, [f32; 2])>,
    ) -> bool {
        match self.clip_bounds_with_scroll_override(id, scroll_override) {
            Some(clip) => clip.intersects(bounds),
            None => true,
        }
    }

    pub fn is_node_visible(&self, id: NodeId) -> bool {
        self.is_node_visible_with_scroll_override(id, None)
    }

    pub fn is_node_visible_with_scroll_override(
        &self,
        id: NodeId,
        scroll_override: Option<(NodeId, [f32; 2])>,
    ) -> bool {
        let Some(bounds) = self.resolved_bounds_with_scroll_override(id, scroll_override) else {
            return false;
        };
        self.is_rect_visible_for_node_with_scroll_override(id, bounds, scroll_override)
    }

    pub fn descendants_with_element_visible_in_scroll_root(
        &self,
        scroll_root: NodeId,
        element: ElementKind,
        offset: [f32; 2],
    ) -> Vec<&RenderNode> {
        self.descendants_with_element(scroll_root, element)
            .into_iter()
            .filter(|node| {
                self.is_node_visible_with_scroll_override(node.id, Some((scroll_root, offset)))
            })
            .collect()
    }

    pub fn visible_descendants_in_scroll_root(
        &self,
        scroll_root: NodeId,
        offset: [f32; 2],
    ) -> Vec<&RenderNode> {
        self.descendants_of(scroll_root)
            .into_iter()
            .filter(|node| {
                self.is_node_visible_with_scroll_override(node.id, Some((scroll_root, offset)))
            })
            .collect()
    }

    pub fn descendants_with_element_visible_in_scroll_root_sorted_by_resolved_position(
        &self,
        scroll_root: NodeId,
        element: ElementKind,
        offset: [f32; 2],
    ) -> Vec<&RenderNode> {
        let mut nodes =
            self.descendants_with_element_visible_in_scroll_root(scroll_root, element, offset);
        nodes.sort_by(|a, b| {
            let a_bounds = self
                .resolved_bounds_with_scroll_override(a.id, Some((scroll_root, offset)))
                .unwrap_or(a.bounds);
            let b_bounds = self
                .resolved_bounds_with_scroll_override(b.id, Some((scroll_root, offset)))
                .unwrap_or(b.bounds);
            a_bounds
                .y
                .total_cmp(&b_bounds.y)
                .then_with(|| a_bounds.x.total_cmp(&b_bounds.x))
        });
        nodes
    }

    pub fn visible_descendants_in_scroll_root_sorted_by_resolved_position(
        &self,
        scroll_root: NodeId,
        offset: [f32; 2],
    ) -> Vec<&RenderNode> {
        let mut nodes = self.visible_descendants_in_scroll_root(scroll_root, offset);
        nodes.sort_by(|a, b| {
            let a_bounds = self
                .resolved_bounds_with_scroll_override(a.id, Some((scroll_root, offset)))
                .unwrap_or(a.bounds);
            let b_bounds = self
                .resolved_bounds_with_scroll_override(b.id, Some((scroll_root, offset)))
                .unwrap_or(b.bounds);
            a_bounds
                .y
                .total_cmp(&b_bounds.y)
                .then_with(|| a_bounds.x.total_cmp(&b_bounds.x))
        });
        nodes
    }
}

fn clamp_named_scroll_offset(scroll: ScrollState, y: f32) -> f32 {
    let max_offset = (scroll.content_size[1] - scroll.viewport_size[1]).max(0.0);
    y.clamp(0.0, max_offset)
}

pub fn scroll_offset_for_node(scene: &RetainedScene, scroll_name: &str) -> Option<f32> {
    scene
        .node_named(scroll_name)
        .and_then(|node| node.scroll)
        .map(|scroll| scroll.offset[1])
}

pub fn set_named_scroll_offset(
    scene: &mut RetainedScene,
    scroll_name: &str,
    offset_y: f32,
) -> bool {
    let Some(scroll_root) = scene.node_named(scroll_name).map(|node| node.id) else {
        return false;
    };
    let Some(mut scroll) = scene.node(scroll_root).and_then(|node| node.scroll) else {
        return false;
    };
    let clamped = clamp_named_scroll_offset(scroll, offset_y);
    if (scroll.offset[1] - clamped).abs() <= f32::EPSILON {
        return false;
    }
    scroll.offset[1] = clamped;
    scene.set_scroll_state(scroll_root, Some(scroll))
}

#[cfg(test)]
mod tests {
    use super::{
        ElementKind, LayerId, Rect, RenderNodeDescriptor, RenderNodeKind, RetainedScene, SceneMode,
        ScrollState, SelectionState, TextRole, ToggleState, UiSlotRange, UiVisualRole,
        UiVisualStyle,
    };
    use std::collections::BTreeSet;

    #[test]
    fn new_scene_starts_with_dirty_root() {
        let scene = RetainedScene::new(SceneMode::Ui2D);

        assert_eq!(scene.root().get(), 1);
        assert!(scene.dirty().full_scene);
        assert!(scene.dirty().nodes.contains(&scene.root()));
        assert_eq!(scene.node(scene.root()).unwrap().element, ElementKind::Root);
    }

    #[test]
    fn append_child_marks_parent_and_child_dirty() {
        let mut scene = RetainedScene::new(SceneMode::UiPhysical);
        scene.clear_dirty();

        let child = scene
            .append_node(
                scene.root(),
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Panel,
                    Rect::new(1.0, 2.0, 3.0, 4.0),
                )
                .named("card")
                .with_material(0.0, 6.0, 12.0),
            )
            .expect("root exists");

        assert_eq!(child.get(), 2);
        assert!(scene.dirty().nodes.contains(&scene.root()));
        assert!(scene.dirty().nodes.contains(&child));
        assert_eq!(
            scene.node(child).unwrap().bounds,
            Rect::new(1.0, 2.0, 3.0, 4.0)
        );
        assert_eq!(scene.node(child).unwrap().element, ElementKind::Panel);
        assert_eq!(scene.node(child).unwrap().corner_radius, 12.0);
    }

    #[test]
    fn remove_subtree_removes_descendants_and_marks_layers_dirty() {
        let mut scene = RetainedScene::new(SceneMode::Ui2D);
        scene.clear_dirty();

        let parent = scene
            .append_child(scene.root(), RenderNodeKind::Group, Rect::default())
            .unwrap();
        let child = scene
            .append_child(parent, RenderNodeKind::Primitive, Rect::default())
            .unwrap();
        assert!(scene.set_layer(child, Some(LayerId::new(7))));

        scene.clear_dirty();
        let removed = scene.remove_subtree(parent);

        assert_eq!(removed, vec![parent, child]);
        assert!(scene.node(parent).is_none());
        assert!(scene.node(child).is_none());
        assert!(scene.dirty().nodes.contains(&scene.root()));
        assert!(scene.dirty().layers.contains(&LayerId::new(7)));
    }

    #[test]
    fn updating_scroll_state_marks_scroll_root_dirty() {
        let mut scene = RetainedScene::new(SceneMode::Ui2D);
        let scroll_root = scene
            .append_node(
                scene.root(),
                RenderNodeDescriptor::new(
                    RenderNodeKind::ScrollRoot,
                    ElementKind::ScrollContainer,
                    Rect::new(0.0, 0.0, 300.0, 120.0),
                )
                .with_scroll(ScrollState::new([300.0, 120.0], [300.0, 800.0])),
            )
            .unwrap();

        scene.clear_dirty();
        assert!(scene.set_scroll_state(
            scroll_root,
            Some(ScrollState {
                offset: [0.0, 128.0],
                viewport_size: [300.0, 120.0],
                content_size: [300.0, 800.0],
            }),
        ));
        assert!(scene.dirty().nodes.contains(&scroll_root));
    }

    #[test]
    fn updating_semantic_state_marks_node_dirty() {
        let mut scene = RetainedScene::new(SceneMode::Ui2D);
        let text = scene
            .append_node(
                scene.root(),
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Text,
                    Rect::new(0.0, 0.0, 10.0, 10.0),
                ),
            )
            .unwrap();

        scene.clear_dirty();
        assert!(scene.set_text_role(text, Some(TextRole::Body)));
        assert!(scene.set_toggle_state(text, Some(ToggleState::On)));
        assert!(scene.set_selection_state(text, Some(SelectionState::Selected)));
        assert!(scene.dirty().nodes.contains(&text));
    }

    #[test]
    fn updating_resource_slots_marks_node_dirty() {
        let mut scene = RetainedScene::new(SceneMode::Ui2D);
        let node = scene
            .append_node(
                scene.root(),
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Text,
                    Rect::new(0.0, 0.0, 10.0, 10.0),
                )
                .with_text("slot me", 12.0),
            )
            .unwrap();

        scene.clear_dirty();
        assert!(scene.set_text_slot(node, Some(2)));
        assert!(scene.set_ui_slot(node, Some(4)));
        assert_eq!(scene.node(node).and_then(|entry| entry.text_slot), Some(2));
        assert_eq!(scene.node(node).and_then(|entry| entry.ui_slot), Some(4));
        assert!(scene.dirty().nodes.contains(&node));
    }

    #[test]
    fn resource_dirty_classifies_text_and_ui_slots() {
        let mut scene = RetainedScene::new(SceneMode::Ui2D);
        let root = scene.root();
        let text = scene
            .append_node(
                root,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Text,
                    Rect::new(10.0, 10.0, 120.0, 20.0),
                )
                .with_text_slot(3)
                .with_text("hello", 14.0),
            )
            .unwrap();
        let checkbox = scene
            .append_node(
                root,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Checkbox,
                    Rect::new(10.0, 40.0, 32.0, 32.0),
                )
                .with_ui_slot(7)
                .with_ui_primitive_count(2)
                .with_ui_visual_role(UiVisualRole::CheckboxControl),
            )
            .unwrap();

        scene.clear_dirty();
        scene.mark_node_dirty(text);
        scene.mark_node_dirty(checkbox);

        let resources = scene.classify_resource_dirty(scene.dirty());
        assert!(!resources.full_text);
        assert!(!resources.full_ui);
        assert_eq!(resources.text_slots, BTreeSet::from([3]));
        assert_eq!(resources.ui_slots, BTreeSet::from([7]));
    }

    #[test]
    fn resource_dirty_expands_scroll_descendants_and_static_ui() {
        let mut scene = RetainedScene::new(SceneMode::Ui2D);
        let root = scene.root();
        let clip = scene
            .append_node(
                root,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Clip,
                    ElementKind::Clip,
                    Rect::new(0.0, 0.0, 200.0, 100.0),
                )
                .with_clip()
                .with_ui_visual_role(UiVisualRole::OutlineRect),
            )
            .unwrap();
        let scroll = scene
            .append_node(
                clip,
                RenderNodeDescriptor::new(
                    RenderNodeKind::ScrollRoot,
                    ElementKind::ScrollContainer,
                    Rect::new(0.0, 0.0, 200.0, 100.0),
                )
                .with_scroll(ScrollState::new([200.0, 100.0], [200.0, 240.0])),
            )
            .unwrap();
        scene
            .append_node(
                scroll,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Text,
                    Rect::new(10.0, 10.0, 120.0, 20.0),
                )
                .with_text_slot(1)
                .with_text("row", 14.0),
            )
            .unwrap();
        scene
            .append_node(
                scroll,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Checkbox,
                    Rect::new(10.0, 40.0, 32.0, 32.0),
                )
                .with_ui_slot(2)
                .with_ui_primitive_count(2)
                .with_ui_visual_role(UiVisualRole::CheckboxControl),
            )
            .unwrap();

        scene.clear_dirty();
        scene.mark_node_dirty(scroll);

        let resources = scene.classify_resource_dirty(scene.dirty());
        assert!(!resources.full_text);
        assert!(!resources.full_ui);
        assert_eq!(resources.text_slots, BTreeSet::from([1]));
        assert_eq!(resources.ui_slots, BTreeSet::from([2]));
    }

    #[test]
    fn scroll_updates_only_dirty_old_and_new_visible_resources() {
        let mut scene = RetainedScene::new(SceneMode::Ui2D);
        let root = scene.root();
        let clip = scene
            .append_node(
                root,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Clip,
                    ElementKind::Clip,
                    Rect::new(0.0, 0.0, 200.0, 60.0),
                )
                .with_clip(),
            )
            .unwrap();
        let scroll = scene
            .append_node(
                clip,
                RenderNodeDescriptor::new(
                    RenderNodeKind::ScrollRoot,
                    ElementKind::ScrollContainer,
                    Rect::new(0.0, 0.0, 200.0, 60.0),
                )
                .with_scroll(ScrollState::new([200.0, 60.0], [200.0, 160.0])),
            )
            .unwrap();

        for (index, y) in [0.0, 28.0, 76.0, 116.0].into_iter().enumerate() {
            scene
                .append_node(
                    scroll,
                    RenderNodeDescriptor::new(
                        RenderNodeKind::Primitive,
                        ElementKind::Text,
                        Rect::new(8.0, y, 80.0, 18.0),
                    )
                    .with_text_slot(index as u16)
                    .with_text(format!("row {index}"), 14.0),
                )
                .unwrap();
        }
        scene
            .append_node(
                scroll,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Checkbox,
                    Rect::new(120.0, 28.0, 24.0, 24.0),
                )
                .with_ui_slot(0)
                .with_ui_primitive_count(2)
                .with_ui_visual_role(UiVisualRole::CheckboxControl),
            )
            .unwrap();
        scene
            .append_node(
                scroll,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Checkbox,
                    Rect::new(120.0, 116.0, 24.0, 24.0),
                )
                .with_ui_slot(1)
                .with_ui_primitive_count(2)
                .with_ui_visual_role(UiVisualRole::CheckboxControl),
            )
            .unwrap();

        scene.clear_dirty();
        assert!(scene.set_scroll_state(
            scroll,
            Some(ScrollState {
                offset: [0.0, 70.0],
                viewport_size: [200.0, 60.0],
                content_size: [200.0, 160.0],
            }),
        ));

        let resources = scene.classify_resource_dirty(scene.dirty());
        assert_eq!(resources.text_slots, BTreeSet::from([0, 1, 2, 3]));
        assert_eq!(resources.ui_slots, BTreeSet::from([0, 1]));

        scene.clear_dirty();
        assert!(scene.set_scroll_state(
            scroll,
            Some(ScrollState {
                offset: [0.0, 38.0],
                viewport_size: [200.0, 60.0],
                content_size: [200.0, 160.0],
            }),
        ));

        let resources = scene.classify_resource_dirty(scene.dirty());
        assert_eq!(resources.text_slots, BTreeSet::from([1, 2, 3]));
        assert_eq!(resources.ui_slots, BTreeSet::from([0, 1]));
    }

    #[test]
    fn element_queries_return_matching_nodes() {
        let mut scene = RetainedScene::new(SceneMode::Ui2D);
        let panel = scene
            .append_node(
                scene.root(),
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Panel,
                    Rect::new(0.0, 0.0, 100.0, 50.0),
                ),
            )
            .unwrap();
        let checkbox = scene
            .append_node(
                panel,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Checkbox,
                    Rect::new(0.0, 0.0, 10.0, 10.0),
                ),
            )
            .unwrap();
        let text = scene
            .append_node(
                panel,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Text,
                    Rect::new(10.0, 0.0, 40.0, 10.0),
                )
                .named("body")
                .with_key("body_text")
                .with_ui_slot(3)
                .with_ui_primitive_count(2)
                .with_text_slot(7)
                .with_text("hello", 12.0),
            )
            .unwrap();

        let children = scene.children_of(panel);
        assert_eq!(children.len(), 2);
        assert_eq!(
            scene
                .first_child_with_element(panel, ElementKind::Checkbox)
                .map(|node| node.id),
            Some(checkbox)
        );
        assert_eq!(
            scene
                .children_with_element(panel, ElementKind::Text)
                .into_iter()
                .map(|node| node.id)
                .collect::<Vec<_>>(),
            vec![text]
        );
        assert_eq!(
            scene
                .nodes_with_element(ElementKind::Panel)
                .into_iter()
                .map(|node| node.id)
                .collect::<Vec<_>>(),
            vec![panel]
        );
        assert_eq!(scene.node_named("body").map(|node| node.id), Some(text));
        assert_eq!(
            scene.node_keyed("body_text").map(|node| node.id),
            Some(text)
        );
        assert_eq!(scene.node_with_ui_slot(3).map(|node| node.id), Some(text));
        assert_eq!(
            scene
                .node_with_ui_slot(3)
                .map(|node| node.ui_primitive_count),
            Some(2)
        );
        assert_eq!(scene.node_with_text_slot(7).map(|node| node.id), Some(text));
        assert_eq!(
            scene
                .nodes_with_ui_slots_sorted()
                .into_iter()
                .map(|node| node.id)
                .collect::<Vec<_>>(),
            vec![text]
        );
        assert_eq!(scene.total_ui_primitive_count(5), 7);
        assert_eq!(
            scene.ui_slot_range(3, 5),
            Some(UiSlotRange {
                node_id: text,
                ui_slot: 3,
                offset: 5,
                primitive_count: 2,
            })
        );
        assert_eq!(
            scene
                .descendants_of(scene.root())
                .into_iter()
                .map(|node| node.id)
                .collect::<Vec<_>>(),
            vec![panel, checkbox, text]
        );
        assert_eq!(
            scene
                .descendants_with_element(panel, ElementKind::Text)
                .into_iter()
                .map(|node| node.id)
                .collect::<Vec<_>>(),
            vec![text]
        );
        assert_eq!(
            scene
                .descendants_with_text(panel)
                .into_iter()
                .map(|node| node.id)
                .collect::<Vec<_>>(),
            vec![text]
        );
        assert_eq!(
            scene
                .descendants_with_element_sorted_by_resolved_position(
                    scene.root(),
                    ElementKind::Text,
                )
                .into_iter()
                .map(|node| node.id)
                .collect::<Vec<_>>(),
            vec![text]
        );
    }

    #[test]
    fn visual_role_queries_return_matching_children() {
        let mut scene = RetainedScene::new(SceneMode::Ui2D);
        let panel = scene
            .append_node(
                scene.root(),
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Panel,
                    Rect::new(0.0, 0.0, 100.0, 50.0),
                ),
            )
            .unwrap();
        let text = scene
            .append_node(
                panel,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Text,
                    Rect::new(10.0, 0.0, 40.0, 10.0),
                )
                .with_ui_visual_role(UiVisualRole::CompletedTextDecoration)
                .with_ui_visual_style(UiVisualStyle {
                    base_color: [1.0, 0.0, 0.0, 1.0],
                    accent_color: [0.0; 4],
                    detail_color: [0.0; 4],
                    stroke_width: 2.0,
                    corner_radius: 0.0,
                    offset: [0.0; 2],
                    extra_size: [0.0; 2],
                }),
            )
            .unwrap();

        assert_eq!(
            scene
                .first_child_with_ui_visual_role(panel, UiVisualRole::CompletedTextDecoration)
                .map(|node| node.id),
            Some(text)
        );
        assert_eq!(
            scene
                .node(text)
                .and_then(|node| node.ui_visual_style)
                .map(|style| style.stroke_width),
            Some(2.0)
        );
    }

    #[test]
    fn resolved_bounds_apply_scroll_ancestors() {
        let mut scene = RetainedScene::new(SceneMode::Ui2D);
        let viewport = scene
            .append_node(
                scene.root(),
                RenderNodeDescriptor::new(
                    RenderNodeKind::Viewport,
                    ElementKind::Viewport,
                    Rect::new(0.0, 0.0, 400.0, 300.0),
                ),
            )
            .unwrap();
        let clip = scene
            .append_node(
                viewport,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Clip,
                    ElementKind::Clip,
                    Rect::new(20.0, 30.0, 100.0, 80.0),
                )
                .with_clip(),
            )
            .unwrap();
        let scroll = scene
            .append_node(
                clip,
                RenderNodeDescriptor::new(
                    RenderNodeKind::ScrollRoot,
                    ElementKind::ScrollContainer,
                    Rect::new(20.0, 30.0, 100.0, 80.0),
                )
                .with_scroll(ScrollState {
                    offset: [0.0, 40.0],
                    viewport_size: [100.0, 80.0],
                    content_size: [100.0, 160.0],
                }),
            )
            .unwrap();
        let child = scene
            .append_node(
                scroll,
                RenderNodeDescriptor::new(
                    RenderNodeKind::Primitive,
                    ElementKind::Text,
                    Rect::new(20.0, 90.0, 40.0, 20.0),
                )
                .with_text("visible", 12.0),
            )
            .unwrap();

        assert_eq!(
            scene.resolved_bounds(child),
            Some(Rect::new(20.0, 50.0, 40.0, 20.0))
        );
        assert_eq!(
            scene.clip_bounds(child),
            Some(Rect::new(20.0, 30.0, 100.0, 80.0))
        );
        assert!(scene.is_node_visible(child));
        assert_eq!(
            scene
                .visible_descendants_of(viewport)
                .into_iter()
                .map(|node| node.id)
                .collect::<Vec<_>>(),
            vec![clip, scroll, child]
        );
        assert_eq!(
            scene
                .visible_descendants_with_text(viewport)
                .into_iter()
                .map(|node| node.id)
                .collect::<Vec<_>>(),
            vec![child]
        );
        assert_eq!(
            scene
                .visible_descendants_with_text_sorted_by_resolved_position(viewport)
                .into_iter()
                .map(|node| node.id)
                .collect::<Vec<_>>(),
            vec![child]
        );
        assert_eq!(
            scene
                .resolved_bounds_with_scroll_override(child, Some((scroll, [0.0, 10.0])))
                .expect("resolved bounds with override"),
            Rect::new(20.0, 80.0, 40.0, 20.0)
        );
        assert_eq!(
            scene
                .descendants_with_element_visible_in_scroll_root_sorted_by_resolved_position(
                    scroll,
                    ElementKind::Text,
                    [0.0, 10.0],
                )
                .into_iter()
                .map(|node| node.id)
                .collect::<Vec<_>>(),
            vec![child]
        );
        assert!(!scene.is_rect_visible_for_node(child, Rect::new(20.0, 200.0, 40.0, 20.0)));
    }
}
