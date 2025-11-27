use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;
use futures::StreamExt;
use std::fs;
use std::time::Duration;
use crate::commands::screenshot_helpers::capture_full_page;

pub fn run(url: &str, output: &str, width: u32, height: u32, _headed: bool) -> Result<()> {
    log::info!(
        "Taking screenshot of {} ({}x{}) -> {}",
        url,
        width,
        height,
        output
    );

    // Use tokio runtime for async operations
    tokio::runtime::Runtime::new()?.block_on(async {
        println!("✓ Launching Chrome...");

        // Configure Chrome with WebGPU/headless-friendly flags (CRITICAL - see CLAUDE.md)
        let webgpu_flags = vec![
            "--disable-dev-shm-usage",
            "--no-sandbox",
            "--hide-scrollbars",
            "--mute-audio",
            "--enable-unsafe-webgpu",
            "--enable-webgpu-developer-features",
            "--enable-features=UseSkiaRenderer",
        ];

        let cfg = BrowserConfig::builder()
            .with_head() // force headed for WebGPU
            .window_size(width, height)
            .args(webgpu_flags);

        let (browser, mut handler) = Browser::launch(
            cfg.build()
                .map_err(|e| anyhow::anyhow!("Failed to build browser config: {}", e))?,
        )
        .await
        .context("Failed to launch Chrome")?;

        // Spawn handler task
        tokio::spawn(async move { while handler.next().await.is_some() {} });

        println!("  Navigating to: {}", url);

        let page = browser
            .new_page(url)
            .await
            .context("Failed to create new page")?;

        // Capture console output inside the page (log/warn/error)
        let _ = page
            .evaluate(
                "() => { \
                    window.__console_buffer = []; \
                    ['log','warn','error'].forEach(t => { \
                      const orig = console[t]; \
                      console[t] = (...args) => { \
                        try { window.__console_buffer.push({type:t, msg: args.map(a=>String(a)).join(' ')}); } catch(_) {} \
                        orig.apply(console, args); \
                      }; \
                    }); \
                  }",
            )
            .await;

        // Set exact viewport dimensions (not just window size)
        page.execute(SetDeviceMetricsOverrideParams::new(
            width as i64,
            height as i64,
            1.0,   // device_scale_factor
            false, // mobile
        ))
        .await
        .context("Failed to set viewport dimensions")?;

        // Wait for page to load (give WebGPU time to initialize)
        tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

        // Poll for emergent WebGPU flag set by renderer; fall back after timeout
        let mut webgpu_ok = false;
        for _ in 0..10 {
            let result: Option<bool> = page
                .evaluate("() => window.__emergent_webgpu_ok === true || window.__classic_webgpu_ok === true")
                .await
                .ok()
                .and_then(|v| v.into_value().ok())
                .and_then(|v| v);
            if result == Some(true) {
                webgpu_ok = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        if webgpu_ok {
            println!("  WebGPU flag detected (__emergent_webgpu_ok=true)");
        } else {
            println!("  WebGPU flag NOT detected; likely captured fallback");
        }

        // Print captured console messages
        if let Ok(val) = page
            .evaluate("() => (window.__console_buffer || []).map(e => `${e.type}: ${e.msg}`).join('\\n')")
            .await
        {
            if let Ok(Some(logs)) = val.into_value::<Option<String>>() {
                if !logs.is_empty() {
                    println!("  Console:\n{}", logs);
                }
            }
        }

        if let Ok(val) = page
            .evaluate("() => window.__layout_error ? String(window.__layout_error) : null")
            .await
        {
            if let Ok(Some(err)) = val.into_value::<Option<String>>() {
                println!("  layout_error: {}", err);
            }
        }

        if let Ok(val) = page
            .evaluate("() => window.__footer_text_count ?? null")
            .await
        {
            if let Ok(Some(cnt)) = val.into_value::<Option<f64>>() {
                println!("  Footer text count: {}", cnt);
            }
        }
        if let Ok(val) = page
            .evaluate("() => window.__footer_debug ?? null")
            .await
        {
            if let Ok(Some(dbg)) = val.into_value::<Option<String>>() {
                if !dbg.is_empty() {
                    println!("  Footer debug: {}", dbg);
                }
            }
        }

        // Debug: report emergent counts if present
        if let Ok(val) = page
            .evaluate("() => typeof window.__emergent_counts === 'string' ? window.__emergent_counts : null")
            .await
        {
            if let Ok(Some(s)) = val.into_value::<Option<String>>() {
                println!("  Emergent counts: {}", s);
            }
        }

        // Prefer real page screenshot (captures the WebGPU swapchain when running headed).
        let page_size = capture_full_page(output, page.clone()).await;
        if let Err(e) = page_size {
            eprintln!("page.screenshot failed ({:?}), trying canvas.toDataURL fallback", e);
            let data_url: Option<String> = page
                .evaluate("() => { const c=document.getElementById('canvas'); return c ? c.toDataURL('image/png') : null; }")
                .await
                .ok()
                .and_then(|v| v.into_value().ok())
                .and_then(|s| s);

            if let Some(data_url) = data_url {
                let prefix = "data:image/png;base64,";
                if let Some(b64) = data_url.strip_prefix(prefix) {
                    let bytes = B64.decode(b64).context("Failed to decode canvas data URL")?;
                    fs::write(output, &bytes)
                        .context(format!("Failed to write screenshot to {}", output))?;
                    println!("✓ Screenshot saved from canvas.toDataURL(): {}", output);
                    println!("  Size: {} bytes", bytes.len());
                } else {
                    println!("Canvas data URL missing PNG prefix; no screenshot saved.");
                }
            } else {
                println!("Canvas data URL unavailable; no screenshot saved.");
            }
        } else {
            let len = page_size.unwrap_or(0);
            if len < 10_000 {
                println!("  Page screenshot is very small ({} bytes) — trying canvas.toDataURL fallback", len);
                let data_url: Option<String> = page
                    .evaluate("() => { const c=document.getElementById('canvas'); return c ? c.toDataURL('image/png') : null; }")
                    .await
                    .ok()
                    .and_then(|v| v.into_value().ok())
                    .and_then(|s| s);
                if let Some(data_url) = data_url {
                    let prefix = "data:image/png;base64,";
                    if let Some(b64) = data_url.strip_prefix(prefix) {
                        let bytes = B64.decode(b64).context("Failed to decode canvas data URL")?;
                        fs::write(output, &bytes)
                            .context(format!("Failed to write screenshot to {}", output))?;
                        println!("✓ Screenshot replaced from canvas.toDataURL(): {}", output);
                        println!("  Size: {} bytes", bytes.len());
                    } else {
                        println!("Canvas data URL missing PNG prefix; keeping page screenshot.");
                    }
                } else {
                    println!("Canvas data URL unavailable; keeping page screenshot (may be blank).");
                }
            } else {
                println!("✓ Screenshot saved via page.screenshot -> {}", output);
            }
        }

        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}
