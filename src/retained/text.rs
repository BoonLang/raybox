use super::{ElementKind, NodeId, RetainedScene, TextRole};
use crate::text::{
    build_fixed_char_grid, fixed_char_grid_cells_for_instance, CharGridCell, FixedCharGridSpec,
    VectorFontAtlas,
};
use std::collections::BTreeSet;

pub const ROLE_ACTIVE: f32 = 0.0;
pub const ROLE_COMPLETED: f32 = 1.0;
pub const ROLE_HEADING: f32 = 2.0;
pub const ROLE_PLACEHOLDER: f32 = 3.0;
pub const ROLE_BODY: f32 = 4.0;
pub const ROLE_INFO: f32 = 5.0;

pub type GpuCharInstanceEx = crate::ui2d_shader_bindings::CharInstanceEx_std430_0;

pub const fn gpu_char_instance_ex(
    pos_and_char: [f32; 4],
    color_flags: [f32; 4],
) -> GpuCharInstanceEx {
    GpuCharInstanceEx::new(pos_and_char, color_flags)
}

#[derive(Clone, Copy)]
pub struct TextColors {
    pub heading: [f32; 3],
    pub active: [f32; 3],
    pub completed: [f32; 3],
    pub placeholder: [f32; 3],
    pub body: [f32; 3],
    pub info: [f32; 3],
}

#[derive(Clone, Copy)]
pub struct TextRenderSpace {
    pub x_offset: f32,
    pub screen_height: f32,
    pub italic_codepoint_offset: u32,
}

fn baseline_y(screen_height: f32, text_top_y: f32, font_size: f32) -> f32 {
    let ascent_ratio = 0.905;
    let metadata_baseline = text_top_y + ascent_ratio * font_size;
    screen_height - metadata_baseline
}

fn text_style_for_node(
    element: ElementKind,
    text_role: Option<TextRole>,
    colors: &TextColors,
    italic_codepoint_offset: u32,
) -> ([f32; 3], f32, u32) {
    match text_role {
        Some(TextRole::Heading) => (colors.heading, ROLE_HEADING, 0),
        Some(TextRole::Placeholder) => (
            colors.placeholder,
            ROLE_PLACEHOLDER,
            italic_codepoint_offset,
        ),
        Some(TextRole::Info) => (colors.info, ROLE_INFO, 0),
        Some(TextRole::Completed) => (colors.completed, ROLE_COMPLETED, 0),
        Some(TextRole::Active) => (colors.active, ROLE_ACTIVE, 0),
        Some(TextRole::Body) | None if element == ElementKind::Button => {
            (colors.body, ROLE_BODY, 0)
        }
        Some(TextRole::Body) | None => (colors.body, ROLE_BODY, 0),
    }
}

fn push_text_instances(
    instances: &mut Vec<GpuCharInstanceEx>,
    text: &str,
    x: f32,
    baseline_y: f32,
    font_size: f32,
    color: [f32; 3],
    flags: f32,
    atlas: &VectorFontAtlas,
    codepoint_offset: u32,
) {
    let mut cx = x;
    for ch in text.chars() {
        if ch == ' ' {
            let adv = atlas
                .glyphs
                .get(&(' ' as u32 + codepoint_offset))
                .or_else(|| atlas.glyphs.get(&(' ' as u32)))
                .map(|e| e.advance)
                .unwrap_or(0.25);
            cx += adv * font_size;
            continue;
        }

        let codepoint = ch as u32 + codepoint_offset;
        if let Some(idx) = atlas.glyph_list.iter().position(|(cp, _)| *cp == codepoint) {
            let entry = &atlas.glyph_list[idx].1;
            instances.push(gpu_char_instance_ex(
                [cx, baseline_y, font_size, idx as f32],
                [color[0], color[1], color[2], flags],
            ));
            cx += entry.advance * font_size;
        }
    }
}

pub fn build_text_instances_for_node(
    scene: &RetainedScene,
    node_id: NodeId,
    atlas: &VectorFontAtlas,
    colors: &TextColors,
    space: TextRenderSpace,
) -> Vec<GpuCharInstanceEx> {
    let Some(node) = scene.node(node_id) else {
        return Vec::new();
    };
    let Some(text) = node.text.as_ref() else {
        return Vec::new();
    };

    let bounds = scene
        .resolved_bounds(node.id)
        .expect("resolved text node bounds");
    if !scene.is_rect_visible_for_node(node.id, bounds) {
        return Vec::new();
    }

    let (color, flags, codepoint_offset) = text_style_for_node(
        node.element,
        node.text_role,
        colors,
        space.italic_codepoint_offset,
    );
    let mut instances = Vec::new();
    push_text_instances(
        &mut instances,
        text.text.as_ref(),
        bounds.x + space.x_offset,
        baseline_y(space.screen_height, bounds.y, text.font_size),
        text.font_size,
        color,
        flags,
        atlas,
        codepoint_offset,
    );
    instances
}

pub fn build_text_instances_from_scene(
    scene: &RetainedScene,
    atlas: &VectorFontAtlas,
    colors: &TextColors,
    space: TextRenderSpace,
) -> Vec<GpuCharInstanceEx> {
    let mut instances = Vec::new();

    for node in scene.visible_descendants_with_text_sorted_by_resolved_position(scene.root()) {
        let Some(text) = node.text.as_ref() else {
            continue;
        };

        let bounds = scene
            .resolved_bounds(node.id)
            .expect("resolved text node bounds");
        if !scene.is_rect_visible_for_node(node.id, bounds) {
            continue;
        }

        let (color, flags, codepoint_offset) = text_style_for_node(
            node.element,
            node.text_role,
            colors,
            space.italic_codepoint_offset,
        );
        push_text_instances(
            &mut instances,
            text.text.as_ref(),
            bounds.x + space.x_offset,
            baseline_y(space.screen_height, bounds.y, text.font_size),
            text.font_size,
            color,
            flags,
            atlas,
            codepoint_offset,
        );
    }

    instances
}

pub fn build_text_instances_for_text_slot(
    scene: &RetainedScene,
    text_slot: u16,
    atlas: &VectorFontAtlas,
    colors: &TextColors,
    space: TextRenderSpace,
) -> Vec<GpuCharInstanceEx> {
    let Some(node) = scene.node_with_text_slot(text_slot) else {
        return Vec::new();
    };
    build_text_instances_for_node(scene, node.id, atlas, colors, space)
}

pub fn count_active_char_instances(
    instances: &[GpuCharInstanceEx],
    atlas: &VectorFontAtlas,
) -> u32 {
    instances
        .iter()
        .filter(|instance| instance.posAndChar_0[3] < atlas.glyph_list.len() as f32)
        .count() as u32
}

pub fn inactive_char_instance(atlas: &VectorFontAtlas) -> GpuCharInstanceEx {
    gpu_char_instance_ex([0.0, 0.0, 0.0, atlas.glyph_list.len() as f32], [0.0; 4])
}

pub fn build_fixed_text_run_slot_buffer(
    instances: Vec<GpuCharInstanceEx>,
    capacity: usize,
    atlas: &VectorFontAtlas,
) -> (Vec<GpuCharInstanceEx>, u32) {
    assert!(instances.len() <= capacity);
    let mut slots = vec![inactive_char_instance(atlas); capacity];
    slots[..instances.len()].copy_from_slice(&instances);
    (slots, instances.len() as u32)
}

#[derive(Clone, Copy)]
pub struct FixedTextRunLayout<'a> {
    pub run_capacities: &'a [usize],
    pub grid_spec: FixedCharGridSpec,
}

impl<'a> FixedTextRunLayout<'a> {
    pub fn run_capacity(self, run_index: usize) -> usize {
        self.run_capacities[run_index]
    }

    pub fn run_offset(self, run_index: usize) -> usize {
        self.run_capacities[..run_index].iter().copied().sum()
    }

    pub fn total_capacity(self) -> usize {
        self.run_capacities.iter().copied().sum()
    }

    pub fn grid_params(self) -> [f32; 4] {
        [
            self.grid_spec.dims[0] as f32,
            self.grid_spec.dims[1] as f32,
            (self.grid_spec.bounds[2] - self.grid_spec.bounds[0]) / self.grid_spec.dims[0] as f32,
            (self.grid_spec.bounds[3] - self.grid_spec.bounds[1]) / self.grid_spec.dims[1] as f32,
        ]
    }

    pub fn grid_cell_count(self) -> usize {
        self.grid_spec.dims[0] as usize * self.grid_spec.dims[1] as usize
    }

    pub fn grid_index_capacity(self) -> usize {
        self.grid_cell_count() * self.grid_spec.cell_capacity
    }
}

#[derive(Debug, Clone)]
pub struct OwnedTextRunLayout {
    run_capacities: Vec<usize>,
    grid_spec: FixedCharGridSpec,
}

impl OwnedTextRunLayout {
    pub fn new(run_capacities: Vec<usize>, grid_spec: FixedCharGridSpec) -> Self {
        Self {
            run_capacities,
            grid_spec,
        }
    }

    pub fn layout(&self) -> FixedTextRunLayout<'_> {
        FixedTextRunLayout {
            run_capacities: &self.run_capacities,
            grid_spec: self.grid_spec,
        }
    }

    pub fn run_capacities(&self) -> &[usize] {
        &self.run_capacities
    }

    pub fn grid_spec(&self) -> FixedCharGridSpec {
        self.grid_spec
    }
}

pub fn assign_text_slots_and_build_layout(
    scene: &mut RetainedScene,
    grid_spec: FixedCharGridSpec,
    padding: usize,
) -> OwnedTextRunLayout {
    let stale_slots = scene
        .nodes()
        .values()
        .filter(|node| node.text.is_none() && node.text_slot.is_some())
        .map(|node| node.id)
        .collect::<Vec<_>>();
    for id in stale_slots {
        let _ = scene.set_text_slot(id, None);
    }

    let text_nodes = scene
        .descendants_sorted_by_resolved_position(scene.root())
        .into_iter()
        .filter_map(|node| {
            node.text
                .as_ref()
                .map(|text| (node.id, text.text.chars().count() + padding))
        })
        .collect::<Vec<_>>();

    let mut run_capacities = Vec::with_capacity(text_nodes.len());
    for (slot_index, (id, capacity)) in text_nodes.into_iter().enumerate() {
        let text_slot = u16::try_from(slot_index).expect("text slot overflow");
        let _ = scene.set_text_slot(id, Some(text_slot));
        run_capacities.push(capacity.max(1));
    }

    OwnedTextRunLayout::new(run_capacities, grid_spec)
}

pub struct FixedTextSceneData {
    pub char_instances: Vec<GpuCharInstanceEx>,
    pub char_count: u32,
    pub char_grid_params: [f32; 4],
    pub char_grid_bounds: [f32; 4],
    pub char_grid_cells: Vec<CharGridCell>,
    pub char_grid_indices: Vec<u32>,
}

pub struct FixedTextScenePatch {
    pub run_updates: Vec<(usize, Vec<GpuCharInstanceEx>)>,
    pub char_count: u32,
    pub changed_cells: Vec<usize>,
}

pub enum FixedTextRuntimeUpdate {
    Full(FixedTextSceneData),
    Partial(FixedTextScenePatch),
}

pub struct FixedTextSceneState {
    run_capacities: Vec<usize>,
    grid_spec: FixedCharGridSpec,
    pub char_instances: Vec<GpuCharInstanceEx>,
    pub text_grid: FixedTextGridCache,
    pub run_counts: Vec<u32>,
}

pub fn build_fixed_text_scene_data<F>(
    layout: FixedTextRunLayout<'_>,
    atlas: &VectorFontAtlas,
    mut build_run_instances: F,
) -> FixedTextSceneData
where
    F: FnMut(usize) -> Vec<GpuCharInstanceEx>,
{
    let mut char_instances = vec![inactive_char_instance(atlas); layout.total_capacity()];
    let mut char_count = 0u32;

    for run_index in 0..layout.run_capacities.len() {
        let instances = build_run_instances(run_index);
        let (run_slots, run_count) =
            build_fixed_text_run_slot_buffer(instances, layout.run_capacity(run_index), atlas);
        let offset = layout.run_offset(run_index);
        char_instances[offset..offset + run_slots.len()].copy_from_slice(&run_slots);
        char_count += run_count;
    }

    let text_grid = FixedTextGridCache::new(&char_instances, atlas, layout);

    FixedTextSceneData {
        char_instances,
        char_count,
        char_grid_params: layout.grid_params(),
        char_grid_bounds: layout.grid_spec.bounds,
        char_grid_cells: text_grid.cells.clone(),
        char_grid_indices: text_grid.indices.clone(),
    }
}

impl FixedTextSceneState {
    fn run_capacity(&self, run_index: usize) -> usize {
        self.run_capacities[run_index]
    }

    fn run_offset(&self, run_index: usize) -> usize {
        self.run_capacities[..run_index].iter().copied().sum()
    }

    pub fn new<F>(
        layout: FixedTextRunLayout<'_>,
        atlas: &VectorFontAtlas,
        build_run_instances: F,
    ) -> (Self, FixedTextSceneData)
    where
        F: FnMut(usize) -> Vec<GpuCharInstanceEx>,
    {
        let (text_data, text_grid, run_counts) =
            build_fixed_text_scene_state_data(layout, atlas, build_run_instances);
        let state = Self {
            run_capacities: layout.run_capacities.to_vec(),
            grid_spec: layout.grid_spec,
            char_instances: text_data.char_instances.clone(),
            text_grid,
            run_counts,
        };
        (state, text_data)
    }

    pub fn layout(&self) -> FixedTextRunLayout<'_> {
        FixedTextRunLayout {
            run_capacities: &self.run_capacities,
            grid_spec: self.grid_spec,
        }
    }

    pub fn grid_cell_capacity(&self) -> usize {
        self.grid_spec.cell_capacity
    }

    pub fn rebuild<F>(
        &mut self,
        atlas: &VectorFontAtlas,
        build_run_instances: F,
    ) -> FixedTextSceneData
    where
        F: FnMut(usize) -> Vec<GpuCharInstanceEx>,
    {
        let (text_data, text_grid, run_counts) =
            build_fixed_text_scene_state_data(self.layout(), atlas, build_run_instances);
        self.char_instances = text_data.char_instances.clone();
        self.text_grid = text_grid;
        self.run_counts = run_counts;
        text_data
    }

    pub fn update_runs<F>(
        &mut self,
        atlas: &VectorFontAtlas,
        run_indices: impl IntoIterator<Item = usize>,
        mut build_run_instances: F,
    ) -> FixedTextScenePatch
    where
        F: FnMut(usize) -> Vec<GpuCharInstanceEx>,
    {
        let mut run_updates = Vec::new();
        let mut changed_cells = BTreeSet::new();

        for run_index in run_indices {
            let instances = build_run_instances(run_index);
            let (run_slots, run_count) =
                build_fixed_text_run_slot_buffer(instances, self.run_capacity(run_index), atlas);
            let offset = self.run_offset(run_index);
            self.char_instances[offset..offset + run_slots.len()].copy_from_slice(&run_slots);
            self.run_counts[run_index] = run_count;
            changed_cells.extend(
                self.text_grid
                    .update_run_slots(atlas, run_index, &run_slots),
            );
            run_updates.push((offset, run_slots));
        }

        FixedTextScenePatch {
            run_updates,
            char_count: self.run_counts.iter().copied().sum(),
            changed_cells: changed_cells.into_iter().collect(),
        }
    }
}

pub fn build_fixed_text_scene_state_for_scene<'a>(
    scene: &RetainedScene,
    layout: FixedTextRunLayout<'a>,
    atlas: &VectorFontAtlas,
    colors: &TextColors,
    space: TextRenderSpace,
) -> (FixedTextSceneState, FixedTextSceneData) {
    FixedTextSceneState::new(layout, atlas, |run_index| {
        build_text_instances_for_text_slot(scene, run_index as u16, atlas, colors, space)
    })
}

pub fn rebuild_fixed_text_scene_state_for_scene(
    state: &mut FixedTextSceneState,
    scene: &RetainedScene,
    atlas: &VectorFontAtlas,
    colors: &TextColors,
    space: TextRenderSpace,
) -> FixedTextSceneData {
    state.rebuild(atlas, |run_index| {
        build_text_instances_for_text_slot(scene, run_index as u16, atlas, colors, space)
    })
}

pub fn update_fixed_text_scene_slots_for_scene(
    state: &mut FixedTextSceneState,
    scene: &RetainedScene,
    atlas: &VectorFontAtlas,
    text_slots: impl IntoIterator<Item = u16>,
    colors: &TextColors,
    space: TextRenderSpace,
) -> FixedTextScenePatch {
    state.update_runs(
        atlas,
        text_slots.into_iter().map(|slot| slot as usize),
        |run_index| {
            build_text_instances_for_text_slot(scene, run_index as u16, atlas, colors, space)
        },
    )
}

pub fn apply_fixed_text_runtime_update<LocalCharInstance: Clone>(
    update: FixedTextRuntimeUpdate,
    text_state: &FixedTextSceneState,
    char_instances: &mut Vec<LocalCharInstance>,
    char_count: &mut u32,
    char_grid_params: &mut [f32; 4],
    char_grid_bounds: &mut [f32; 4],
    max_char_instances: usize,
    max_grid_indices: usize,
    convert_instances: impl Fn(Vec<GpuCharInstanceEx>) -> Vec<LocalCharInstance>,
    mut write_full_instances: impl FnMut(&[LocalCharInstance]),
    mut write_partial_instances: impl FnMut(usize, &[GpuCharInstanceEx]),
    mut write_full_grid: impl FnMut(&[CharGridCell], &[u32]),
    mut write_grid_cell: impl FnMut(usize, &CharGridCell),
    mut write_grid_indices: impl FnMut(usize, &[u32]),
) {
    match update {
        FixedTextRuntimeUpdate::Full(text_data) => {
            let local_instances = convert_instances(text_data.char_instances);

            assert!(local_instances.len() <= max_char_instances);
            assert!(text_data.char_grid_indices.len() <= max_grid_indices);

            write_full_instances(&local_instances);
            write_full_grid(&text_data.char_grid_cells, &text_data.char_grid_indices);

            *char_instances = local_instances;
            *char_count = text_data.char_count;
            *char_grid_params = text_data.char_grid_params;
            *char_grid_bounds = text_data.char_grid_bounds;
        }
        FixedTextRuntimeUpdate::Partial(text_patch) => {
            for (offset, run_slots) in &text_patch.run_updates {
                let local_slots = convert_instances(run_slots.clone());
                char_instances[*offset..*offset + local_slots.len()].clone_from_slice(&local_slots);
                write_partial_instances(*offset, run_slots);
            }

            *char_count = text_patch.char_count;

            for cell_idx in text_patch.changed_cells {
                write_grid_cell(cell_idx, &text_state.text_grid.cells[cell_idx]);
                let offset = text_state.text_grid.cell_index_offset(cell_idx);
                write_grid_indices(
                    offset,
                    &text_state.text_grid.indices[offset..offset + text_state.grid_cell_capacity()],
                );
            }
        }
    }
}

fn build_fixed_text_scene_state_data<F>(
    layout: FixedTextRunLayout<'_>,
    atlas: &VectorFontAtlas,
    mut build_run_instances: F,
) -> (FixedTextSceneData, FixedTextGridCache, Vec<u32>)
where
    F: FnMut(usize) -> Vec<GpuCharInstanceEx>,
{
    let mut run_counts = Vec::with_capacity(layout.run_capacities.len());
    let text_data = build_fixed_text_scene_data(layout, atlas, |run_index| {
        let instances = build_run_instances(run_index);
        run_counts.push(instances.len() as u32);
        instances
    });
    let text_grid = FixedTextGridCache::new(&text_data.char_instances, atlas, layout);
    (text_data, text_grid, run_counts)
}

pub struct FixedTextGridCache {
    pub cells: Vec<CharGridCell>,
    pub indices: Vec<u32>,
    cell_chars: Vec<Vec<u32>>,
    char_cells: Vec<Vec<usize>>,
    grid_spec: FixedCharGridSpec,
    run_offsets: Vec<usize>,
    run_capacities: Vec<usize>,
}

impl FixedTextGridCache {
    pub fn new(
        instances: &[GpuCharInstanceEx],
        atlas: &VectorFontAtlas,
        layout: FixedTextRunLayout<'_>,
    ) -> Self {
        let instance_data: Vec<[f32; 4]> = instances.iter().map(|c| c.posAndChar_0).collect();
        let grid = build_fixed_char_grid(&instance_data, atlas, layout.grid_spec);
        let mut run_offsets = Vec::with_capacity(layout.run_capacities.len());
        let mut offset = 0usize;
        for &capacity in layout.run_capacities {
            run_offsets.push(offset);
            offset += capacity;
        }

        let mut cache = Self {
            cells: grid.cells,
            indices: grid.char_indices,
            cell_chars: vec![Vec::new(); layout.grid_cell_count()],
            char_cells: vec![Vec::new(); instances.len()],
            grid_spec: layout.grid_spec,
            run_offsets,
            run_capacities: layout.run_capacities.to_vec(),
        };
        cache.rebuild_membership();
        cache
    }

    fn rebuild_membership(&mut self) {
        self.cell_chars.iter_mut().for_each(Vec::clear);
        self.char_cells.iter_mut().for_each(Vec::clear);

        for cell_idx in 0..self.cells.len() {
            let cell = self.cells[cell_idx];
            let start = cell.offset as usize;
            let end = start + cell.count as usize;
            self.cell_chars[cell_idx].extend_from_slice(&self.indices[start..end]);
            for &char_idx in &self.cell_chars[cell_idx] {
                if let Some(char_cells) = self.char_cells.get_mut(char_idx as usize) {
                    char_cells.push(cell_idx);
                }
            }
        }
    }

    fn refresh_cell_storage(&mut self, cell_idx: usize) {
        let offset = self.cell_index_offset(cell_idx);
        let cell_chars = &self.cell_chars[cell_idx];
        self.cells[cell_idx] = CharGridCell {
            offset: offset as u32,
            count: cell_chars.len() as u32,
        };
        self.indices[offset..offset + self.grid_spec.cell_capacity].fill(0);
        self.indices[offset..offset + cell_chars.len()].copy_from_slice(cell_chars);
    }

    pub fn cell_index_offset(&self, cell_idx: usize) -> usize {
        cell_idx * self.grid_spec.cell_capacity
    }

    pub fn update_run_slots(
        &mut self,
        atlas: &VectorFontAtlas,
        run_index: usize,
        run_slots: &[GpuCharInstanceEx],
    ) -> Vec<usize> {
        assert_eq!(run_slots.len(), self.run_capacities[run_index]);

        let run_offset = self.run_offsets[run_index];
        let mut changed_cells = BTreeSet::new();

        for (slot_idx, instance) in run_slots.iter().enumerate() {
            let char_idx = run_offset + slot_idx;

            for cell_idx in std::mem::take(&mut self.char_cells[char_idx]) {
                let cell_chars = &mut self.cell_chars[cell_idx];
                if let Some(pos) = cell_chars.iter().position(|&idx| idx == char_idx as u32) {
                    cell_chars.remove(pos);
                }
                changed_cells.insert(cell_idx);
            }

            for cell_idx in
                fixed_char_grid_cells_for_instance(instance.posAndChar_0, atlas, self.grid_spec)
            {
                let cell_idx = cell_idx as usize;
                let cell_chars = &mut self.cell_chars[cell_idx];
                if !cell_chars.contains(&(char_idx as u32)) {
                    assert!(
                        cell_chars.len() < self.grid_spec.cell_capacity,
                        "Fixed text grid overflow: cell {cell_idx} needs > {} entries",
                        self.grid_spec.cell_capacity
                    );
                    cell_chars.push(char_idx as u32);
                    cell_chars.sort_unstable();
                }
                self.char_cells[char_idx].push(cell_idx);
                changed_cells.insert(cell_idx);
            }
        }

        for &cell_idx in &changed_cells {
            self.refresh_cell_storage(cell_idx);
        }

        changed_cells.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        assign_text_slots_and_build_layout, build_fixed_text_run_slot_buffer,
        build_fixed_text_scene_data, build_fixed_text_scene_state_for_scene,
        build_text_instances_for_node, build_text_instances_for_text_slot,
        build_text_instances_from_scene, count_active_char_instances,
        rebuild_fixed_text_scene_state_for_scene, update_fixed_text_scene_slots_for_scene,
        FixedTextGridCache, FixedTextRunLayout, TextColors, TextRenderSpace, ROLE_BODY,
        ROLE_COMPLETED, ROLE_HEADING, ROLE_INFO,
    };
    use crate::retained::samples::{
        build_scrolling_feed_scene, build_settings_panel_scene, sample_text_run_layout,
        scrolling_feed_text_run_layout,
    };
    use crate::text::{FixedCharGridSpec, VectorFont, VectorFontAtlas};
    use std::collections::BTreeSet;

    fn sample_colors() -> TextColors {
        TextColors {
            heading: [0.2, 0.2, 0.2],
            active: [0.3, 0.3, 0.3],
            completed: [0.4, 0.4, 0.4],
            placeholder: [0.5, 0.5, 0.5],
            body: [0.6, 0.6, 0.6],
            info: [0.7, 0.7, 0.7],
        }
    }

    fn sample_space() -> TextRenderSpace {
        TextRenderSpace {
            x_offset: 0.0,
            screen_height: 260.0,
            italic_codepoint_offset: 0x10000,
        }
    }

    fn load_test_atlas() -> VectorFontAtlas {
        let font_data = std::fs::read("assets/fonts/DejaVuSans.ttf").expect("load test font");
        let font = VectorFont::from_ttf(&font_data).expect("parse test font");
        VectorFontAtlas::from_font(&font)
    }

    fn text_slot(scene: &crate::retained::RetainedScene, name: &str) -> u16 {
        scene
            .node_named(name)
            .and_then(|node| node.text_slot)
            .expect("named text slot")
    }

    #[test]
    fn builds_text_instances_from_non_todomvc_scene() {
        let scene = build_settings_panel_scene();
        let atlas = load_test_atlas();
        let instances =
            build_text_instances_from_scene(&scene, &atlas, &sample_colors(), sample_space());

        assert!(!instances.is_empty());
        assert_eq!(
            count_active_char_instances(&instances, &atlas),
            instances.len() as u32
        );
        let roles = instances
            .iter()
            .map(|instance| instance.colorFlags_0[3] as u32)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            roles,
            BTreeSet::from([
                ROLE_HEADING as u32,
                ROLE_COMPLETED as u32,
                ROLE_BODY as u32,
                ROLE_INFO as u32,
            ])
        );
    }

    #[test]
    fn single_node_text_instances_match_scene_subset() {
        let scene = build_settings_panel_scene();
        let atlas = load_test_atlas();
        let node = scene
            .node_named("completed_label")
            .expect("completed label node");

        let direct = build_text_instances_for_node(
            &scene,
            node.id,
            &atlas,
            &sample_colors(),
            sample_space(),
        );
        let all = build_text_instances_from_scene(&scene, &atlas, &sample_colors(), sample_space());

        assert!(!direct.is_empty());
        assert!(all.windows(direct.len()).any(|window| window == direct));
    }

    #[test]
    fn text_slot_instances_match_direct_node_instances() {
        let scene = build_settings_panel_scene();
        let atlas = load_test_atlas();
        let node = scene
            .node_named("completed_label")
            .expect("completed label node");

        let direct = build_text_instances_for_node(
            &scene,
            node.id,
            &atlas,
            &sample_colors(),
            sample_space(),
        );
        let slotted = build_text_instances_for_text_slot(
            &scene,
            text_slot(&scene, "completed_label"),
            &atlas,
            &sample_colors(),
            sample_space(),
        );

        assert_eq!(slotted, direct);
    }

    #[test]
    fn fixed_text_helpers_build_scene_data_and_support_partial_updates() {
        let scene = build_settings_panel_scene();
        let atlas = load_test_atlas();
        let node = scene
            .node_named("completed_label")
            .expect("completed label node");
        let layout = FixedTextRunLayout {
            run_capacities: &[64],
            grid_spec: FixedCharGridSpec {
                dims: [16, 10],
                bounds: [0.0, 0.0, 420.0, 260.0],
                cell_capacity: 8,
            },
        };

        let scene_data = build_fixed_text_scene_data(layout, &atlas, |_| {
            build_text_instances_for_node(&scene, node.id, &atlas, &sample_colors(), sample_space())
        });
        let mut cache = FixedTextGridCache::new(&scene_data.char_instances, &atlas, layout);
        let updated_instances = build_text_instances_for_node(
            &scene,
            node.id,
            &atlas,
            &sample_colors(),
            sample_space(),
        );
        let (run_slots, run_count) =
            build_fixed_text_run_slot_buffer(updated_instances, layout.run_capacity(0), &atlas);
        let changed_cells = cache.update_run_slots(&atlas, 0, &run_slots);

        assert!(run_count > 0);
        assert!(!changed_cells.is_empty());
        assert_eq!(scene_data.char_count, run_count);
    }

    #[test]
    fn scene_state_helpers_build_and_update_from_scene_slots() {
        let atlas = load_test_atlas();
        let mut scene = build_settings_panel_scene();
        let layout = sample_text_run_layout();
        let (mut state, scene_data) = build_fixed_text_scene_state_for_scene(
            &scene,
            layout.layout(),
            &atlas,
            &sample_colors(),
            sample_space(),
        );

        assert!(scene_data.char_count > 0);
        assert_eq!(state.char_instances, scene_data.char_instances);

        super::super::samples::toggle_settings_panel_state(&mut scene);
        let patch = update_fixed_text_scene_slots_for_scene(
            &mut state,
            &scene,
            &atlas,
            [
                text_slot(&scene, "status_line"),
                text_slot(&scene, "completed_label"),
            ],
            &sample_colors(),
            sample_space(),
        );

        assert!(!patch.run_updates.is_empty());
        assert!(patch.char_count > 0);

        let rebuilt = rebuild_fixed_text_scene_state_for_scene(
            &mut state,
            &scene,
            &atlas,
            &sample_colors(),
            sample_space(),
        );
        assert_eq!(
            rebuilt.char_count,
            state.run_counts.iter().copied().sum::<u32>()
        );
    }

    #[test]
    fn sample_scene_fixed_layout_builds_multiple_text_runs() {
        let scene = build_settings_panel_scene();
        let atlas = load_test_atlas();
        let layout = sample_text_run_layout();

        let scene_data = build_fixed_text_scene_data(layout.layout(), &atlas, |run_index| {
            let Some(node) = scene.node_with_text_slot(run_index as u16) else {
                return Vec::new();
            };
            build_text_instances_for_node(&scene, node.id, &atlas, &sample_colors(), sample_space())
        });

        assert!(scene_data.char_count > 20);
        assert_eq!(
            scene_data.char_instances.len(),
            layout.layout().total_capacity()
        );
        assert_eq!(scene_data.char_grid_params, layout.layout().grid_params());
        assert_eq!(scene_data.char_grid_bounds, layout.grid_spec().bounds);
    }

    #[test]
    fn scrolling_feed_scene_clips_offscreen_text_runs() {
        let scene = build_scrolling_feed_scene();
        let atlas = load_test_atlas();
        let hidden = scene
            .node_named("feed_row_0_title")
            .expect("hidden row title node");
        let visible = scene
            .node_named("feed_row_2_title")
            .expect("visible row title node");

        let hidden_instances = build_text_instances_for_node(
            &scene,
            hidden.id,
            &atlas,
            &sample_colors(),
            sample_space(),
        );
        let visible_instances = build_text_instances_for_node(
            &scene,
            visible.id,
            &atlas,
            &sample_colors(),
            sample_space(),
        );
        let layout = scrolling_feed_text_run_layout();
        let scene_data = build_fixed_text_scene_data(layout.layout(), &atlas, |run_index| {
            let Some(node) = scene.node_with_text_slot(run_index as u16) else {
                return Vec::new();
            };
            build_text_instances_for_node(&scene, node.id, &atlas, &sample_colors(), sample_space())
        });

        assert!(hidden_instances.is_empty());
        assert!(!visible_instances.is_empty());
        assert!(scene_data.char_count > visible_instances.len() as u32);
        assert_eq!(
            scene_data.char_instances.len(),
            layout.layout().total_capacity()
        );
    }

    #[test]
    fn dynamic_text_slot_assignment_follows_scene_order() {
        let mut scene = build_settings_panel_scene();
        let layout = assign_text_slots_and_build_layout(
            &mut scene,
            FixedCharGridSpec {
                dims: [32, 20],
                bounds: [0.0, 0.0, 420.0, 260.0],
                cell_capacity: 64,
            },
            16,
        );

        let ordered_names = scene
            .descendants_sorted_by_resolved_position(scene.root())
            .into_iter()
            .filter(|node| node.text.is_some())
            .map(|node| node.name.as_deref().unwrap_or_default().to_string())
            .collect::<Vec<_>>();
        let slotted_names = (0..layout.run_capacities().len())
            .map(|slot| {
                scene
                    .node_with_text_slot(slot as u16)
                    .and_then(|node| node.name.as_deref())
                    .unwrap_or_default()
                    .to_string()
            })
            .collect::<Vec<_>>();

        assert_eq!(slotted_names, ordered_names);
    }
}
