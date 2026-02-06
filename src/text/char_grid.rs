//! Character spatial grid for fast GPU text lookup
//!
//! Divides the text bounding box into a 2D grid. Each cell stores indices
//! of characters whose bounding boxes overlap it, reducing per-pixel
//! character lookup from O(n) to O(1).

use super::glyph_atlas::{GlyphAtlasEntry, VectorFontAtlas};
use bytemuck::{Pod, Zeroable};

/// GPU-side cell header: offset into char index list + count
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable, Default)]
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
}

/// A character instance with its world-space bounding box
struct CharBBox {
    /// Index into the charInstances buffer
    char_idx: u32,
    /// World-space bounding box [min_x, min_y, max_x, max_y]
    bbox: [f32; 4],
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
        return CharGrid {
            dims: grid_dims,
            bounds: [0.0; 4],
            cell_size: [1.0, 1.0],
            cells: vec![CharGridCell::default(); (grid_dims[0] * grid_dims[1]) as usize],
            char_indices: vec![0],
        };
    }

    // Compute world-space bounding boxes for all characters
    let mut char_bboxes: Vec<CharBBox> = Vec::with_capacity(instances.len());
    let mut global_min_x = f32::MAX;
    let mut global_min_y = f32::MAX;
    let mut global_max_x = f32::MIN;
    let mut global_max_y = f32::MIN;

    for (i, inst) in instances.iter().enumerate() {
        let x = inst[0];
        let y = inst[1];
        let scale = inst[2];
        let glyph_idx = inst[3] as usize;

        if glyph_idx >= atlas.glyph_list.len() {
            continue;
        }

        let (_, entry): &(u32, GlyphAtlasEntry) = &atlas.glyph_list[glyph_idx];
        let bounds = entry.bounds;

        let min_x = x + bounds[0] * scale;
        let min_y = y + bounds[1] * scale;
        let max_x = x + bounds[2] * scale;
        let max_y = y + bounds[3] * scale;

        global_min_x = global_min_x.min(min_x);
        global_min_y = global_min_y.min(min_y);
        global_max_x = global_max_x.max(max_x);
        global_max_y = global_max_y.max(max_y);

        char_bboxes.push(CharBBox {
            char_idx: i as u32,
            bbox: [min_x, min_y, max_x, max_y],
        });
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
        let cx_min = ((cb.bbox[0] - margin - global_min_x) / cell_w).floor().max(0.0) as u32;
        let cy_min = ((cb.bbox[1] - margin - global_min_y) / cell_h).floor().max(0.0) as u32;
        let cx_max = ((cb.bbox[2] + margin - global_min_x) / cell_w).ceil().min(grid_w as f32) as u32;
        let cy_max = ((cb.bbox[3] + margin - global_min_y) / cell_h).ceil().min(grid_h as f32) as u32;

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

    CharGrid {
        dims: [grid_w, grid_h],
        bounds: [global_min_x, global_min_y, global_max_x, global_max_y],
        cell_size: [cell_w, cell_h],
        cells,
        char_indices,
    }
}
