use anyhow::{Context, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;
use futures::StreamExt;
use std::fs;
use std::time::Duration;

use crate::layout::LayoutData;

/// Extraction script that runs in the browser
const EXTRACTION_SCRIPT: &str = include_str!("../../../scripts/extract_dom_layout.js");

pub fn run(url: &str, output: &str, width: u32, height: u32) -> Result<()> {
    log::info!(
        "Extracting DOM layout from {} ({}x{}) -> {}",
        url,
        width,
        height,
        output
    );

    tokio::runtime::Runtime::new()?.block_on(async {
        println!("✓ Launching Chrome for DOM extraction...");

        // Configure Chrome - don't need WebGPU flags for DOM extraction
        let flags = vec![
            "--disable-dev-shm-usage",
            "--no-sandbox",
            "--hide-scrollbars",
            "--mute-audio",
        ];

        let cfg = BrowserConfig::builder()
            .with_head()
            .window_size(width, height)
            .args(flags);

        let (browser, mut handler) = Browser::launch(
            cfg.build()
                .map_err(|e| anyhow::anyhow!("Failed to build browser config: {}", e))?,
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

        // Set exact viewport dimensions
        page.execute(SetDeviceMetricsOverrideParams::new(
            width as i64,
            height as i64,
            1.0,   // device_scale_factor
            false, // mobile
        ))
        .await
        .context("Failed to set viewport dimensions")?;

        // Wait for page to fully load
        tokio::time::sleep(Duration::from_millis(1000)).await;

        println!("  Executing extraction script...");

        // Inject the extraction script and run it
        let script = format!(
            r#"
            (() => {{
                {}
                return extractDOMLayout();
            }})()
            "#,
            EXTRACTION_SCRIPT
        );

        let result = page
            .evaluate(script.as_str())
            .await
            .context("Failed to execute extraction script")?;

        // The script returns a JSON string
        let json_str: String = result
            .into_value()
            .context("Failed to get extraction result")?;

        println!("  Parsing extracted layout...");

        // Parse the JSON into our LayoutData struct
        let layout: LayoutData =
            serde_json::from_str(&json_str).context("Failed to parse layout JSON")?;

        println!(
            "  Extracted {} elements",
            layout.summary.total_elements
        );

        // Write to output file
        let pretty_json = serde_json::to_string_pretty(&layout)
            .context("Failed to serialize layout")?;
        fs::write(output, &pretty_json)
            .context(format!("Failed to write to {}", output))?;

        println!("✓ Layout extracted to: {}", output);
        println!("  Elements: {}", layout.elements.len());

        // Print some stats
        if let Some(label_count) = layout.summary.by_tag.get("label") {
            println!("  Labels: {}", label_count);
        }

        // Verify label positions are correct (x should be ~75, not 135)
        let labels: Vec<_> = layout
            .elements
            .iter()
            .filter(|e| e.tag == "label" && e.classes.is_empty())
            .collect();
        if !labels.is_empty() {
            let first_label = labels[0];
            println!(
                "  First todo label position: x={}, width={}",
                first_label.x, first_label.width
            );
            if first_label.x > 100.0 {
                println!("  ⚠️  Warning: Label x position looks wrong (expected ~75, got {})", first_label.x);
            }
        }

        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}
