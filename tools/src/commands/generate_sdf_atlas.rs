use anyhow::{Context, Result};
use fontdue::{Font, FontSettings};
use image::{ImageBuffer, Rgba};
use rectangle_pack::{
    contains_smallest_box, pack_rects, volume_heuristic, GroupedRectsToPlace, RectToInsert,
    TargetBin,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Glyph metrics stored in the atlas metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlyphMetrics {
    /// Unicode codepoint
    pub codepoint: u32,
    /// Position in atlas (x, y, width, height) in pixels
    pub atlas_x: u32,
    pub atlas_y: u32,
    pub atlas_width: u32,
    pub atlas_height: u32,
    /// Horizontal advance (how far to move cursor after this glyph)
    pub advance: f32,
    /// Bearing (offset from baseline/cursor to glyph)
    pub bearing_x: f32,
    pub bearing_y: f32,
    /// Original glyph bounding box
    pub glyph_width: f32,
    pub glyph_height: f32,
}

/// Atlas metadata JSON format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtlasMetadata {
    /// Font name
    pub font_name: String,
    /// Atlas texture dimensions
    pub atlas_width: u32,
    pub atlas_height: u32,
    /// Font units per EM
    pub units_per_em: u16,
    /// Recommended line height
    pub line_height: f32,
    /// Ascender (above baseline)
    pub ascender: f32,
    /// Descender (below baseline, negative)
    pub descender: f32,
    /// SDF distance range in pixels
    pub sdf_range: f32,
    /// Glyph size in pixels (before padding)
    pub glyph_size: u32,
    /// Padding added around each glyph for SDF
    pub padding: u32,
    /// Per-glyph metrics, keyed by character
    pub glyphs: BTreeMap<char, GlyphMetrics>,
}

/// Configuration for atlas generation
pub struct AtlasConfig {
    /// Size of each glyph in pixels
    pub glyph_size: u32,
    /// Padding around each glyph for SDF range
    pub padding: u32,
    /// Characters to include in atlas
    pub charset: String,
}

impl Default for AtlasConfig {
    fn default() -> Self {
        // ASCII printable chars
        let charset: String = (32u8..=126u8).map(|c| c as char).collect();

        Self {
            glyph_size: 64,
            padding: 8,
            charset,
        }
    }
}

pub fn run(font_path: &str, output_png: &str, output_json: &str, config: AtlasConfig) -> Result<()> {
    println!("Loading font: {}", font_path);

    let font_data = std::fs::read(font_path)
        .with_context(|| format!("Failed to read font file: {}", font_path))?;

    // Load font with fontdue
    let font = Font::from_bytes(font_data.as_slice(), FontSettings::default())
        .map_err(|e| anyhow::anyhow!("Failed to parse font: {}", e))?;

    let font_name = font_path
        .split('/')
        .last()
        .unwrap_or("Unknown")
        .trim_end_matches(".ttf")
        .trim_end_matches(".otf")
        .to_string();

    println!("Font: {}", font_name);

    // Get font metrics
    let metrics = font.horizontal_line_metrics(config.glyph_size as f32);
    let (ascender, descender, line_height) = if let Some(m) = metrics {
        (m.ascent, m.descent, m.new_line_size)
    } else {
        let size = config.glyph_size as f32;
        (size * 0.8, -size * 0.2, size * 1.2)
    };

    println!("Line metrics: ascender={:.1}, descender={:.1}, line_height={:.1}",
             ascender, descender, line_height);

    // Generate glyphs
    let padded_size = config.glyph_size + config.padding * 2;
    let mut glyph_images: Vec<(char, ImageBuffer<Rgba<u8>, Vec<u8>>, GlyphMetrics)> = Vec::new();

    println!("Generating SDF for {} characters...", config.charset.len());

    for ch in config.charset.chars() {
        match generate_glyph_sdf(&font, ch, &config) {
            Ok(Some((img, metrics))) => {
                glyph_images.push((ch, img, metrics));
            }
            Ok(None) => {
                // Space or empty glyph - create placeholder
                let metrics_data = font.metrics(ch, config.glyph_size as f32);
                let img = ImageBuffer::from_fn(padded_size, padded_size, |_, _| {
                    Rgba([0, 0, 0, 0]) // Transparent
                });

                let metrics = GlyphMetrics {
                    codepoint: ch as u32,
                    atlas_x: 0,
                    atlas_y: 0,
                    atlas_width: padded_size,
                    atlas_height: padded_size,
                    advance: metrics_data.advance_width,
                    bearing_x: 0.0,
                    bearing_y: 0.0,
                    glyph_width: 0.0,
                    glyph_height: 0.0,
                };
                glyph_images.push((ch, img, metrics));
            }
            Err(e) => {
                eprintln!("  Warning: Failed to generate glyph for '{}': {}", ch, e);
            }
        }
    }

    println!("Generated {} glyphs", glyph_images.len());

    // Pack glyphs into atlas
    let (atlas_width, atlas_height, placements) = pack_glyphs(&glyph_images, padded_size)?;

    println!("Atlas size: {}x{}", atlas_width, atlas_height);

    // Create atlas image
    let mut atlas = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(atlas_width, atlas_height);

    // Fill with transparent
    for pixel in atlas.pixels_mut() {
        *pixel = Rgba([0, 0, 0, 0]);
    }

    // Build metadata
    let mut glyphs = BTreeMap::new();

    for (ch, img, mut metrics) in glyph_images {
        if let Some(&(x, y)) = placements.get(&ch) {
            // Copy glyph image to atlas
            for (gx, gy, pixel) in img.enumerate_pixels() {
                atlas.put_pixel(x + gx, y + gy, *pixel);
            }

            // Update metrics with atlas position
            metrics.atlas_x = x;
            metrics.atlas_y = y;
            metrics.atlas_width = img.width();
            metrics.atlas_height = img.height();

            glyphs.insert(ch, metrics);
        }
    }

    // Save atlas image
    atlas.save(output_png)
        .with_context(|| format!("Failed to save atlas image: {}", output_png))?;

    println!("Saved atlas: {}", output_png);

    // Save metadata JSON
    let metadata = AtlasMetadata {
        font_name,
        atlas_width,
        atlas_height,
        units_per_em: 1000, // fontdue normalizes to 1000
        line_height,
        ascender,
        descender,
        sdf_range: config.padding as f32,
        glyph_size: config.glyph_size,
        padding: config.padding,
        glyphs,
    };

    let json = serde_json::to_string_pretty(&metadata)
        .with_context(|| "Failed to serialize metadata")?;

    std::fs::write(output_json, json)
        .with_context(|| format!("Failed to save metadata: {}", output_json))?;

    println!("Saved metadata: {}", output_json);

    Ok(())
}

fn generate_glyph_sdf(
    font: &Font,
    ch: char,
    config: &AtlasConfig,
) -> Result<Option<(ImageBuffer<Rgba<u8>, Vec<u8>>, GlyphMetrics)>> {
    let padded_size = config.glyph_size + config.padding * 2;

    // Rasterize the glyph
    let (metrics, bitmap) = font.rasterize(ch, config.glyph_size as f32);

    // Skip empty glyphs
    if metrics.width == 0 || metrics.height == 0 {
        return Ok(None);
    }

    // Create SDF from the rasterized bitmap
    // We'll expand the bitmap with padding and compute a simple distance field
    let mut img = ImageBuffer::new(padded_size, padded_size);

    // Fill with transparent (outside)
    for pixel in img.pixels_mut() {
        *pixel = Rgba([0, 0, 0, 0]);
    }

    // Calculate offset to center the glyph in the padded area
    let glyph_center_x = config.padding as i32 + (config.glyph_size as i32 - metrics.width as i32) / 2;
    let glyph_center_y = config.padding as i32 + (config.glyph_size as i32 - metrics.height as i32) / 2;

    // Offset based on bearing
    let offset_x = glyph_center_x + metrics.xmin;
    let offset_y = config.padding as i32 + (ascender_estimate(config.glyph_size) - metrics.ymin - metrics.height as i32);

    // Copy the glyph bitmap and generate a simple "SDF" by using the alpha channel
    // For proper SDF, we'd compute distance to edge, but this is a simplified approach
    for gy in 0..metrics.height {
        for gx in 0..metrics.width {
            let src_idx = gy * metrics.width + gx;
            let coverage = bitmap[src_idx];

            let dst_x = offset_x + gx as i32;
            let dst_y = offset_y + gy as i32;

            if dst_x >= 0 && dst_x < padded_size as i32 &&
               dst_y >= 0 && dst_y < padded_size as i32 {
                // Store coverage as RGBA (white glyph with coverage-based alpha)
                // For SDF rendering, we'll use the alpha channel as the distance
                img.put_pixel(dst_x as u32, dst_y as u32, Rgba([255, 255, 255, coverage]));
            }
        }
    }

    // Now compute a proper SDF by propagating distances
    let sdf_img = compute_sdf(&img, config.padding);

    let glyph_metrics = GlyphMetrics {
        codepoint: ch as u32,
        atlas_x: 0,
        atlas_y: 0,
        atlas_width: padded_size,
        atlas_height: padded_size,
        advance: metrics.advance_width,
        bearing_x: metrics.xmin as f32,
        bearing_y: (metrics.ymin + metrics.height as i32) as f32,
        glyph_width: metrics.width as f32,
        glyph_height: metrics.height as f32,
    };

    Ok(Some((sdf_img, glyph_metrics)))
}

/// Estimate ascender height for glyph positioning
fn ascender_estimate(glyph_size: u32) -> i32 {
    (glyph_size as f32 * 0.8) as i32
}

/// Compute a signed distance field from a coverage bitmap
fn compute_sdf(img: &ImageBuffer<Rgba<u8>, Vec<u8>>, max_dist: u32) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let (width, height) = img.dimensions();
    let max_dist_f = max_dist as f32;

    let mut sdf = ImageBuffer::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let coverage = img.get_pixel(x, y)[3];
            let inside = coverage > 127;

            // Find distance to nearest edge (simple brute force for now)
            let mut min_dist = max_dist_f;

            let search_radius = max_dist as i32;
            for dy in -search_radius..=search_radius {
                for dx in -search_radius..=search_radius {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;

                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        let neighbor_coverage = img.get_pixel(nx as u32, ny as u32)[3];
                        let neighbor_inside = neighbor_coverage > 127;

                        if inside != neighbor_inside {
                            let dist = ((dx * dx + dy * dy) as f32).sqrt();
                            min_dist = min_dist.min(dist);
                        }
                    }
                }
            }

            // Signed distance: positive inside, negative outside
            let signed_dist = if inside { min_dist } else { -min_dist };

            // Map to [0, 255] with 128 being the edge
            let normalized = (signed_dist / max_dist_f + 1.0) / 2.0;
            let value = (normalized.clamp(0.0, 1.0) * 255.0) as u8;

            // Store as RGBA (copy to all channels for MTSDF compatibility)
            sdf.put_pixel(x, y, Rgba([value, value, value, value]));
        }
    }

    sdf
}

fn pack_glyphs(
    glyphs: &[(char, ImageBuffer<Rgba<u8>, Vec<u8>>, GlyphMetrics)],
    _glyph_size: u32,
) -> Result<(u32, u32, std::collections::HashMap<char, (u32, u32)>)> {
    use std::collections::HashMap;

    let mut rects_to_place: GroupedRectsToPlace<char, ()> = GroupedRectsToPlace::new();

    for (ch, img, _) in glyphs {
        rects_to_place.push_rect(
            *ch,
            None,
            RectToInsert::new(img.width(), img.height(), 1),
        );
    }

    let sizes = [256, 512, 1024, 2048, 4096];

    for &size in &sizes {
        let mut target_bins: BTreeMap<u32, TargetBin> = BTreeMap::new();
        target_bins.insert(0, TargetBin::new(size, size, 1));

        match pack_rects(
            &rects_to_place,
            &mut target_bins,
            &volume_heuristic,
            &contains_smallest_box,
        ) {
            Ok(placements) => {
                let mut result = HashMap::new();

                for (ch, (_bin, location)) in placements.packed_locations() {
                    result.insert(*ch, (location.x(), location.y()));
                }

                return Ok((size, size, result));
            }
            Err(_) => continue,
        }
    }

    anyhow::bail!("Failed to pack glyphs into atlas (max size 4096x4096)")
}
