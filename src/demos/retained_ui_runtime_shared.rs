use super::gpu_runtime_common::{build_font_gpu_data, create_storage_buffers, UiStorageBuffers};
use crate::retained::text::{
    apply_fixed_text_runtime_update, FixedTextRuntimeUpdate, FixedTextSceneData,
    FixedTextScenePatch, FixedTextSceneState, GpuCharInstanceEx,
};
use crate::retained::ui::{
    apply_gpu_ui_runtime_update, GpuUiPatch, GpuUiPrimitive, GpuUiRuntimeUpdate,
};
use crate::text::{CharGridCell, VectorFontAtlas};

pub struct RetainedUiRuntimeState {
    pub char_instances: Vec<GpuCharInstanceEx>,
    pub char_count: u32,
    pub ui_prim_count: u32,
    pub char_grid_params: [f32; 4],
    pub char_grid_bounds: [f32; 4],
    pub text_capacity: usize,
    pub grid_cell_capacity: usize,
    pub grid_index_capacity: usize,
    pub primitive_capacity: usize,
}

impl RetainedUiRuntimeState {
    pub fn new(
        text_data: &FixedTextSceneData,
        ui_primitives: &[GpuUiPrimitive],
        text_capacity: usize,
        grid_index_capacity: usize,
        primitive_capacity: usize,
    ) -> Self {
        Self {
            char_instances: text_data.char_instances.clone(),
            char_count: text_data.char_count,
            ui_prim_count: ui_primitives.len() as u32,
            char_grid_params: text_data.char_grid_params,
            char_grid_bounds: text_data.char_grid_bounds,
            text_capacity,
            grid_cell_capacity: text_data.char_grid_cells.len(),
            grid_index_capacity,
            primitive_capacity,
        }
    }

    pub fn can_fit_scene_data(
        &self,
        text_data: &FixedTextSceneData,
        ui_primitives: &[GpuUiPrimitive],
    ) -> bool {
        text_data.char_instances.len() <= self.text_capacity
            && text_data.char_grid_cells.len() <= self.grid_cell_capacity
            && text_data.char_grid_indices.len() <= self.grid_index_capacity
            && ui_primitives.len() <= self.primitive_capacity
    }

    pub fn sync_text_scene_data(
        &mut self,
        queue: &wgpu::Queue,
        storage_buffers: &UiStorageBuffers,
        text_data: &FixedTextSceneData,
    ) {
        assert!(text_data.char_instances.len() <= self.text_capacity);
        assert!(text_data.char_grid_cells.len() <= self.grid_cell_capacity);
        assert!(text_data.char_grid_indices.len() <= self.grid_index_capacity);

        queue.write_buffer(
            &storage_buffers.char_instances_buffer,
            0,
            bytemuck::cast_slice(&text_data.char_instances),
        );
        queue.write_buffer(
            &storage_buffers.char_grid_cells_buffer,
            0,
            bytemuck::cast_slice(&text_data.char_grid_cells),
        );
        queue.write_buffer(
            &storage_buffers.char_grid_indices_buffer,
            0,
            bytemuck::cast_slice(&text_data.char_grid_indices),
        );

        self.char_instances = text_data.char_instances.clone();
        self.char_count = text_data.char_count;
        self.char_grid_params = text_data.char_grid_params;
        self.char_grid_bounds = text_data.char_grid_bounds;
    }

    pub fn sync_ui_scene_data(
        &mut self,
        queue: &wgpu::Queue,
        storage_buffers: &UiStorageBuffers,
        ui_primitives: &[GpuUiPrimitive],
    ) {
        assert!(ui_primitives.len() <= self.primitive_capacity);
        queue.write_buffer(
            &storage_buffers.ui_primitives_buffer,
            0,
            bytemuck::cast_slice(ui_primitives),
        );
        self.ui_prim_count = ui_primitives.len() as u32;
    }

    pub fn sync_scene_data(
        &mut self,
        queue: &wgpu::Queue,
        storage_buffers: &UiStorageBuffers,
        text_data: &FixedTextSceneData,
        ui_primitives: &[GpuUiPrimitive],
    ) {
        self.sync_text_scene_data(queue, storage_buffers, text_data);
        self.sync_ui_scene_data(queue, storage_buffers, ui_primitives);
    }

    pub fn sync_ui_patches(
        &mut self,
        queue: &wgpu::Queue,
        storage_buffers: &UiStorageBuffers,
        patches: &[GpuUiPatch],
    ) {
        let primitive_size = std::mem::size_of::<GpuUiPrimitive>() as u64;
        for patch in patches {
            if patch.primitives.is_empty() {
                continue;
            }
            queue.write_buffer(
                &storage_buffers.ui_primitives_buffer,
                patch.offset as u64 * primitive_size,
                bytemuck::cast_slice(&patch.primitives),
            );
        }
    }

    pub fn sync_text_patch(
        &mut self,
        queue: &wgpu::Queue,
        storage_buffers: &UiStorageBuffers,
        text_patch: &FixedTextScenePatch,
        text_state: &FixedTextSceneState,
    ) {
        let char_instance_size = std::mem::size_of::<GpuCharInstanceEx>() as u64;
        for (offset, slots) in &text_patch.run_updates {
            if slots.is_empty() {
                continue;
            }
            self.char_instances[*offset..*offset + slots.len()].clone_from_slice(slots);
            queue.write_buffer(
                &storage_buffers.char_instances_buffer,
                *offset as u64 * char_instance_size,
                bytemuck::cast_slice(slots),
            );
        }

        let grid_cell_size = std::mem::size_of::<CharGridCell>() as u64;
        let grid_index_size = std::mem::size_of::<u32>() as u64;
        for &cell_idx in &text_patch.changed_cells {
            queue.write_buffer(
                &storage_buffers.char_grid_cells_buffer,
                cell_idx as u64 * grid_cell_size,
                bytemuck::cast_slice(&[text_state.text_grid.cells[cell_idx]]),
            );

            let index_offset = text_state.text_grid.cell_index_offset(cell_idx);
            queue.write_buffer(
                &storage_buffers.char_grid_indices_buffer,
                index_offset as u64 * grid_index_size,
                bytemuck::cast_slice(
                    &text_state.text_grid.indices
                        [index_offset..index_offset + self.grid_cell_capacity],
                ),
            );
        }

        self.char_count = text_patch.char_count;
    }

    pub fn apply_text_runtime_update(
        &mut self,
        queue: &wgpu::Queue,
        storage_buffers: &UiStorageBuffers,
        update: FixedTextRuntimeUpdate,
        text_state: &FixedTextSceneState,
    ) {
        apply_fixed_text_runtime_update(
            update,
            text_state,
            &mut self.char_instances,
            &mut self.char_count,
            &mut self.char_grid_params,
            &mut self.char_grid_bounds,
            self.text_capacity,
            self.grid_index_capacity,
            |instances| instances,
            |instances| {
                queue.write_buffer(
                    &storage_buffers.char_instances_buffer,
                    0,
                    bytemuck::cast_slice(instances),
                );
            },
            |offset, run_slots| {
                queue.write_buffer(
                    &storage_buffers.char_instances_buffer,
                    (offset * std::mem::size_of::<GpuCharInstanceEx>()) as u64,
                    bytemuck::cast_slice(run_slots),
                );
            },
            |cells, indices| {
                queue.write_buffer(
                    &storage_buffers.char_grid_cells_buffer,
                    0,
                    bytemuck::cast_slice(cells),
                );
                queue.write_buffer(
                    &storage_buffers.char_grid_indices_buffer,
                    0,
                    bytemuck::cast_slice(indices),
                );
            },
            |cell_idx, cell| {
                queue.write_buffer(
                    &storage_buffers.char_grid_cells_buffer,
                    (cell_idx * std::mem::size_of::<CharGridCell>()) as u64,
                    bytemuck::bytes_of(cell),
                );
            },
            |offset, indices| {
                queue.write_buffer(
                    &storage_buffers.char_grid_indices_buffer,
                    (offset * std::mem::size_of::<u32>()) as u64,
                    bytemuck::cast_slice(indices),
                );
            },
        );
    }

    pub fn apply_ui_runtime_update(
        &mut self,
        queue: &wgpu::Queue,
        storage_buffers: &UiStorageBuffers,
        update: GpuUiRuntimeUpdate,
    ) {
        apply_gpu_ui_runtime_update(
            update,
            &mut self.ui_prim_count,
            self.primitive_capacity,
            |primitives| {
                queue.write_buffer(
                    &storage_buffers.ui_primitives_buffer,
                    0,
                    bytemuck::cast_slice(primitives),
                );
            },
            |offset, primitives| {
                queue.write_buffer(
                    &storage_buffers.ui_primitives_buffer,
                    (offset * std::mem::size_of::<GpuUiPrimitive>()) as u64,
                    bytemuck::cast_slice(primitives),
                );
            },
        );
    }
}

pub fn create_retained_ui_storage_buffers(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    atlas: &VectorFontAtlas,
    text_data: &FixedTextSceneData,
    max_char_instances: usize,
    max_grid_indices: usize,
    ui_primitives: &[GpuUiPrimitive],
    max_ui_primitives: usize,
    label: &str,
) -> UiStorageBuffers {
    create_storage_buffers(
        device,
        queue,
        &build_font_gpu_data(atlas),
        bytemuck::cast_slice(&text_data.char_instances),
        max_char_instances * std::mem::size_of::<GpuCharInstanceEx>(),
        &text_data.char_grid_cells,
        &text_data.char_grid_indices,
        max_grid_indices,
        bytemuck::cast_slice(ui_primitives),
        max_ui_primitives * std::mem::size_of::<GpuUiPrimitive>(),
        label,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retained::text::gpu_char_instance_ex;

    fn sample_text_data(cell_count: usize, index_count: usize) -> FixedTextSceneData {
        FixedTextSceneData {
            char_instances: vec![gpu_char_instance_ex([0.0; 4], [0.0; 4]); 4],
            char_count: 4,
            char_grid_params: [0.0; 4],
            char_grid_bounds: [0.0; 4],
            char_grid_cells: vec![
                CharGridCell {
                    offset: 0,
                    count: 0,
                };
                cell_count
            ],
            char_grid_indices: vec![0; index_count],
            char_grid_distances: vec![0; cell_count],
        }
    }

    fn sample_ui_primitives(count: usize) -> Vec<GpuUiPrimitive> {
        vec![GpuUiPrimitive::new([0.0; 4], [0.0; 4], [0.0; 4], [0.0; 4]); count]
    }

    #[test]
    fn can_fit_scene_data_uses_actual_grid_cell_capacity() {
        let text_data = sample_text_data(4, 16);
        let ui_primitives = sample_ui_primitives(2);
        let state = RetainedUiRuntimeState::new(&text_data, &ui_primitives, 8, 32, 4);

        let larger_grid = sample_text_data(5, 16);
        assert!(!state.can_fit_scene_data(&larger_grid, &ui_primitives));
    }

    #[test]
    fn can_fit_scene_data_accepts_scene_within_current_buffer_sizes() {
        let text_data = sample_text_data(4, 16);
        let ui_primitives = sample_ui_primitives(2);
        let state = RetainedUiRuntimeState::new(&text_data, &ui_primitives, 8, 32, 4);

        let smaller_scene = sample_text_data(3, 12);
        let smaller_ui = sample_ui_primitives(1);
        assert!(state.can_fit_scene_data(&smaller_scene, &smaller_ui));
    }
}
