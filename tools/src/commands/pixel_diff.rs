use anyhow::{Context, Result};
use image::{DynamicImage, GenericImageView};

pub fn run(reference: &str, current: &str, output: Option<&str>, threshold: f64) -> Result<()> {
    log::info!("Comparing images: {} vs {}", reference, current);

    println!("✓ Loading images...");

    // Load images
    let ref_img = image::open(reference)
        .context(format!("Failed to load reference image: {}", reference))?;

    let cur_img = image::open(current)
        .context(format!("Failed to load current image: {}", current))?;

    // Check dimensions match
    if ref_img.dimensions() != cur_img.dimensions() {
        anyhow::bail!(
            "Image dimensions don't match: {}x{} vs {}x{}",
            ref_img.width(),
            ref_img.height(),
            cur_img.width(),
            cur_img.height()
        );
    }

    println!("  Reference: {}x{} pixels", ref_img.width(), ref_img.height());
    println!("  Current: {}x{} pixels", cur_img.width(), cur_img.height());
    println!();

    // Perform comparison using simple pixel difference
    println!("✓ Computing similarity metrics...");

    let (score, total_diff, max_diff) = compute_similarity(&ref_img, &cur_img);

    // Print results
    println!();
    println!("=== Comparison Results ===");
    println!("  Similarity Score: {:.4} (1.0 = perfect match)", score);
    println!("  Total difference: {} pixels", total_diff);
    println!("  Max pixel diff: {}", max_diff);
    println!("  Threshold: {:.4}", threshold);
    println!();

    // Determine if images match
    let matches = score >= threshold;

    if matches {
        println!("✅ Images MATCH (score >= threshold)");
    } else {
        println!("❌ Images DIFFER (score < threshold)");
        println!("   Difference: {:.2}%", (1.0 - score) * 100.0);
    }

    // Save diff image if output path provided
    if let Some(output_path) = output {
        println!();
        println!("✓ Generating diff image...");

        // Create a diff image showing differences
        let diff_img = create_diff_image(&ref_img, &cur_img)?;

        diff_img.save(output_path)
            .context(format!("Failed to save diff image to {}", output_path))?;

        println!("  Diff image saved: {}", output_path);

        // Also save to absolute path for easy opening
        let abs_path = std::fs::canonicalize(output_path)?;
        println!("  Absolute path: {}", abs_path.display());
    }

    println!();

    if !matches {
        anyhow::bail!("Images do not match (score {:.4} < threshold {:.4})", score, threshold);
    }

    Ok(())
}

fn compute_similarity(ref_img: &DynamicImage, cur_img: &DynamicImage) -> (f64, usize, u32) {
    let (width, height) = ref_img.dimensions();
    let ref_rgb = ref_img.to_rgb8();
    let cur_rgb = cur_img.to_rgb8();

    let total_pixels = (width * height) as usize;
    let mut total_diff_pixels = 0;
    let mut max_diff = 0u32;
    let mut sum_diff = 0u64;

    for y in 0..height {
        for x in 0..width {
            let ref_pixel = ref_rgb.get_pixel(x, y);
            let cur_pixel = cur_rgb.get_pixel(x, y);

            // Calculate per-channel difference
            let r_diff = (ref_pixel[0] as i32 - cur_pixel[0] as i32).abs() as u32;
            let g_diff = (ref_pixel[1] as i32 - cur_pixel[1] as i32).abs() as u32;
            let b_diff = (ref_pixel[2] as i32 - cur_pixel[2] as i32).abs() as u32;

            let pixel_diff = r_diff + g_diff + b_diff;

            if pixel_diff > 10 {
                total_diff_pixels += 1;
            }

            max_diff = max_diff.max(pixel_diff);
            sum_diff += pixel_diff as u64;
        }
    }

    // Compute similarity score (1.0 = perfect match, 0.0 = totally different)
    // We normalize by 3 * 255 (max possible diff per pixel)
    let max_possible_diff = (total_pixels as u64) * 3 * 255;
    let score = 1.0 - (sum_diff as f64 / max_possible_diff as f64);

    (score, total_diff_pixels, max_diff)
}

fn create_diff_image(ref_img: &DynamicImage, cur_img: &DynamicImage) -> Result<DynamicImage> {
    use image::{ImageBuffer, Rgb};

    let (width, height) = ref_img.dimensions();
    let ref_rgb = ref_img.to_rgb8();
    let cur_rgb = cur_img.to_rgb8();

    let mut diff_img = ImageBuffer::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let ref_pixel = ref_rgb.get_pixel(x, y);
            let cur_pixel = cur_rgb.get_pixel(x, y);

            // Calculate per-channel difference
            let r_diff = (ref_pixel[0] as i32 - cur_pixel[0] as i32).abs();
            let g_diff = (ref_pixel[1] as i32 - cur_pixel[1] as i32).abs();
            let b_diff = (ref_pixel[2] as i32 - cur_pixel[2] as i32).abs();

            // Highlight differences in red
            let diff_magnitude = (r_diff + g_diff + b_diff) / 3;

            if diff_magnitude > 10 {
                // Red for differences
                diff_img.put_pixel(x, y, Rgb([255, 0, 0]));
            } else {
                // Grayscale for matches
                let gray = cur_pixel[0] / 2 + 64; // Lighten for better visibility
                diff_img.put_pixel(x, y, Rgb([gray, gray, gray]));
            }
        }
    }

    Ok(DynamicImage::ImageRgb8(diff_img))
}
