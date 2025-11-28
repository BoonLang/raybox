use anyhow::{Context, Result};
use image::{ImageBuffer, Rgba};
use mint::Vector2;
use msdf::{GlyphLoader, MSDFConfig, Projection, SDFTrait};
use rectangle_pack::{
    contains_smallest_box, pack_rects, volume_heuristic, GroupedRectsToPlace, RectToInsert,
    TargetBin,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

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
    /// Original glyph bounding box (before MTSDF padding)
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
    /// Size of each glyph in pixels (before padding)
    pub glyph_size: u32,
    /// Padding around each glyph for SDF range
    pub padding: u32,
    /// SDF distance range
    pub sdf_range: f32,
    /// Characters to include in atlas
    pub charset: String,
}

impl Default for AtlasConfig {
    fn default() -> Self {
        // ASCII printable chars plus common punctuation
        let charset: String = (32u8..=126u8).map(|c| c as char).collect();

        Self {
            glyph_size: 64,
            padding: 8,
            sdf_range: 8.0,
            charset,
        }
    }
}

pub fn run(font_path: &str, output_png: &str, output_json: &str, config: AtlasConfig) -> Result<()> {
    println!("Loading font: {}", font_path);

    let font_data = std::fs::read(font_path)
        .with_context(|| format!("Failed to read font file: {}", font_path))?;

    let face = ttf_parser::Face::from_slice(&font_data, 0)
        .with_context(|| "Failed to parse font file")?;

    let font_name = face
        .names()
        .into_iter()
        .find(|n| n.name_id == ttf_parser::name_id::FULL_NAME)
        .and_then(|n| n.to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    println!("Font: {} (units_per_em: {})", font_name, face.units_per_em());

    let units_per_em = face.units_per_em();
    let scale = config.glyph_size as f64 / units_per_em as f64;

    // Generate MTSDF for each character
    let padded_size = config.glyph_size + config.padding * 2;
    let mut glyph_images: Vec<(char, ImageBuffer<Rgba<u8>, Vec<u8>>, GlyphMetrics)> = Vec::new();

    println!("Generating MTSDF for {} characters...", config.charset.len());

    for ch in config.charset.chars() {
        if let Some(glyph_id) = face.glyph_index(ch) {
            match generate_glyph_mtsdf(&face, glyph_id, ch, &config, scale) {
                Ok(Some((img, metrics))) => {
                    glyph_images.push((ch, img, metrics));
                }
                Ok(None) => {
                    // Space or empty glyph - still track metrics
                    let h_advance = face.glyph_hor_advance(glyph_id).unwrap_or(0) as f32;
                    let scale_f32 = scale as f32;

                    // Create empty glyph image for spaces
                    let img = ImageBuffer::from_fn(padded_size, padded_size, |_, _| {
                        Rgba([128, 128, 128, 128]) // Neutral SDF value (on the edge)
                    });

                    let metrics = GlyphMetrics {
                        codepoint: ch as u32,
                        atlas_x: 0,
                        atlas_y: 0,
                        atlas_width: padded_size,
                        atlas_height: padded_size,
                        advance: h_advance * scale_f32,
                        bearing_x: 0.0,
                        bearing_y: 0.0,
                        glyph_width: 0.0,
                        glyph_height: 0.0,
                    };
                    glyph_images.push((ch, img, metrics));
                }
                Err(e) => {
                    eprintln!("  Warning: Failed to generate glyph for '{}' (U+{:04X}): {}",
                             ch, ch as u32, e);
                }
            }
        } else {
            eprintln!("  Warning: No glyph for '{}' (U+{:04X})", ch, ch as u32);
        }
    }

    println!("Generated {} glyphs", glyph_images.len());

    // Pack glyphs into atlas
    let (atlas_width, atlas_height, placements) = pack_glyphs(&glyph_images, padded_size)?;

    println!("Atlas size: {}x{}", atlas_width, atlas_height);

    // Create atlas image
    let mut atlas = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(atlas_width, atlas_height);

    // Fill with transparent black
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
        units_per_em,
        line_height: (face.ascender() - face.descender()) as f32 * scale as f32,
        ascender: face.ascender() as f32 * scale as f32,
        descender: face.descender() as f32 * scale as f32,
        sdf_range: config.sdf_range,
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

fn generate_glyph_mtsdf(
    face: &ttf_parser::Face,
    glyph_id: ttf_parser::GlyphId,
    ch: char,
    config: &AtlasConfig,
    scale: f64,
) -> Result<Option<(ImageBuffer<Rgba<u8>, Vec<u8>>, GlyphMetrics)>> {
    let padded_size = config.glyph_size + config.padding * 2;

    // Get horizontal metrics
    let h_advance = face.glyph_hor_advance(glyph_id).unwrap_or(0) as f32;
    let scale_f32 = scale as f32;

    // Get glyph bounding box
    let bbox = face.glyph_bounding_box(glyph_id);
    let (glyph_width, glyph_height, bearing_x, bearing_y) = if let Some(bbox) = bbox {
        (
            (bbox.x_max - bbox.x_min) as f32 * scale_f32,
            (bbox.y_max - bbox.y_min) as f32 * scale_f32,
            bbox.x_min as f32 * scale_f32,
            bbox.y_max as f32 * scale_f32,
        )
    } else {
        // No bounding box - likely a space or empty glyph
        return Ok(None);
    };

    // Load shape from font using GlyphLoader trait
    let shape = match GlyphLoader::load_shape(face, glyph_id) {
        Some(s) => s,
        None => return Ok(None), // Empty glyph
    };

    // Color edges for MSDF (angle threshold in radians, 3.0 ~ 172 degrees)
    let colored = shape.color_edges_simple(3.0);

    // Create projection (scale and translate)
    // Font coordinates: Y goes UP, origin at baseline
    // Image coordinates: Y goes DOWN, origin at top-left
    // We need to flip Y by using negative scale and adjusting translation

    // Get the original (unscaled) bearing values for projection calculation
    let bbox = face.glyph_bounding_box(glyph_id).unwrap();
    let orig_bearing_x = bbox.x_min as f64;
    let orig_bearing_y = bbox.y_max as f64;

    // Translation positions the glyph within the padded output image
    // X: padding offset minus the left bearing (so glyph starts at padding)
    // Y: Use negative scale to flip, and position so top of glyph is at padding
    let translate_x = config.padding as f64 - orig_bearing_x * scale;
    let translate_y = config.padding as f64 + orig_bearing_y * scale;

    let projection = Projection {
        scale: Vector2 { x: scale, y: -scale }, // Negative Y to flip
        translation: Vector2 { x: translate_x, y: translate_y },
    };

    // Configure MTSDF generation
    let msdf_config = MSDFConfig::default();

    // Generate MTSDF (4-channel: RGB for corners, A for true distance)
    let mtsdf = colored.generate_mtsdf(
        padded_size,
        padded_size,
        config.sdf_range as f64,
        &projection,
        &msdf_config,
    );

    // Convert to image using SDFTrait::image()
    // Pass the scale factor to convert font units to pixel units
    let img = mtsdf_to_u8_image(&mtsdf, config.sdf_range as f64, scale, ch == 'A' || ch == 't');

    let metrics = GlyphMetrics {
        codepoint: ch as u32,
        atlas_x: 0,
        atlas_y: 0,
        atlas_width: padded_size,
        atlas_height: padded_size,
        advance: h_advance * scale_f32,
        bearing_x,
        bearing_y,
        glyph_width,
        glyph_height,
    };

    Ok(Some((img, metrics)))
}

fn mtsdf_to_u8_image(mtsdf: &msdf::MTSDF, sdf_range_pixels: f64, scale: f64, debug: bool) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    // Get the underlying f32 image
    let f32_img = mtsdf.image();
    let (width, height) = f32_img.dimensions();

    // The msdf values are in font units. Convert to pixels using scale.
    // Then use sdf_range_pixels as the range for mapping.
    //
    // For standard MSDF convention:
    // - Positive distance = OUTSIDE the glyph
    // - Negative distance = INSIDE the glyph
    // - 0 = on the edge
    //
    // For rendering, we want:
    // - 0.5 (128) = edge
    // - >0.5 = inside (should be rendered)
    // - <0.5 = outside (should be discarded)
    //
    // So we need to INVERT the sign: output = -input

    // Convert sdf_range from pixels to font units for proper mapping
    let sdf_range_font_units = sdf_range_pixels / scale;

    if debug {
        let mut min_val = f32::MAX;
        let mut max_val = f32::MIN;
        for y in 0..height {
            for x in 0..width {
                let pixel = f32_img.get_pixel(x, y);
                for i in 0..4 {
                    min_val = min_val.min(pixel[i]);
                    max_val = max_val.max(pixel[i]);
                }
            }
        }
        let min_px = min_val as f64 * scale;
        let max_px = max_val as f64 * scale;
        println!("    MTSDF: font_units=[{:.1}, {:.1}], pixels=[{:.1}, {:.1}], range={:.1}px",
                 min_val, max_val, min_px, max_px, sdf_range_pixels);
    }

    let mut img = ImageBuffer::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let pixel = f32_img.get_pixel(x, y);

            // Map distance from font units to [0, 255]
            // Invert sign so inside = high value, outside = low value
            let map_channel = |v: f32| -> u8 {
                // Invert: inside (negative) becomes positive
                let inverted = -v;
                // Normalize to [-range, +range] -> [0, 1]
                let normalized = (inverted as f64 / sdf_range_font_units + 1.0) / 2.0;
                (normalized.clamp(0.0, 1.0) * 255.0) as u8
            };

            img.put_pixel(x, y, Rgba([
                map_channel(pixel[0]),
                map_channel(pixel[1]),
                map_channel(pixel[2]),
                map_channel(pixel[3]),
            ]));
        }
    }

    img
}

fn pack_glyphs(
    glyphs: &[(char, ImageBuffer<Rgba<u8>, Vec<u8>>, GlyphMetrics)],
    _glyph_size: u32,
) -> Result<(u32, u32, std::collections::HashMap<char, (u32, u32)>)> {
    use std::collections::HashMap;

    // Create rectangles to pack
    let mut rects_to_place: GroupedRectsToPlace<char, ()> = GroupedRectsToPlace::new();

    for (ch, img, _) in glyphs {
        rects_to_place.push_rect(
            *ch,
            None,
            RectToInsert::new(img.width(), img.height(), 1),
        );
    }

    // Try different atlas sizes until we find one that fits
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

/// Run with default configuration for Inter font
pub fn run_default(font_path: &str, output_dir: &str) -> Result<()> {
    let output_dir = Path::new(output_dir);
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("Failed to create output directory: {:?}", output_dir))?;

    let output_png = output_dir.join("inter_mtsdf_atlas.png");
    let output_json = output_dir.join("inter_mtsdf_atlas.json");

    run(
        font_path,
        output_png.to_str().unwrap(),
        output_json.to_str().unwrap(),
        AtlasConfig::default(),
    )
}
