//! Font atlas loading and text layout for MTSDF text rendering
//!
//! This module provides:
//! - Loading of MTSDF atlas metadata (JSON)
//! - Glyph lookup and metrics
//! - Text layout calculation

use std::collections::BTreeMap;

/// Glyph metrics from the atlas metadata
#[derive(Debug, Clone)]
pub struct GlyphMetrics {
    /// Position in atlas (x, y, width, height) in pixels
    pub atlas_x: f32,
    pub atlas_y: f32,
    pub atlas_width: f32,
    pub atlas_height: f32,
    /// Horizontal advance (how far to move cursor after this glyph)
    pub advance: f32,
    /// Bearing (offset from baseline/cursor to glyph)
    pub bearing_x: f32,
    pub bearing_y: f32,
    /// Original glyph bounding box dimensions
    pub glyph_width: f32,
    pub glyph_height: f32,
}

/// Font atlas data for MTSDF text rendering
#[derive(Debug)]
pub struct FontAtlas {
    /// Font name
    pub font_name: String,
    /// Atlas texture dimensions
    pub atlas_width: f32,
    pub atlas_height: f32,
    /// Font units per EM
    pub units_per_em: u16,
    /// Recommended line height (at glyph_size)
    pub line_height: f32,
    /// Ascender (above baseline)
    pub ascender: f32,
    /// Descender (below baseline, negative)
    pub descender: f32,
    /// SDF distance range in pixels
    pub sdf_range: f32,
    /// Glyph size in pixels (before padding)
    pub glyph_size: f32,
    /// Padding added around each glyph for SDF
    pub padding: f32,
    /// Per-glyph metrics, keyed by character
    pub glyphs: BTreeMap<char, GlyphMetrics>,
}

/// Positioned glyph for text rendering
#[derive(Debug, Clone)]
pub struct PositionedGlyph {
    /// Character being rendered
    pub char: char,
    /// Position in screen space (top-left)
    pub x: f32,
    pub y: f32,
    /// Size in screen space
    pub width: f32,
    pub height: f32,
    /// UV coordinates in atlas (0-1 range)
    pub uv_x: f32,
    pub uv_y: f32,
    pub uv_width: f32,
    pub uv_height: f32,
}

/// Text layout result
#[derive(Debug)]
pub struct TextLayout {
    /// Positioned glyphs
    pub glyphs: Vec<PositionedGlyph>,
    /// Total width of the text
    pub total_width: f32,
    /// Total height of the text (line height)
    pub total_height: f32,
}

impl FontAtlas {
    /// Load font atlas from JSON metadata
    /// This is designed to be called at runtime with the JSON content
    pub fn from_json(json: &str) -> Result<Self, String> {
        // Parse JSON manually to avoid serde dependency in WASM
        let parsed: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| format!("Failed to parse font atlas JSON: {}", e))?;

        let font_name = parsed["font_name"]
            .as_str()
            .unwrap_or("Unknown")
            .to_string();

        let atlas_width = parsed["atlas_width"].as_f64().unwrap_or(512.0) as f32;
        let atlas_height = parsed["atlas_height"].as_f64().unwrap_or(512.0) as f32;
        let units_per_em = parsed["units_per_em"].as_u64().unwrap_or(2048) as u16;
        let line_height = parsed["line_height"].as_f64().unwrap_or(40.0) as f32;
        let ascender = parsed["ascender"].as_f64().unwrap_or(32.0) as f32;
        let descender = parsed["descender"].as_f64().unwrap_or(-8.0) as f32;
        let sdf_range = parsed["sdf_range"].as_f64().unwrap_or(4.0) as f32;
        let glyph_size = parsed["glyph_size"].as_f64().unwrap_or(32.0) as f32;
        let padding = parsed["padding"].as_f64().unwrap_or(4.0) as f32;

        let mut glyphs = BTreeMap::new();

        if let Some(glyphs_obj) = parsed["glyphs"].as_object() {
            for (key, value) in glyphs_obj {
                // Key is the character as a string
                if let Some(ch) = key.chars().next() {
                    let metrics = GlyphMetrics {
                        atlas_x: value["atlas_x"].as_f64().unwrap_or(0.0) as f32,
                        atlas_y: value["atlas_y"].as_f64().unwrap_or(0.0) as f32,
                        atlas_width: value["atlas_width"].as_f64().unwrap_or(40.0) as f32,
                        atlas_height: value["atlas_height"].as_f64().unwrap_or(40.0) as f32,
                        advance: value["advance"].as_f64().unwrap_or(10.0) as f32,
                        bearing_x: value["bearing_x"].as_f64().unwrap_or(0.0) as f32,
                        bearing_y: value["bearing_y"].as_f64().unwrap_or(0.0) as f32,
                        glyph_width: value["glyph_width"].as_f64().unwrap_or(0.0) as f32,
                        glyph_height: value["glyph_height"].as_f64().unwrap_or(0.0) as f32,
                    };
                    glyphs.insert(ch, metrics);
                }
            }
        }

        Ok(Self {
            font_name,
            atlas_width,
            atlas_height,
            units_per_em,
            line_height,
            ascender,
            descender,
            sdf_range,
            glyph_size,
            padding,
            glyphs,
        })
    }

    /// Get metrics for a character, falling back to space if not found
    pub fn get_glyph(&self, ch: char) -> Option<&GlyphMetrics> {
        self.glyphs.get(&ch).or_else(|| self.glyphs.get(&' '))
    }

    /// Calculate the width of a string at a given font size
    pub fn measure_text(&self, text: &str, font_size: f32) -> f32 {
        let scale = font_size / self.glyph_size;
        let mut width = 0.0;

        for ch in text.chars() {
            if let Some(glyph) = self.get_glyph(ch) {
                width += glyph.advance * scale;
            }
        }

        width
    }

    /// Layout text at a given position and size
    /// Returns positioned glyphs ready for rendering
    pub fn layout_text(
        &self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
    ) -> TextLayout {
        let scale = font_size / self.glyph_size;
        let mut glyphs = Vec::new();
        let mut cursor_x = x;
        let baseline_y = y + self.ascender * scale;

        for ch in text.chars() {
            if let Some(glyph) = self.get_glyph(ch) {
                // Calculate glyph position
                // Atlas cells are uniform 80x80 (glyph_size=64 + padding=8 on each side)
                //
                // CRITICAL: The baseline must be at a FIXED position for all glyphs
                // to ensure proper baseline alignment. The atlas stores all glyphs
                // with their baseline at the same Y position within each cell.
                //
                // baseline_in_cell = padding + ascender = 8 + 62 = 70
                // This positions the baseline consistently so all letters align.
                let baseline_in_cell = self.padding + self.ascender;

                // Account for glyph centering in the cell horizontally
                let glyph_x_in_cell = (glyph.atlas_width - glyph.glyph_width) / 2.0;
                let glyph_x = cursor_x + glyph.bearing_x * scale - glyph_x_in_cell * scale;
                let glyph_y = baseline_y - baseline_in_cell * scale;
                let glyph_w = glyph.atlas_width * scale;
                let glyph_h = glyph.atlas_height * scale;

                // UV coordinates in atlas (0-1 range)
                let uv_x = glyph.atlas_x / self.atlas_width;
                let uv_y = glyph.atlas_y / self.atlas_height;
                let uv_width = glyph.atlas_width / self.atlas_width;
                let uv_height = glyph.atlas_height / self.atlas_height;

                glyphs.push(PositionedGlyph {
                    char: ch,
                    x: glyph_x,
                    y: glyph_y,
                    width: glyph_w,
                    height: glyph_h,
                    uv_x,
                    uv_y,
                    uv_width,
                    uv_height,
                });

                cursor_x += glyph.advance * scale;
            }
        }

        let total_width = cursor_x - x;
        let total_height = self.line_height * scale;

        TextLayout {
            glyphs,
            total_width,
            total_height,
        }
    }

    /// Layout text centered at a given position
    pub fn layout_text_centered(
        &self,
        text: &str,
        center_x: f32,
        center_y: f32,
        font_size: f32,
    ) -> TextLayout {
        let width = self.measure_text(text, font_size);
        let height = self.line_height * (font_size / self.glyph_size);

        self.layout_text(
            text,
            center_x - width / 2.0,
            center_y - height / 2.0,
            font_size,
        )
    }
}

// Embed the Inter atlas JSON at compile time
pub const INTER_ATLAS_JSON: &str = include_str!("../assets/fonts/inter_sdf_atlas.json");

/// Load the embedded Inter font atlas
pub fn load_inter_atlas() -> Result<FontAtlas, String> {
    FontAtlas::from_json(INTER_ATLAS_JSON)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_inter_atlas() {
        let atlas = load_inter_atlas().expect("Failed to load Inter atlas");
        assert_eq!(atlas.font_name, "Inter-Light");
        assert!(atlas.glyphs.contains_key(&'A'));
        assert!(atlas.glyphs.contains_key(&'t'));
    }

    #[test]
    fn test_measure_text() {
        let atlas = load_inter_atlas().expect("Failed to load Inter atlas");
        let width = atlas.measure_text("todos", 100.0);
        assert!(width > 0.0);
    }

    #[test]
    fn test_layout_text() {
        let atlas = load_inter_atlas().expect("Failed to load Inter atlas");
        let layout = atlas.layout_text("Hi", 0.0, 0.0, 32.0);
        assert_eq!(layout.glyphs.len(), 2);
    }
}
