//! MSDF Atlas loading
//!
//! Loads multi-channel signed distance field atlas from msdf-atlas-gen output.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Atlas metadata from msdf-atlas-gen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtlasInfo {
    /// Type of atlas (e.g., "msdf")
    #[serde(rename = "type")]
    pub atlas_type: Option<String>,
    /// Distance field range in pixels
    #[serde(rename = "distanceRange")]
    pub distance_range: f32,
    /// Distance range middle value
    #[serde(rename = "distanceRangeMiddle")]
    pub distance_range_middle: f32,
    /// Font size used for generation
    pub size: f32,
    /// Atlas width in pixels
    pub width: u32,
    /// Atlas height in pixels
    pub height: u32,
    /// Y-axis origin ("bottom" or "top")
    #[serde(rename = "yOrigin")]
    pub y_origin: String,
}

/// Font metrics from msdf-atlas-gen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontMetrics {
    /// Em size (usually 1.0)
    #[serde(rename = "emSize")]
    pub em_size: f32,
    /// Line height
    #[serde(rename = "lineHeight")]
    pub line_height: f32,
    /// Ascender height
    pub ascender: f32,
    /// Descender depth (negative)
    pub descender: f32,
    /// Underline Y position
    #[serde(rename = "underlineY")]
    pub underline_y: f32,
    /// Underline thickness
    #[serde(rename = "underlineThickness")]
    pub underline_thickness: f32,
}

/// Bounds rectangle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bounds {
    pub left: f32,
    pub bottom: f32,
    pub right: f32,
    pub top: f32,
}

/// Glyph data from msdf-atlas-gen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlyphData {
    /// Unicode codepoint
    pub unicode: u32,
    /// Horizontal advance width (in em units)
    pub advance: f32,
    /// Glyph bounds in em/plane coordinates (optional for space)
    #[serde(rename = "planeBounds")]
    pub plane_bounds: Option<Bounds>,
    /// Glyph bounds in atlas pixel coordinates (optional for space)
    #[serde(rename = "atlasBounds")]
    pub atlas_bounds: Option<Bounds>,
}

/// Kerning pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KerningPair {
    pub unicode1: u32,
    pub unicode2: u32,
    pub advance: f32,
}

/// Raw JSON structure from msdf-atlas-gen
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MsdfAtlasJson {
    atlas: AtlasInfo,
    metrics: FontMetrics,
    glyphs: Vec<GlyphData>,
    #[serde(default)]
    kerning: Vec<KerningPair>,
}

/// Metrics for a single glyph in the atlas (processed for easy use)
#[derive(Debug, Clone)]
pub struct GlyphMetrics {
    /// Unicode codepoint
    pub codepoint: u32,
    /// Advance width (normalized to em)
    pub advance: f32,
    /// UV coordinates in atlas (min_u, min_v, max_u, max_v)
    pub uvs: Option<(f32, f32, f32, f32)>,
    /// Plane bounds (left, bottom, right, top) in em units
    pub plane_bounds: Option<(f32, f32, f32, f32)>,
}

/// MSDF Atlas containing packed glyph textures and metrics
#[derive(Debug, Clone)]
pub struct MsdfAtlas {
    /// Atlas width in pixels
    pub width: u32,
    /// Atlas height in pixels
    pub height: u32,
    /// Distance field range in pixels
    pub distance_range: f32,
    /// Font metrics
    pub metrics: FontMetrics,
    /// Y-origin ("bottom" or "top")
    pub y_origin: String,
    /// Glyph metrics indexed by codepoint
    pub glyphs: HashMap<u32, GlyphMetrics>,
    /// Kerning pairs indexed by (first, second) unicode
    pub kerning: HashMap<(u32, u32), f32>,
}

impl MsdfAtlas {
    /// Load atlas from JSON file (msdf-atlas-gen format)
    pub fn load(json_path: &Path) -> Result<Self> {
        let json = fs::read_to_string(json_path)?;
        let raw: MsdfAtlasJson =
            serde_json::from_str(&json).context("Failed to parse msdf-atlas-gen JSON")?;

        let atlas_width = raw.atlas.width as f32;
        let atlas_height = raw.atlas.height as f32;
        let y_origin_bottom = raw.atlas.y_origin == "bottom";

        // Convert glyphs to our format
        let mut glyphs = HashMap::new();
        for glyph in &raw.glyphs {
            // Calculate UV coordinates from atlas bounds
            let uvs = glyph.atlas_bounds.as_ref().map(|ab| {
                let min_u = ab.left / atlas_width;
                let max_u = ab.right / atlas_width;

                // Handle Y-axis orientation
                let (min_v, max_v) = if y_origin_bottom {
                    // Bottom origin: flip V coordinates
                    let min_v = 1.0 - ab.top / atlas_height;
                    let max_v = 1.0 - ab.bottom / atlas_height;
                    (min_v, max_v)
                } else {
                    // Top origin: use as-is
                    (ab.top / atlas_height, ab.bottom / atlas_height)
                };

                (min_u, min_v, max_u, max_v)
            });

            let plane_bounds = glyph.plane_bounds.as_ref().map(|pb| {
                (pb.left, pb.bottom, pb.right, pb.top)
            });

            glyphs.insert(
                glyph.unicode,
                GlyphMetrics {
                    codepoint: glyph.unicode,
                    advance: glyph.advance,
                    uvs,
                    plane_bounds,
                },
            );
        }

        // Convert kerning pairs
        let mut kerning = HashMap::new();
        for kp in &raw.kerning {
            kerning.insert((kp.unicode1, kp.unicode2), kp.advance);
        }

        Ok(MsdfAtlas {
            width: raw.atlas.width,
            height: raw.atlas.height,
            distance_range: raw.atlas.distance_range,
            metrics: raw.metrics,
            y_origin: raw.atlas.y_origin,
            glyphs,
            kerning,
        })
    }

    /// Get glyph metrics for a character
    pub fn get_glyph(&self, ch: char) -> Option<&GlyphMetrics> {
        self.glyphs.get(&(ch as u32))
    }

    /// Get UV coordinates for a glyph (returns min_u, min_v, max_u, max_v)
    pub fn get_glyph_uvs(&self, ch: char) -> Option<(f32, f32, f32, f32)> {
        self.get_glyph(ch)?.uvs
    }

    /// Get kerning adjustment between two characters
    pub fn get_kerning(&self, first: char, second: char) -> f32 {
        self.kerning
            .get(&(first as u32, second as u32))
            .copied()
            .unwrap_or(0.0)
    }
}

/// Default ASCII charset for reference
pub const ASCII_CHARSET: &str =
    " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~";
