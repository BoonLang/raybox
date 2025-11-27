use anyhow::{Context, Result};
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotParams;
use chromiumoxide::Page;
use std::fs;

/// Capture a full-page screenshot and return its byte length.
pub async fn capture_full_page(output: &str, page: Page) -> Result<usize> {
    let screenshot_data = page
        .screenshot(CaptureScreenshotParams::default())
        .await
        .context("Failed to capture screenshot")?;
    fs::write(output, &screenshot_data)
        .context(format!("Failed to write screenshot to {}", output))?;
    println!("✓ Screenshot saved: {}", output);
    println!("  Size: {} bytes", screenshot_data.len());
    Ok(screenshot_data.len())
}
