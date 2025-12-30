use anyhow::{Context, Result};
use image::{GenericImage, GenericImageView, GrayImage};
use image_compare::{Algorithm, Metric, Similarity};
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

    // Convert to grayscale for SSIM comparison
    let ref_gray: GrayImage = ref_img.to_luma8();
    let cur_gray: GrayImage = cur_img.to_luma8();

    // Calculate SSIM using image_compare crate
    let result = image_compare::gray_similarity_structure(
        &Algorithm::MSSIMSimple,
        &ref_gray,
        &cur_gray,
    ).map_err(|e| anyhow::anyhow!("SSIM calculation failed: {:?}", e))?;

    let score = result.score;
    println!("SSIM: {:.4} (threshold {:.4})", score, threshold);

    if score < threshold {
        if let Some(out) = output {
            // Create diff visualization - show differences more clearly
            let (w, h) = ref_img.dimensions();
            let mut diff = image::DynamicImage::new_rgba8(w, h);
            for y in 0..h {
                for x in 0..w {
                    let a = ref_img.get_pixel(x, y);
                    let b = cur_img.get_pixel(x, y);
                    // Calculate color difference
                    let dr = (a[0] as i32 - b[0] as i32).unsigned_abs() as u8;
                    let dg = (a[1] as i32 - b[1] as i32).unsigned_abs() as u8;
                    let db = (a[2] as i32 - b[2] as i32).unsigned_abs() as u8;
                    // Amplify difference for visibility
                    let amp = 3u8;
                    let dr = dr.saturating_mul(amp);
                    let dg = dg.saturating_mul(amp);
                    let db = db.saturating_mul(amp);
                    // If difference is small, show original image darkened
                    if dr < 30 && dg < 30 && db < 30 {
                        diff.put_pixel(x, y, image::Rgba([a[0]/4, a[1]/4, a[2]/4, 255]));
                    } else {
                        diff.put_pixel(x, y, image::Rgba([dr, dg, db, 255]));
                    }
                }
            }
            diff.save(out)
                .with_context(|| format!("Failed to write diff image: {}", out))?;
            println!("Wrote diff image to {}", out);
        }
        anyhow::bail!("SSIM below threshold");
    }
    Ok(())
}

pub fn run_simple(reference: &str, candidate: &str, threshold: f64) -> Result<()> {
    run(reference, candidate, None, threshold)
}
