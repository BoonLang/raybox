//! Grid-based glyph atlas for GPU SDF text rendering
//!
//! Subdivides each glyph into a grid where each cell stores indices to
//! intersecting Bézier curves, reducing per-pixel cost from O(n) to O(1-3).

use super::vector_font::{BezierCurve, VectorFont, VectorGlyphMetrics};
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;

/// Grid cell storing indices to curves that intersect it
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable, Default)]
pub struct GridCell {
    /// Start index into curve index array
    pub curve_start: u16,
    /// Number of curves in this cell (typically 0-4)
    pub curve_count: u8,
    /// Flags: bit 0 = fully inside glyph, bit 1 = fully outside
    pub flags: u8,
}

impl GridCell {
    pub const FLAG_INSIDE: u8 = 0x01;
    pub const FLAG_OUTSIDE: u8 = 0x02;
}

/// Metadata for a glyph in the atlas
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable, Default)]
pub struct GlyphAtlasEntry {
    /// Glyph bounds [left, bottom, right, top] in em units
    pub bounds: [f32; 4],
    /// Advance width in em units
    pub advance: f32,
    /// Grid dimensions (width, height)
    pub grid_size: [u32; 2],
    /// Offset into grid_cells array
    pub grid_offset: u32,
    /// Offset into curves array
    pub curve_offset: u32,
    /// Number of curves
    pub curve_count: u32,
    /// Padding
    pub _padding: [u32; 2],
}

/// Complete vector font atlas with grid subdivision
pub struct VectorFontAtlas {
    /// All curves from all glyphs, densely packed
    pub curves: Vec<BezierCurve>,
    /// Grid cells for all glyphs, densely packed
    pub grid_cells: Vec<GridCell>,
    /// Curve indices referenced by grid cells
    pub curve_indices: Vec<u16>,
    /// Per-glyph metadata, indexed by codepoint
    pub glyphs: HashMap<u32, GlyphAtlasEntry>,
    /// Ordered list of glyph entries for GPU upload
    #[allow(dead_code)]
    pub glyph_list: Vec<(u32, GlyphAtlasEntry)>,
    /// Grid resolution (cells per em unit)
    pub grid_resolution: u32,
    /// Font metrics
    pub ascender: f32,
    #[allow(dead_code)]
    pub descender: f32,
    #[allow(dead_code)]
    pub line_height: f32,
}

impl VectorFontAtlas {
    /// Build an atlas from a vector font
    pub fn from_font(font: &VectorFont, grid_resolution: u32) -> Self {
        let mut atlas = VectorFontAtlas {
            curves: Vec::new(),
            grid_cells: Vec::new(),
            curve_indices: Vec::new(),
            glyphs: HashMap::new(),
            glyph_list: Vec::new(),
            grid_resolution,
            ascender: font.ascender,
            descender: font.descender,
            line_height: font.line_height,
        };

        // Process each glyph
        for (&codepoint, glyph_metrics) in &font.glyphs {
            atlas.add_glyph(font, codepoint, glyph_metrics);
        }

        // Sort glyph list by codepoint for consistent GPU ordering
        atlas.glyph_list.sort_by_key(|(cp, _)| *cp);

        atlas
    }

    fn add_glyph(&mut self, font: &VectorFont, codepoint: u32, metrics: &VectorGlyphMetrics) {
        let glyph_curves = font.get_glyph_curves(metrics);

        // Copy curves to our array
        let curve_offset = self.curves.len() as u32;
        self.curves.extend_from_slice(glyph_curves);

        // Calculate grid dimensions based on glyph bounds
        let width = metrics.bounds[2] - metrics.bounds[0];
        let height = metrics.bounds[3] - metrics.bounds[1];

        // Ensure at least 1x1 grid
        let grid_w = ((width * self.grid_resolution as f32).ceil() as u32).max(1);
        let grid_h = ((height * self.grid_resolution as f32).ceil() as u32).max(1);

        let grid_offset = self.grid_cells.len() as u32;

        // Build grid cells
        for gy in 0..grid_h {
            for gx in 0..grid_w {
                let cell_bounds = self.cell_to_bounds(metrics, gx, gy, grid_w, grid_h);
                let cell = self.build_cell(&cell_bounds, glyph_curves, curve_offset);
                self.grid_cells.push(cell);
            }
        }

        let entry = GlyphAtlasEntry {
            bounds: metrics.bounds,
            advance: metrics.advance,
            grid_size: [grid_w, grid_h],
            grid_offset,
            curve_offset,
            curve_count: metrics.curve_count,
            _padding: [0; 2],
        };

        self.glyphs.insert(codepoint, entry);
        self.glyph_list.push((codepoint, entry));
    }

    fn cell_to_bounds(
        &self,
        glyph: &VectorGlyphMetrics,
        cx: u32,
        cy: u32,
        grid_w: u32,
        grid_h: u32,
    ) -> [f32; 4] {
        let glyph_width = glyph.bounds[2] - glyph.bounds[0];
        let glyph_height = glyph.bounds[3] - glyph.bounds[1];

        let cell_width = glyph_width / grid_w as f32;
        let cell_height = glyph_height / grid_h as f32;

        [
            glyph.bounds[0] + cx as f32 * cell_width,
            glyph.bounds[1] + cy as f32 * cell_height,
            glyph.bounds[0] + (cx + 1) as f32 * cell_width,
            glyph.bounds[1] + (cy + 1) as f32 * cell_height,
        ]
    }

    fn build_cell(
        &mut self,
        cell_bounds: &[f32; 4],
        curves: &[BezierCurve],
        curve_offset: u32,
    ) -> GridCell {
        let curve_start = self.curve_indices.len() as u16;
        let mut curve_count = 0u8;

        // Find curves that intersect this cell
        for (i, curve) in curves.iter().enumerate() {
            if curve.intersects_aabb(cell_bounds) {
                self.curve_indices.push((curve_offset + i as u32) as u16);
                curve_count = curve_count.saturating_add(1);
                // Limit to 255 curves per cell (unlikely to ever hit this)
                if curve_count == 255 {
                    break;
                }
            }
        }

        // Compute inside/outside flags based on winding at cell center
        let center = [
            (cell_bounds[0] + cell_bounds[2]) * 0.5,
            (cell_bounds[1] + cell_bounds[3]) * 0.5,
        ];

        let winding = self.compute_winding(center, curves);
        let is_inside = winding != 0;

        // Check all corners to see if cell is uniform
        let corners = [
            [cell_bounds[0], cell_bounds[1]],
            [cell_bounds[2], cell_bounds[1]],
            [cell_bounds[2], cell_bounds[3]],
            [cell_bounds[0], cell_bounds[3]],
        ];

        let all_same = corners
            .iter()
            .all(|c| (self.compute_winding(*c, curves) != 0) == is_inside);

        // Store inside/outside status for all cells
        // Bit 0 (FLAG_INSIDE): center is inside the glyph
        // Bit 1 (FLAG_OUTSIDE): cell is uniform (all corners same as center) - for early return
        let mut flags = 0u8;
        if is_inside {
            flags |= GridCell::FLAG_INSIDE;
        }
        if curve_count == 0 || all_same {
            flags |= GridCell::FLAG_OUTSIDE; // Repurpose as "uniform" flag
        }

        GridCell {
            curve_start,
            curve_count,
            flags,
        }
    }

    /// Compute winding number at a point using horizontal ray casting
    fn compute_winding(&self, point: [f32; 2], curves: &[BezierCurve]) -> i32 {
        let mut winding = 0i32;

        for curve in curves {
            winding += self.curve_winding_contribution(point, curve);
        }

        winding
    }

    /// Compute winding contribution of a single quadratic Bézier curve
    /// Uses epsilon to avoid boundary issues at curve junctions
    fn curve_winding_contribution(&self, point: [f32; 2], curve: &BezierCurve) -> i32 {
        const EPS: f32 = 1e-6;

        let (px, py) = (point[0], point[1]);
        let (x0, y0) = curve.p0();
        let (x1, y1) = curve.p1();
        let (x2, y2) = curve.p2();

        // Check if ray can possibly intersect
        let y_min = y0.min(y1).min(y2);
        let y_max = y0.max(y1).max(y2);
        if py < y_min - EPS || py >= y_max - EPS {
            return 0;
        }

        // Check if point is to the right of all control points
        let x_max = x0.max(x1).max(x2);
        if px > x_max + EPS {
            return 0;
        }

        // Solve for t where y(t) = py
        let a = y0 - 2.0 * y1 + y2;
        let b = 2.0 * (y1 - y0);
        let c = y0 - py;

        let mut crossings = 0i32;

        if a.abs() < 1e-7 {
            // Nearly linear case
            if b.abs() > 1e-7 {
                let t = -c / b;
                if t > -EPS && t < 1.0 - EPS {
                    let x = (1.0 - t) * (1.0 - t) * x0 + 2.0 * (1.0 - t) * t * x1 + t * t * x2;
                    if x > px {
                        let dy = y2 - y0;
                        crossings += if dy > 0.0 { 1 } else { -1 };
                    }
                }
            }
        } else {
            let discriminant = b * b - 4.0 * a * c;
            if discriminant >= 0.0 {
                let sqrt_d = discriminant.sqrt();
                let t1 = (-b - sqrt_d) / (2.0 * a);
                let t2 = (-b + sqrt_d) / (2.0 * a);

                for t in [t1, t2] {
                    // Use slightly open interval to avoid endpoint issues
                    if t > EPS && t < 1.0 - EPS {
                        let x = (1.0 - t) * (1.0 - t) * x0 + 2.0 * (1.0 - t) * t * x1 + t * t * x2;
                        if x > px {
                            let dy_dt = 2.0 * ((y1 - y0) * (1.0 - t) + (y2 - y1) * t);
                            crossings += if dy_dt > 0.0 { 1 } else { -1 };
                        }
                    }
                }
            }
        }

        crossings
    }

    /// Get glyph entry by character
    #[allow(dead_code)]
    pub fn get_glyph(&self, ch: char) -> Option<&GlyphAtlasEntry> {
        self.glyphs.get(&(ch as u32))
    }

    /// Get total size needed for curve buffer in bytes
    #[allow(dead_code)]
    pub fn curve_buffer_size(&self) -> usize {
        self.curves.len() * std::mem::size_of::<BezierCurve>()
    }

    /// Get total size needed for grid cell buffer in bytes
    #[allow(dead_code)]
    pub fn grid_cell_buffer_size(&self) -> usize {
        self.grid_cells.len() * std::mem::size_of::<GridCell>()
    }

    /// Get total size needed for curve index buffer in bytes
    #[allow(dead_code)]
    pub fn curve_index_buffer_size(&self) -> usize {
        self.curve_indices.len() * std::mem::size_of::<u16>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_cell_flags() {
        let cell = GridCell {
            curve_start: 0,
            curve_count: 0,
            flags: GridCell::FLAG_INSIDE,
        };
        assert_eq!(cell.flags & GridCell::FLAG_INSIDE, GridCell::FLAG_INSIDE);
        assert_eq!(cell.flags & GridCell::FLAG_OUTSIDE, 0);
    }
}
