//! Character spatial grid for fast GPU text lookup
//!
//! Divides the text bounding box into a 2D grid. Each cell stores indices
//! of characters whose bounding boxes overlap it, reducing per-pixel
//! character lookup from O(n) to O(1).

use super::glyph_atlas::{GlyphAtlasEntry, VectorFontAtlas};
use bytemuck::{Pod, Zeroable};
use std::collections::VecDeque;

/// GPU-side cell header: offset into char index list + count
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable, Default, PartialEq, Eq)]
pub struct CharGridCell {
    /// Offset into the char_indices array
    pub offset: u32,
    /// Number of character indices in this cell
    pub count: u32,
}

/// Result of building a character spatial grid
pub struct CharGrid {
    /// Grid dimensions (cols, rows)
    pub dims: [u32; 2],
    /// Grid world-space bounds [min_x, min_y, max_x, max_y]
    pub bounds: [f32; 4],
    /// Cell size in world units [cell_w, cell_h]
    pub cell_size: [f32; 2],
    /// Per-cell headers (dims.x * dims.y entries)
    pub cells: Vec<CharGridCell>,
    /// Flat list of character indices referenced by cells
    pub char_indices: Vec<u32>,
    /// Chebyshev distance from each cell to nearest occupied cell (for safe raymarcher steps)
    pub cell_distances: Vec<u32>,
}

/// Fixed-layout character grid specification with stable cell storage.
#[derive(Copy, Clone, Debug)]
pub struct FixedCharGridSpec {
    pub dims: [u32; 2],
    pub bounds: [f32; 4],
    pub cell_capacity: usize,
}

/// A character instance with its world-space bounding box
struct CharBBox {
    /// Index into the charInstances buffer
    char_idx: u32,
    /// World-space bounding box [min_x, min_y, max_x, max_y]
    bbox: [f32; 4],
}

fn char_bbox_for_instance(
    char_idx: u32,
    inst: [f32; 4],
    atlas: &VectorFontAtlas,
) -> Option<CharBBox> {
    let x = inst[0];
    let y = inst[1];
    let scale = inst[2];
    let glyph_idx = inst[3] as usize;

    if glyph_idx >= atlas.glyph_list.len() {
        return None;
    }

    let (_, entry): &(u32, GlyphAtlasEntry) = &atlas.glyph_list[glyph_idx];
    let bounds = entry.bounds;

    Some(CharBBox {
        char_idx,
        bbox: [
            x + bounds[0] * scale,
            y + bounds[1] * scale,
            x + bounds[2] * scale,
            y + bounds[3] * scale,
        ],
    })
}

fn compute_cell_distances(cell_chars: &[Vec<u32>], grid_dims: [u32; 2]) -> Vec<u32> {
    let grid_w = grid_dims[0] as usize;
    let grid_h = grid_dims[1] as usize;
    let total_cells = grid_w * grid_h;
    let mut cell_distances = vec![u32::MAX; total_cells];
    let mut queue = VecDeque::new();

    for (idx, cell_list) in cell_chars.iter().enumerate() {
        if !cell_list.is_empty() {
            cell_distances[idx] = 0;
            queue.push_back(idx);
        }
    }

    while let Some(idx) = queue.pop_front() {
        let cx = (idx % grid_w) as i32;
        let cy = (idx / grid_w) as i32;
        let current_dist = cell_distances[idx];

        for dy in -1..=1_i32 {
            for dx in -1..=1_i32 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = cx + dx;
                let ny = cy + dy;
                if nx < 0 || ny < 0 || nx >= grid_w as i32 || ny >= grid_h as i32 {
                    continue;
                }
                let nidx = (ny * grid_w as i32 + nx) as usize;
                if current_dist + 1 < cell_distances[nidx] {
                    cell_distances[nidx] = current_dist + 1;
                    queue.push_back(nidx);
                }
            }
        }
    }

    for d in &mut cell_distances {
        *d = (*d).min(255);
    }

    cell_distances
}

/// Returns the fixed-grid cells overlapped by one character instance.
pub fn fixed_char_grid_cells_for_instance(
    inst: [f32; 4],
    atlas: &VectorFontAtlas,
    spec: FixedCharGridSpec,
) -> Vec<u32> {
    let Some(cb) = char_bbox_for_instance(0, inst, atlas) else {
        return Vec::new();
    };

    let [grid_w, grid_h] = spec.dims;
    let [min_x, min_y, max_x, max_y] = spec.bounds;
    let cell_w = (max_x - min_x) / grid_w as f32;
    let cell_h = (max_y - min_y) / grid_h as f32;
    let margin = 0.15 * inst[2];

    let overlap_min_x = (cb.bbox[0] - margin).max(min_x);
    let overlap_min_y = (cb.bbox[1] - margin).max(min_y);
    let overlap_max_x = (cb.bbox[2] + margin).min(max_x);
    let overlap_max_y = (cb.bbox[3] + margin).min(max_y);
    if overlap_max_x <= overlap_min_x || overlap_max_y <= overlap_min_y {
        return Vec::new();
    }

    let cx_min = ((overlap_min_x - min_x) / cell_w).floor().max(0.0) as u32;
    let cy_min = ((overlap_min_y - min_y) / cell_h).floor().max(0.0) as u32;
    let cx_max = ((overlap_max_x - min_x) / cell_w).ceil().min(grid_w as f32) as u32;
    let cy_max = ((overlap_max_y - min_y) / cell_h).ceil().min(grid_h as f32) as u32;

    let mut cells = Vec::new();
    for cy in cy_min..cy_max {
        for cx in cx_min..cx_max {
            cells.push(cy * grid_w + cx);
        }
    }

    cells
}

/// Build a character spatial grid from character instances.
///
/// `instances` is a slice of `[x, y, scale, glyph_idx]` arrays (matching GpuCharInstance layout).
/// `atlas` provides glyph bounds for computing world-space bounding boxes.
/// `grid_dims` is (cols, rows) for the grid.
pub fn build_char_grid(
    instances: &[[f32; 4]],
    atlas: &VectorFontAtlas,
    grid_dims: [u32; 2],
) -> CharGrid {
    if instances.is_empty() {
        let total = (grid_dims[0] * grid_dims[1]) as usize;
        return CharGrid {
            dims: grid_dims,
            bounds: [0.0; 4],
            cell_size: [1.0, 1.0],
            cells: vec![CharGridCell::default(); total],
            char_indices: vec![0],
            cell_distances: vec![255; total],
        };
    }

    // Compute world-space bounding boxes for all characters
    let mut char_bboxes: Vec<CharBBox> = Vec::with_capacity(instances.len());
    let mut global_min_x = f32::MAX;
    let mut global_min_y = f32::MAX;
    let mut global_max_x = f32::MIN;
    let mut global_max_y = f32::MIN;

    for (i, inst) in instances.iter().enumerate() {
        let Some(char_bbox) = char_bbox_for_instance(i as u32, *inst, atlas) else {
            continue;
        };

        global_min_x = global_min_x.min(char_bbox.bbox[0]);
        global_min_y = global_min_y.min(char_bbox.bbox[1]);
        global_max_x = global_max_x.max(char_bbox.bbox[2]);
        global_max_y = global_max_y.max(char_bbox.bbox[3]);

        char_bboxes.push(char_bbox);
    }

    // Add small margin to avoid edge cases
    global_min_x -= 0.01;
    global_min_y -= 0.01;
    global_max_x += 0.01;
    global_max_y += 0.01;

    let grid_w = grid_dims[0];
    let grid_h = grid_dims[1];
    let cell_w = (global_max_x - global_min_x) / grid_w as f32;
    let cell_h = (global_max_y - global_min_y) / grid_h as f32;

    // Build per-cell character lists
    let total_cells = (grid_w * grid_h) as usize;
    let mut cell_chars: Vec<Vec<u32>> = vec![Vec::new(); total_cells];

    for cb in &char_bboxes {
        // Compute cell range this character overlaps (with margin for SDF evaluation)
        let margin = 0.15 * instances[cb.char_idx as usize][2]; // 0.15 * scale
        let cx_min = ((cb.bbox[0] - margin - global_min_x) / cell_w)
            .floor()
            .max(0.0) as u32;
        let cy_min = ((cb.bbox[1] - margin - global_min_y) / cell_h)
            .floor()
            .max(0.0) as u32;
        let cx_max = ((cb.bbox[2] + margin - global_min_x) / cell_w)
            .ceil()
            .min(grid_w as f32) as u32;
        let cy_max = ((cb.bbox[3] + margin - global_min_y) / cell_h)
            .ceil()
            .min(grid_h as f32) as u32;

        for cy in cy_min..cy_max {
            for cx in cx_min..cx_max {
                let cell_idx = (cy * grid_w + cx) as usize;
                if cell_idx < total_cells {
                    cell_chars[cell_idx].push(cb.char_idx);
                }
            }
        }
    }

    // Flatten into offset/count format
    let mut cells = Vec::with_capacity(total_cells);
    let mut char_indices: Vec<u32> = Vec::new();

    for cell_list in &cell_chars {
        let offset = char_indices.len() as u32;
        let count = cell_list.len() as u32;
        cells.push(CharGridCell { offset, count });
        char_indices.extend_from_slice(cell_list);
    }

    // Ensure at least one element in char_indices for GPU buffer
    if char_indices.is_empty() {
        char_indices.push(0);
    }

    // Compute Chebyshev distance from each cell to nearest occupied cell (BFS)
    CharGrid {
        dims: [grid_w, grid_h],
        bounds: [global_min_x, global_min_y, global_max_x, global_max_y],
        cell_size: [cell_w, cell_h],
        cells,
        char_indices,
        cell_distances: compute_cell_distances(&cell_chars, [grid_w, grid_h]),
    }
}

/// Build a fixed-bounds character grid with stable per-cell storage offsets.
pub fn build_fixed_char_grid(
    instances: &[[f32; 4]],
    atlas: &VectorFontAtlas,
    spec: FixedCharGridSpec,
) -> CharGrid {
    let [grid_w, grid_h] = spec.dims;
    let total_cells = (grid_w * grid_h) as usize;
    let cell_w = (spec.bounds[2] - spec.bounds[0]) / grid_w as f32;
    let cell_h = (spec.bounds[3] - spec.bounds[1]) / grid_h as f32;
    let mut cell_chars: Vec<Vec<u32>> = vec![Vec::new(); total_cells];

    for (char_idx, inst) in instances.iter().enumerate() {
        for cell_idx in fixed_char_grid_cells_for_instance(*inst, atlas, spec) {
            let cell_chars_for_cell = &mut cell_chars[cell_idx as usize];
            assert!(
                cell_chars_for_cell.len() < spec.cell_capacity,
                "fixed char grid cell overflow: cell {cell_idx} needs > {} entries",
                spec.cell_capacity
            );
            cell_chars_for_cell.push(char_idx as u32);
        }
    }

    let mut cells = Vec::with_capacity(total_cells);
    let mut char_indices = vec![0u32; total_cells * spec.cell_capacity];
    for (cell_idx, cell_list) in cell_chars.iter().enumerate() {
        let offset = cell_idx * spec.cell_capacity;
        cells.push(CharGridCell {
            offset: offset as u32,
            count: cell_list.len() as u32,
        });
        char_indices[offset..offset + cell_list.len()].copy_from_slice(cell_list);
    }

    CharGrid {
        dims: [grid_w, grid_h],
        bounds: spec.bounds,
        cell_size: [cell_w, cell_h],
        cells,
        char_indices,
        cell_distances: compute_cell_distances(&cell_chars, [grid_w, grid_h]),
    }
}
