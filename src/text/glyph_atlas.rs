//! Packed glyph atlas for GPU SDF text rendering.
//!
//! Stores the shared curve list plus per-glyph metadata used by the active
//! brute-force glyph evaluation path.

use super::vector_font::{BezierCurve, VectorFont, VectorGlyphMetrics};
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;

/// Metadata for one glyph in the packed atlas.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable, Default)]
pub struct GlyphAtlasEntry {
    /// Glyph bounds [left, bottom, right, top] in em units.
    pub bounds: [f32; 4],
    /// Advance width in em units.
    pub advance: f32,
    /// Offset into the packed curve array.
    pub curve_offset: u32,
    /// Number of curves for the glyph.
    pub curve_count: u32,
}

/// Complete vector font atlas with densely packed curves and glyph metadata.
pub struct VectorFontAtlas {
    /// All curves from all glyphs, densely packed.
    pub curves: Vec<BezierCurve>,
    /// Per-glyph metadata, indexed by codepoint.
    pub glyphs: HashMap<u32, GlyphAtlasEntry>,
    /// Ordered list of glyph entries for GPU upload.
    #[allow(dead_code)]
    pub glyph_list: Vec<(u32, GlyphAtlasEntry)>,
    /// Font metrics.
    pub ascender: f32,
    #[allow(dead_code)]
    pub descender: f32,
    #[allow(dead_code)]
    pub line_height: f32,
}

impl VectorFontAtlas {
    /// Build an atlas from a vector font.
    pub fn from_font(font: &VectorFont) -> Self {
        let mut atlas = VectorFontAtlas {
            curves: Vec::new(),
            glyphs: HashMap::new(),
            glyph_list: Vec::new(),
            ascender: font.ascender,
            descender: font.descender,
            line_height: font.line_height,
        };

        for (&codepoint, glyph_metrics) in &font.glyphs {
            atlas.add_glyph(font, codepoint, glyph_metrics);
        }

        atlas.glyph_list.sort_by_key(|(cp, _)| *cp);
        atlas
    }

    fn add_glyph(&mut self, font: &VectorFont, codepoint: u32, metrics: &VectorGlyphMetrics) {
        let glyph_curves = font.get_glyph_curves(metrics);
        let curve_offset = self.curves.len() as u32;
        self.curves.extend_from_slice(glyph_curves);

        let entry = GlyphAtlasEntry {
            bounds: metrics.bounds,
            advance: metrics.advance,
            curve_offset,
            curve_count: metrics.curve_count,
        };

        self.glyphs.insert(codepoint, entry);
        self.glyph_list.push((codepoint, entry));
    }

    /// Get glyph entry by character.
    #[allow(dead_code)]
    pub fn get_glyph(&self, ch: char) -> Option<&GlyphAtlasEntry> {
        self.glyphs.get(&(ch as u32))
    }

    /// Get total size needed for the curve buffer in bytes.
    #[allow(dead_code)]
    pub fn curve_buffer_size(&self) -> usize {
        self.curves.len() * std::mem::size_of::<BezierCurve>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_entry_is_pod_like() {
        let entry = GlyphAtlasEntry {
            bounds: [1.0, 2.0, 3.0, 4.0],
            advance: 5.0,
            curve_offset: 6,
            curve_count: 7,
        };
        assert_eq!(entry.bounds[0], 1.0);
        assert_eq!(entry.advance, 5.0);
        assert_eq!(entry.curve_offset, 6);
        assert_eq!(entry.curve_count, 7);
    }
}
