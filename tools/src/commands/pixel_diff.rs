use anyhow::{Context, Result};
use image::{GenericImage, GenericImageView};
use std::path::Path;

pub fn run(reference: &str, current: &str, output: Option<&str>, threshold: f64) -> Result<()> {
    let ref_img = image::open(Path::new(reference))
        .with_context(|| format!("Failed to open reference image: {}", reference))?;
    let cur_img = image::open(Path::new(current))
        .with_context(|| format!("Failed to open current image: {}", current))?;

    if ref_img.dimensions() != cur_img.dimensions() {
        anyhow::bail!(
            "Dimension mismatch: reference {:?}, current {:?}",
            ref_img.dimensions(),
            cur_img.dimensions()
        );
    }

    let (w, h) = ref_img.dimensions();
    let mut equal = 0u64;
    let total = (w as u64) * (h as u64);

    for y in 0..h {
        for x in 0..w {
            if ref_img.get_pixel(x, y) == cur_img.get_pixel(x, y) {
                equal += 1;
            }
        }
    }

    let score = (equal as f64) / (total as f64);
    println!(
        "Exact pixel match: {:.4}% (threshold {:.4}%)",
        score * 100.0,
        threshold * 100.0
    );
    if score < threshold {
        if let Some(out) = output {
            // simple XOR diff visualization
            let mut diff = image::DynamicImage::new_rgba8(w, h);
            for y in 0..h {
                for x in 0..w {
                    let a = ref_img.get_pixel(x, y);
                    let b = cur_img.get_pixel(x, y);
                    let dr = (a[0] ^ b[0]) as u8;
                    let dg = (a[1] ^ b[1]) as u8;
                    let db = (a[2] ^ b[2]) as u8;
                    diff.put_pixel(x, y, image::Rgba([dr, dg, db, 255]));
                }
            }
            diff.save(out)
                .with_context(|| format!("Failed to write diff image: {}", out))?;
            println!("Wrote diff image to {}", out);
        }
        anyhow::bail!("Pixel diff below threshold");
    }
    Ok(())
}

pub fn run_simple(reference: &str, candidate: &str, threshold: f64) -> Result<()> {
    run(reference, candidate, None, threshold)
}
