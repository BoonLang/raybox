//! MSDF Atlas generation and loading
//!
//! Generates a multi-channel signed distance field atlas from a font file.

use anyhow::{Context, Result};
use msdfgen::{Bitmap, FillRule, FontExt, Framing, MsdfGeneratorConfig, Rgb, Shape};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use ttf_parser::Face;

/// Size of each glyph cell in the atlas (pixels)
pub const GLYPH_SIZE: u32 = 64;

/// Padding around each glyph (pixels)
pub const GLYPH_PADDING: u32 = 4;

/// Distance field range (in pixels)
pub const SDF_RANGE: f64 = 4.0;

/// Metrics for a single glyph in the atlas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlyphMetrics {
    /// Unicode codepoint
    pub codepoint: u32,
    /// Position in atlas (column, row)
    pub atlas_x: u32,
    pub atlas_y: u32,
    /// Glyph bounding box (normalized to glyph cell)
    pub bbox_left: f32,
    pub bbox_bottom: f32,
    pub bbox_right: f32,
    pub bbox_top: f32,
    /// Advance width (normalized to em)
    pub advance: f32,
    /// Bearing (offset from baseline)
    pub bearing_x: f32,
    pub bearing_y: f32,
}

/// MSDF Atlas containing packed glyph textures and metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsdfAtlas {
    /// Atlas width in pixels
    pub width: u32,
    /// Atlas height in pixels
    pub height: u32,
    /// Number of glyphs per row
    pub glyphs_per_row: u32,
    /// Glyph cell size in pixels
    pub glyph_size: u32,
    /// Font units per em
    pub units_per_em: f32,
    /// Glyph metrics indexed by codepoint
    pub glyphs: HashMap<u32, GlyphMetrics>,
}

impl MsdfAtlas {
    /// Generate an MSDF atlas from a font file
    pub fn generate(font_data: &[u8], charset: &str) -> Result<(Self, Vec<u8>)> {
        let face = Face::parse(font_data, 0).context("Failed to parse font")?;
        let units_per_em = face.units_per_em() as f32;

        // Calculate atlas dimensions
        let glyph_count = charset.chars().count();
        let glyphs_per_row = (glyph_count as f32).sqrt().ceil() as u32;
        let rows = ((glyph_count as u32) + glyphs_per_row - 1) / glyphs_per_row;
        let atlas_width = glyphs_per_row * GLYPH_SIZE;
        let atlas_height = rows * GLYPH_SIZE;

        // Create atlas bitmap (RGB for MSDF)
        let mut atlas_data = vec![0u8; (atlas_width * atlas_height * 3) as usize];
        let mut glyphs = HashMap::new();

        let config = MsdfGeneratorConfig::default();

        for (idx, ch) in charset.chars().enumerate() {
            let col = (idx as u32) % glyphs_per_row;
            let row = (idx as u32) / glyphs_per_row;

            let glyph_id = match face.glyph_index(ch) {
                Some(id) => id,
                None => continue,
            };

            // Get glyph shape using FontExt trait
            let mut shape: Shape = match face.glyph_shape(glyph_id) {
                Some(s) => s,
                None => continue,
            };

            // Get glyph metrics
            let h_advance = face.glyph_hor_advance(glyph_id).unwrap_or(0) as f32;
            let bbox = face.glyph_bounding_box(glyph_id);

            let (bbox_left, bbox_bottom, bbox_right, bbox_top) = if let Some(b) = bbox {
                (
                    b.x_min as f32 / units_per_em,
                    b.y_min as f32 / units_per_em,
                    b.x_max as f32 / units_per_em,
                    b.y_max as f32 / units_per_em,
                )
            } else {
                (0.0, 0.0, 0.0, 0.0)
            };

            // Edge coloring for MSDF
            shape.edge_coloring_simple(3.0, 0);

            // Calculate framing (fit glyph in cell with padding)
            let padded_size = GLYPH_SIZE - GLYPH_PADDING * 2;
            let bounds = shape.get_bound();

            // Check if shape has any geometry
            let framing = if bounds.right > bounds.left && bounds.top > bounds.bottom {
                let width = bounds.right - bounds.left;
                let height = bounds.top - bounds.bottom;
                let max_dim = width.max(height);
                let scale = padded_size as f64 / max_dim;

                // Center the glyph in the cell
                let translate_x = GLYPH_PADDING as f64 - bounds.left * scale
                    + (padded_size as f64 - width * scale) / 2.0;
                let translate_y = GLYPH_PADDING as f64 - bounds.bottom * scale
                    + (padded_size as f64 - height * scale) / 2.0;

                // Framing::new takes (range, scale, translate)
                Framing::new(
                    SDF_RANGE,
                    scale,
                    msdfgen::Vector2::new(translate_x, translate_y),
                )
            } else {
                // Empty glyph (like space)
                let scale = padded_size as f64 / units_per_em as f64;
                Framing::new(
                    SDF_RANGE,
                    scale,
                    msdfgen::Vector2::new(GLYPH_PADDING as f64, GLYPH_PADDING as f64),
                )
            };

            // Generate MSDF
            let mut glyph_bitmap: Bitmap<Rgb<f32>> = Bitmap::new(GLYPH_SIZE, GLYPH_SIZE);

            shape.generate_msdf(&mut glyph_bitmap, &framing, &config);
            shape.correct_sign(&mut glyph_bitmap, &framing, FillRule::NonZero);

            // Copy glyph to atlas
            let atlas_offset_x = col * GLYPH_SIZE;
            let atlas_offset_y = row * GLYPH_SIZE;

            for y in 0..GLYPH_SIZE {
                for x in 0..GLYPH_SIZE {
                    let pixel = glyph_bitmap.pixel(x, y);
                    let atlas_idx =
                        ((atlas_offset_y + y) * atlas_width + (atlas_offset_x + x)) * 3;
                    let atlas_idx = atlas_idx as usize;

                    // Convert from [-range, range] to [0, 255]
                    let r =
                        ((pixel.r / SDF_RANGE as f32 + 0.5) * 255.0).clamp(0.0, 255.0) as u8;
                    let g =
                        ((pixel.g / SDF_RANGE as f32 + 0.5) * 255.0).clamp(0.0, 255.0) as u8;
                    let b =
                        ((pixel.b / SDF_RANGE as f32 + 0.5) * 255.0).clamp(0.0, 255.0) as u8;

                    atlas_data[atlas_idx] = r;
                    atlas_data[atlas_idx + 1] = g;
                    atlas_data[atlas_idx + 2] = b;
                }
            }

            glyphs.insert(
                ch as u32,
                GlyphMetrics {
                    codepoint: ch as u32,
                    atlas_x: col,
                    atlas_y: row,
                    bbox_left,
                    bbox_bottom,
                    bbox_right,
                    bbox_top,
                    advance: h_advance / units_per_em,
                    bearing_x: bbox_left,
                    bearing_y: bbox_top,
                },
            );
        }

        let atlas = MsdfAtlas {
            width: atlas_width,
            height: atlas_height,
            glyphs_per_row,
            glyph_size: GLYPH_SIZE,
            units_per_em,
            glyphs,
        };

        Ok((atlas, atlas_data))
    }

    /// Save atlas to PNG and JSON files
    pub fn save(&self, atlas_data: &[u8], output_dir: &Path) -> Result<()> {
        fs::create_dir_all(output_dir)?;

        // Save PNG
        let png_path = output_dir.join("atlas.png");
        image::save_buffer(
            &png_path,
            atlas_data,
            self.width,
            self.height,
            image::ColorType::Rgb8,
        )
        .context("Failed to save atlas PNG")?;

        // Save JSON metrics
        let json_path = output_dir.join("atlas.json");
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&json_path, json)?;

        log::info!(
            "Saved MSDF atlas: {}x{} with {} glyphs",
            self.width,
            self.height,
            self.glyphs.len()
        );

        Ok(())
    }

    /// Load atlas from JSON file
    pub fn load(json_path: &Path) -> Result<Self> {
        let json = fs::read_to_string(json_path)?;
        let atlas: MsdfAtlas = serde_json::from_str(&json)?;
        Ok(atlas)
    }

    /// Get glyph metrics for a character
    pub fn get_glyph(&self, ch: char) -> Option<&GlyphMetrics> {
        self.glyphs.get(&(ch as u32))
    }

    /// Get UV coordinates for a glyph (returns min_u, min_v, max_u, max_v)
    pub fn get_glyph_uvs(&self, ch: char) -> Option<(f32, f32, f32, f32)> {
        let glyph = self.get_glyph(ch)?;
        let cell_u = self.glyph_size as f32 / self.width as f32;
        let cell_v = self.glyph_size as f32 / self.height as f32;

        let min_u = glyph.atlas_x as f32 * cell_u;
        let min_v = glyph.atlas_y as f32 * cell_v;
        let max_u = min_u + cell_u;
        let max_v = min_v + cell_v;

        Some((min_u, min_v, max_u, max_v))
    }
}

/// Default ASCII charset for atlas generation
pub const ASCII_CHARSET: &str =
    " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~";
