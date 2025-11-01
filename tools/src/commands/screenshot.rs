use anyhow::{Context, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotParams;
use futures::StreamExt;
use std::fs;

pub fn run(url: &str, output: &str, width: u32, height: u32) -> Result<()> {
    log::info!("Taking screenshot of {} ({}x{}) -> {}", url, width, height, output);

    // Use tokio runtime for async operations
    tokio::runtime::Runtime::new()?.block_on(async {
        println!("✓ Launching Chrome...");

        // Configure Chrome with WebGPU flags (CRITICAL - see CLAUDE.md)
        let webgpu_flags = vec![
            "--enable-unsafe-webgpu",
            "--enable-webgpu-developer-features",
            "--enable-features=Vulkan,VulkanFromANGLE",
            "--enable-vulkan",
            "--use-angle=vulkan",
            "--disable-software-rasterizer",
            "--ozone-platform=x11",
        ];

        let (browser, mut handler) = Browser::launch(
            BrowserConfig::builder()
                .with_head() // Show browser window (WebGPU needs this)
                .window_size(width, height)
                .args(webgpu_flags)
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to build browser config: {}", e))?
        )
        .await
        .context("Failed to launch Chrome")?;

        // Spawn handler task
        tokio::spawn(async move {
            while handler.next().await.is_some() {}
        });

        println!("  Navigating to: {}", url);

        let page = browser
            .new_page(url)
            .await
            .context("Failed to create new page")?;

        // Wait for page to load (give WebGPU time to initialize)
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        println!("  Capturing screenshot...");

        let screenshot_data = page
            .screenshot(CaptureScreenshotParams::default())
            .await
            .context("Failed to capture screenshot")?;

        fs::write(output, &screenshot_data)
            .context(format!("Failed to write screenshot to {}", output))?;

        println!("✓ Screenshot saved: {}", output);
        println!("  Size: {} bytes", screenshot_data.len());

        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}
