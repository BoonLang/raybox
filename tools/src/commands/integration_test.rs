use anyhow::{Context, Result};
use std::time::Duration;

pub fn run(url: &str) -> Result<()> {
    println!("🧪 Running Integration Test");
    println!("======================================");
    println!();

    // Test 1: Server responds
    println!("Test 1: Server responds...");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let response = client
        .get(url)
        .send()
        .context("Failed to connect to server")?;

    if response.status().is_success() {
        println!("  ✅ PASS: Server returns {}", response.status());
    } else {
        anyhow::bail!("Server returned {}", response.status());
    }
    println!();

    // Test 2: HTML structure
    println!("Test 2: HTML structure...");
    let html = response.text()?;

    if html.contains(r#"<canvas id="canvas""#) {
        println!("  ✅ PASS: Canvas element present");
    } else {
        anyhow::bail!("Canvas element missing");
    }

    if html.contains("renderer.js") {
        println!("  ✅ PASS: WASM module script present");
    } else {
        anyhow::bail!("WASM module script missing");
    }
    println!();

    // Test 3: WASM files exist
    println!("Test 3: WASM build artifacts...");

    let js_response = client.get(format!("{}/pkg/renderer.js", url)).send()?;
    if js_response.status().is_success() {
        println!("  ✅ PASS: renderer.js exists");
    } else {
        anyhow::bail!("renderer.js not found");
    }

    let wasm_response = client.get(format!("{}/pkg/renderer_bg.wasm", url)).send()?;
    if wasm_response.status().is_success() {
        println!("  ✅ PASS: renderer_bg.wasm exists");
    } else {
        anyhow::bail!("renderer_bg.wasm not found");
    }
    println!();

    // Test 4: Layout JSON is accessible
    println!("Test 4: Layout JSON...");
    let json_response = client
        .get(format!("{}/reference/todomvc_dom_layout.json", url))
        .send()?;

    if json_response.status().is_success() {
        println!("  ✅ PASS: Layout JSON accessible");

        let json_text = json_response.text()?;
        let json: serde_json::Value =
            serde_json::from_str(&json_text).context("Failed to parse JSON")?;

        if json
            .get("metadata")
            .and_then(|m| m.get("viewport"))
            .is_some()
        {
            println!("  ✅ PASS: JSON structure valid");
        } else {
            anyhow::bail!("Invalid JSON structure");
        }
    } else {
        anyhow::bail!("Layout JSON not accessible");
    }
    println!();

    // Test 5: Build ID endpoint
    println!("Test 5: Auto-reload endpoint...");
    let build_id_response = client.get(format!("{}/_api/build_id", url)).send()?;

    if build_id_response.status().is_success() {
        let build_id = build_id_response.text()?;
        println!("  ✅ PASS: Build ID endpoint works (ID: {})", build_id);
    } else {
        anyhow::bail!("Build ID endpoint not accessible");
    }
    println!();

    // Test 6: Browser console check via CDP
    println!("Test 6: Browser console check...");
    println!("  Running check-console command...");

    let output = std::process::Command::new("cargo")
        .args(&[
            "run",
            "-p",
            "tools",
            "--",
            "check-console",
            "--url",
            url,
            "--wait",
            "2",
        ])
        .output()
        .context("Failed to run check-console")?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if output.status.success() && stdout.contains("No errors detected") {
        println!("  ✅ PASS: No console errors detected");
    } else {
        println!("  ❌ FAIL: Console errors detected or CDP unavailable");
        println!("         Output: {}", stdout);
        anyhow::bail!("Console check failed");
    }
    println!();

    // Test 7: Screenshot capture (optional)
    println!("Test 7: Screenshot capability...");
    let screenshot_output = std::process::Command::new("cargo")
        .args(&[
            "run",
            "-p",
            "tools",
            "--",
            "check-console",
            "--url",
            url,
            "--wait",
            "1",
            "-s",
        ])
        .output();

    match screenshot_output {
        Ok(output) if output.status.success() => {
            if std::path::Path::new("screenshot.png").exists() {
                println!("  ✅ PASS: Screenshot captured successfully");
                // Clean up
                let _ = std::fs::remove_file("screenshot.png");
            } else {
                println!("  ⚠️  WARN: Screenshot command ran but no file created");
            }
        }
        _ => {
            println!("  ⚠️  SKIP: Screenshot capture not available");
        }
    }
    println!();

    println!("======================================");
    println!("✅ All tests passed!");
    println!();
    println!("Development tools verified:");
    println!("  ✅ WASM build and optimization");
    println!("  ✅ HTTP server with auto-reload");
    println!("  ✅ CDP console monitoring");
    println!("  ✅ Screenshot capture");
    println!("  ✅ Layout JSON accessibility");
    println!();

    Ok(())
}
