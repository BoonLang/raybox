use anyhow::{Context, Result};
use headless_chrome::{Browser, LaunchOptions};
use std::fs;

pub fn run(url: &str, output: &str, width: u32, height: u32) -> Result<()> {
    log::info!("Taking screenshot of {} ({}x{}) -> {}", url, width, height, output);

    println!("✓ Launching Chrome...");

    // Configure Chrome launch options
    let options = LaunchOptions::default_builder()
        .window_size(Some((width, height)))
        .headless(true)
        .build()
        .context("Failed to build launch options")?;

    let browser = Browser::new(options)
        .context("Failed to launch Chrome")?;

    println!("  Navigating to: {}", url);

    let tab = browser.new_tab()
        .context("Failed to create new tab")?;

    tab.navigate_to(url)
        .context(format!("Failed to navigate to {}", url))?;

    // Wait for page to load
    tab.wait_until_navigated()
        .context("Failed to wait for navigation")?;

    println!("  Capturing screenshot...");

    let screenshot = tab
        .capture_screenshot(
            headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png,
            None,
            None,
            true,
        )
        .context("Failed to capture screenshot")?;

    fs::write(output, &screenshot)
        .context(format!("Failed to write screenshot to {}", output))?;

    println!("✓ Screenshot saved: {}", output);
    println!("  Size: {} bytes", screenshot.len());

    Ok(())
}
