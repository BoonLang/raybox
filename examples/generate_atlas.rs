//! Generate MSDF atlas from a font file
//!
//! Usage: cargo run --example generate_atlas

use anyhow::Result;
use raybox::text::atlas::{MsdfAtlas, ASCII_CHARSET};
use std::path::Path;

fn main() -> Result<()> {
    env_logger::init();

    // Load font
    let font_path = Path::new("assets/fonts/DejaVuSans.ttf");
    let font_data = std::fs::read(font_path)?;

    println!("Generating MSDF atlas from {:?}...", font_path);

    // Generate atlas
    let (atlas, atlas_data) = MsdfAtlas::generate(&font_data, ASCII_CHARSET)?;

    println!(
        "Generated atlas: {}x{} with {} glyphs",
        atlas.width,
        atlas.height,
        atlas.glyphs.len()
    );

    // Save atlas
    let output_dir = Path::new("assets/fonts");
    atlas.save(&atlas_data, output_dir)?;

    println!("Saved atlas to {:?}", output_dir);

    Ok(())
}
