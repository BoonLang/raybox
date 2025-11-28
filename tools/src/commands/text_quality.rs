//! Text quality analysis tool
//!
//! Compares text rendering quality by analyzing:
//! - Structural similarity (SSIM)
//! - Edge sharpness (gradient analysis)
//! - Anti-aliasing quality (intermediate pixel distribution)

use anyhow::{Context, Result};
use image::{GenericImageView, GrayImage, Luma, Rgba, RgbaImage};
use std::path::Path;

/// Region of interest for cropping
#[derive(Debug, Clone, Copy)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Text quality metrics
#[derive(Debug)]
pub struct TextQualityMetrics {
    /// Structural similarity (0.0 - 1.0, higher = better)
    pub ssim: f64,
    /// Edge sharpness score (0.0 - 1.0, higher = sharper edges)
    pub edge_sharpness: f64,
    /// Jaggedness score (0.0 - 1.0, lower = smoother)
    pub jaggedness: f64,
    /// Anti-aliasing quality (0.0 - 1.0, ideal around 0.5)
    pub aa_quality: f64,
    /// Overall quality score
    pub overall: f64,
}

impl TextQualityMetrics {
    pub fn print_report(&self) {
        println!("\n📊 Text Quality Analysis:");
        println!("  ├─ SSIM:            {:.4} (structural similarity)", self.ssim);
        println!("  ├─ Edge sharpness:  {:.4} (higher = crisper)", self.edge_sharpness);
        println!("  ├─ Jaggedness:      {:.4} (lower = smoother)", self.jaggedness);
        println!("  ├─ AA quality:      {:.4} (ideal ~0.5)", self.aa_quality);
        println!("  └─ Overall:         {:.4}", self.overall);

        // Diagnosis
        println!("\n🔍 Diagnosis:");
        if self.jaggedness > 0.3 {
            println!("  ⚠️  Text is JAGGED - increase AA width multiplier");
        } else if self.edge_sharpness < 0.3 {
            println!("  ⚠️  Text is BLURRY - decrease AA width multiplier");
        } else {
            println!("  ✅ Text quality is GOOD");
        }
    }
}

/// Crop a region from an image
fn crop_region(img: &RgbaImage, region: Region) -> RgbaImage {
    let mut cropped = RgbaImage::new(region.width, region.height);
    for y in 0..region.height {
        for x in 0..region.width {
            let src_x = region.x + x;
            let src_y = region.y + y;
            if src_x < img.width() && src_y < img.height() {
                cropped.put_pixel(x, y, *img.get_pixel(src_x, src_y));
            }
        }
    }
    cropped
}

/// Convert to grayscale
fn to_grayscale(img: &RgbaImage) -> GrayImage {
    let mut gray = GrayImage::new(img.width(), img.height());
    for (x, y, pixel) in img.enumerate_pixels() {
        let r = pixel[0] as f64;
        let g = pixel[1] as f64;
        let b = pixel[2] as f64;
        let luma = (0.299 * r + 0.587 * g + 0.114 * b) as u8;
        gray.put_pixel(x, y, Luma([luma]));
    }
    gray
}

/// Calculate SSIM between two grayscale images
fn calculate_ssim(img1: &GrayImage, img2: &GrayImage) -> f64 {
    if img1.dimensions() != img2.dimensions() {
        return 0.0;
    }

    let (w, h) = img1.dimensions();
    let n = (w * h) as f64;

    // Constants for stability
    let c1 = 6.5025; // (0.01 * 255)^2
    let c2 = 58.5225; // (0.03 * 255)^2

    // Calculate means
    let mut sum1 = 0.0;
    let mut sum2 = 0.0;
    for (p1, p2) in img1.pixels().zip(img2.pixels()) {
        sum1 += p1[0] as f64;
        sum2 += p2[0] as f64;
    }
    let mean1 = sum1 / n;
    let mean2 = sum2 / n;

    // Calculate variances and covariance
    let mut var1 = 0.0;
    let mut var2 = 0.0;
    let mut cov = 0.0;
    for (p1, p2) in img1.pixels().zip(img2.pixels()) {
        let d1 = p1[0] as f64 - mean1;
        let d2 = p2[0] as f64 - mean2;
        var1 += d1 * d1;
        var2 += d2 * d2;
        cov += d1 * d2;
    }
    var1 /= n - 1.0;
    var2 /= n - 1.0;
    cov /= n - 1.0;

    // SSIM formula
    let numerator = (2.0 * mean1 * mean2 + c1) * (2.0 * cov + c2);
    let denominator = (mean1 * mean1 + mean2 * mean2 + c1) * (var1 + var2 + c2);

    numerator / denominator
}

/// Calculate Sobel gradient magnitude
fn sobel_gradient(img: &GrayImage) -> Vec<f64> {
    let (w, h) = img.dimensions();
    let mut gradients = Vec::with_capacity((w * h) as usize);

    for y in 1..(h - 1) {
        for x in 1..(w - 1) {
            // Sobel kernels
            let gx = -1.0 * img.get_pixel(x - 1, y - 1)[0] as f64
                + 1.0 * img.get_pixel(x + 1, y - 1)[0] as f64
                - 2.0 * img.get_pixel(x - 1, y)[0] as f64
                + 2.0 * img.get_pixel(x + 1, y)[0] as f64
                - 1.0 * img.get_pixel(x - 1, y + 1)[0] as f64
                + 1.0 * img.get_pixel(x + 1, y + 1)[0] as f64;

            let gy = -1.0 * img.get_pixel(x - 1, y - 1)[0] as f64
                - 2.0 * img.get_pixel(x, y - 1)[0] as f64
                - 1.0 * img.get_pixel(x + 1, y - 1)[0] as f64
                + 1.0 * img.get_pixel(x - 1, y + 1)[0] as f64
                + 2.0 * img.get_pixel(x, y + 1)[0] as f64
                + 1.0 * img.get_pixel(x + 1, y + 1)[0] as f64;

            gradients.push((gx * gx + gy * gy).sqrt());
        }
    }

    gradients
}

/// Measure edge sharpness (0.0 - 1.0)
/// Higher values mean sharper edges
fn measure_edge_sharpness(img: &GrayImage) -> f64 {
    let gradients = sobel_gradient(img);
    if gradients.is_empty() {
        return 0.0;
    }

    // Find strong edges (top percentile)
    let mut sorted = gradients.clone();
    sorted.sort_by(|a, b| b.partial_cmp(a).unwrap());

    let top_10_percent = sorted.len() / 10;
    if top_10_percent == 0 {
        return 0.0;
    }

    let strong_edges: f64 = sorted[..top_10_percent].iter().sum();
    let avg_strong_edge = strong_edges / top_10_percent as f64;

    // Normalize to 0-1 range (255*sqrt(2)*4 is theoretical max Sobel response)
    (avg_strong_edge / 1000.0).min(1.0)
}

/// Measure jaggedness by analyzing diagonal patterns
/// Higher values mean more jagged (staircase) patterns
fn measure_jaggedness(img: &GrayImage) -> f64 {
    let (w, h) = img.dimensions();
    let mut diagonal_changes = 0u64;
    let mut horizontal_changes = 0u64;
    let mut vertical_changes = 0u64;
    let threshold = 50;

    for y in 1..(h - 1) {
        for x in 1..(w - 1) {
            let center = img.get_pixel(x, y)[0] as i32;
            let left = img.get_pixel(x - 1, y)[0] as i32;
            let right = img.get_pixel(x + 1, y)[0] as i32;
            let up = img.get_pixel(x, y - 1)[0] as i32;
            let down = img.get_pixel(x, y + 1)[0] as i32;
            let diag_ul = img.get_pixel(x - 1, y - 1)[0] as i32;
            let diag_dr = img.get_pixel(x + 1, y + 1)[0] as i32;

            // Count significant changes
            if (center - left).abs() > threshold {
                horizontal_changes += 1;
            }
            if (center - right).abs() > threshold {
                horizontal_changes += 1;
            }
            if (center - up).abs() > threshold {
                vertical_changes += 1;
            }
            if (center - down).abs() > threshold {
                vertical_changes += 1;
            }
            if (center - diag_ul).abs() > threshold {
                diagonal_changes += 1;
            }
            if (center - diag_dr).abs() > threshold {
                diagonal_changes += 1;
            }
        }
    }

    let total = horizontal_changes + vertical_changes + diagonal_changes;
    if total == 0 {
        return 0.0;
    }

    // Jagged text has more horizontal/vertical changes relative to diagonal
    // Smooth anti-aliased text has more balanced distribution
    let hv_ratio = (horizontal_changes + vertical_changes) as f64 / total as f64;

    // More H/V changes = more jagged (staircase pattern)
    // Score is 0 (smooth) to 1 (very jagged)
    ((hv_ratio - 0.5) * 2.0).max(0.0).min(1.0)
}

/// Analyze anti-aliasing quality
/// Measures distribution of intermediate gray values
fn analyze_aa_quality(img: &GrayImage) -> f64 {
    let mut pure_black = 0u64;
    let mut pure_white = 0u64;
    let mut intermediate = 0u64;

    for pixel in img.pixels() {
        let v = pixel[0];
        if v < 10 {
            pure_black += 1;
        } else if v > 245 {
            pure_white += 1;
        } else {
            intermediate += 1;
        }
    }

    let total = (pure_black + pure_white + intermediate) as f64;
    if total == 0.0 {
        return 0.0;
    }

    // Good AA has ~10-30% intermediate pixels
    // Too few = jagged, too many = blurry
    let aa_ratio = intermediate as f64 / total;

    // Score peaks at 0.2 (20% intermediate pixels)
    let ideal = 0.2;
    let deviation = (aa_ratio - ideal).abs();
    (1.0 - deviation * 3.0).max(0.0).min(1.0)
}

/// Analyze text quality of a single image
pub fn analyze_single(img: &RgbaImage) -> TextQualityMetrics {
    let gray = to_grayscale(img);

    let edge_sharpness = measure_edge_sharpness(&gray);
    let jaggedness = measure_jaggedness(&gray);
    let aa_quality = analyze_aa_quality(&gray);

    // Overall score (weighted combination)
    let overall = edge_sharpness * 0.4 + (1.0 - jaggedness) * 0.3 + aa_quality * 0.3;

    TextQualityMetrics {
        ssim: 1.0, // N/A for single image
        edge_sharpness,
        jaggedness,
        aa_quality,
        overall,
    }
}

/// Compare text quality between reference and current image
pub fn compare_quality(reference: &RgbaImage, current: &RgbaImage) -> TextQualityMetrics {
    let ref_gray = to_grayscale(reference);
    let cur_gray = to_grayscale(current);

    let ssim = calculate_ssim(&ref_gray, &cur_gray);
    let edge_sharpness = measure_edge_sharpness(&cur_gray);
    let jaggedness = measure_jaggedness(&cur_gray);
    let aa_quality = analyze_aa_quality(&cur_gray);

    // Overall score (weighted combination)
    let overall = ssim * 0.3 + edge_sharpness * 0.25 + (1.0 - jaggedness) * 0.25 + aa_quality * 0.2;

    TextQualityMetrics {
        ssim,
        edge_sharpness,
        jaggedness,
        aa_quality,
        overall,
    }
}

/// Run text quality analysis
pub fn run(
    reference: &str,
    current: &str,
    region: Option<(u32, u32, u32, u32)>, // x, y, width, height
    output_crop: Option<&str>,
) -> Result<TextQualityMetrics> {
    // Load images
    let ref_img = image::open(Path::new(reference))
        .with_context(|| format!("Failed to open reference image: {}", reference))?
        .into_rgba8();

    let cur_img = image::open(Path::new(current))
        .with_context(|| format!("Failed to open current image: {}", current))?
        .into_rgba8();

    // Crop if region specified
    let (ref_cropped, cur_cropped) = if let Some((x, y, w, h)) = region {
        let region = Region { x, y, width: w, height: h };
        println!("📐 Cropping region: {}x{} at ({}, {})", w, h, x, y);
        (crop_region(&ref_img, region), crop_region(&cur_img, region))
    } else {
        (ref_img, cur_img)
    };

    // Save cropped images if requested
    if let Some(out) = output_crop {
        let ref_path = format!("{}_reference.png", out);
        let cur_path = format!("{}_current.png", out);
        ref_cropped.save(&ref_path).with_context(|| format!("Failed to save: {}", ref_path))?;
        cur_cropped.save(&cur_path).with_context(|| format!("Failed to save: {}", cur_path))?;
        println!("💾 Saved cropped images: {}, {}", ref_path, cur_path);
    }

    // Compare quality
    let metrics = compare_quality(&ref_cropped, &cur_cropped);

    Ok(metrics)
}

/// Run text quality analysis on a single image (no reference)
pub fn run_single(image_path: &str, region: Option<(u32, u32, u32, u32)>) -> Result<TextQualityMetrics> {
    let img = image::open(Path::new(image_path))
        .with_context(|| format!("Failed to open image: {}", image_path))?
        .into_rgba8();

    let cropped = if let Some((x, y, w, h)) = region {
        let region = Region { x, y, width: w, height: h };
        println!("📐 Cropping region: {}x{} at ({}, {})", w, h, x, y);
        crop_region(&img, region)
    } else {
        img
    };

    Ok(analyze_single(&cropped))
}
