use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use crate::wasm_bindgen::{check_or_install_wasm_bindgen, run_wasm_bindgen};
use crate::wasm_opt::{check_or_install_wasm_opt, run_wasm_opt};

pub fn run(release: bool) -> Result<()> {
    let start = Instant::now();

    println!(
        "Building WASM renderer{}...",
        if release { " (release)" } else { "" }
    );
    println!();

    // Phase 1: Check/install tools
    println!("[1/5] Checking build tools...");
    check_or_install_wasm_bindgen()?;

    if release {
        check_or_install_wasm_opt()?;
    }
    println!();

    // Phase 2: Compile to WASM
    println!("[2/5] Compiling Rust to WASM...");
    compile_to_wasm(release)?;
    println!("✓ Compilation complete");
    println!();

    // Phase 3: Run wasm-bindgen
    println!("[3/5] Generating JS bindings...");
    generate_bindings(release)?;
    println!("✓ JS bindings generated");
    println!();

    // Phase 4: Optimize (release only)
    if release {
        println!("[4/5] Optimizing WASM...");
        optimize_wasm()?;
        println!("✓ WASM optimized");
        println!();

        // Phase 5: Compress (release only)
        println!("[5/5] Compressing WASM...");
        compress_wasm()?;
        println!("✓ WASM compressed");
        println!();
    } else {
        println!("[4/5] Skipping optimization (dev mode)");
        println!("[5/5] Skipping compression (dev mode)");
        println!();
    }

    // Print summary
    let duration = start.elapsed();
    println!("=== Build Complete ===");
    println!("  Time: {:.2}s", duration.as_secs_f64());
    println!("  Output: web/pkg/");

    if release {
        print_file_sizes()?;
    }

    Ok(())
}

fn compile_to_wasm(release: bool) -> Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--target")
        .arg("wasm32-unknown-unknown")
        .arg("--package")
        .arg("renderer");

    if release {
        cmd.arg("--release");
    }

    // Set RUSTFLAGS for WebGPU
    cmd.env("RUSTFLAGS", "--cfg=web_sys_unstable_apis");

    log::info!("Running: cargo build --target wasm32-unknown-unknown --package renderer");

    let status = cmd.status().context("Failed to run cargo build")?;

    if !status.success() {
        anyhow::bail!("Cargo build failed");
    }

    Ok(())
}

fn generate_bindings(release: bool) -> Result<()> {
    let profile = if release { "release" } else { "debug" };
    let wasm_path = Path::new("target")
        .join("wasm32-unknown-unknown")
        .join(profile)
        .join("renderer.wasm");

    if !wasm_path.exists() {
        anyhow::bail!("WASM file not found: {:?}", wasm_path);
    }

    let out_dir = Path::new("web/pkg");

    // Remove old pkg directory
    if out_dir.exists() {
        fs::remove_dir_all(out_dir).context("Failed to remove old pkg directory")?;
    }

    // Create new pkg directory
    fs::create_dir_all(out_dir).context("Failed to create pkg directory")?;

    run_wasm_bindgen(&wasm_path, out_dir, "web", !release)?;

    Ok(())
}

fn optimize_wasm() -> Result<()> {
    let wasm_path = Path::new("web/pkg/renderer_bg.wasm");

    if !wasm_path.exists() {
        anyhow::bail!("WASM file not found: {:?}", wasm_path);
    }

    run_wasm_opt(wasm_path, true)?;

    Ok(())
}

fn compress_wasm() -> Result<()> {
    let wasm_path = Path::new("web/pkg/renderer_bg.wasm");

    if !wasm_path.exists() {
        anyhow::bail!("WASM file not found: {:?}", wasm_path);
    }

    // Read WASM file
    let wasm_data = fs::read(wasm_path).context("Failed to read WASM file")?;

    // Compress with Brotli
    let mut br_output = Vec::new();
    let mut br_reader = std::io::Cursor::new(&wasm_data);
    brotli::BrotliCompress(
        &mut br_reader,
        &mut br_output,
        &brotli::enc::BrotliEncoderParams {
            quality: 11, // Maximum compression
            ..Default::default()
        },
    )?;

    let br_path = wasm_path.with_extension("wasm.br");
    fs::write(&br_path, br_output)
        .with_context(|| format!("Failed to write Brotli file: {:?}", br_path))?;

    // Compress with Gzip
    use flate2::write::GzEncoder;
    use flate2::Compression;

    let gz_path = wasm_path.with_extension("wasm.gz");
    let gz_file = fs::File::create(&gz_path)
        .with_context(|| format!("Failed to create gzip file: {:?}", gz_path))?;

    let mut gz_encoder = GzEncoder::new(gz_file, Compression::best());
    gz_encoder.write_all(&wasm_data)?;
    gz_encoder.finish()?;

    println!("  - Created: {:?}", br_path.file_name().unwrap());
    println!("  - Created: {:?}", gz_path.file_name().unwrap());

    Ok(())
}

fn print_file_sizes() -> Result<()> {
    let wasm_path = Path::new("web/pkg/renderer_bg.wasm");
    let br_path = wasm_path.with_extension("wasm.br");
    let gz_path = wasm_path.with_extension("wasm.gz");

    let wasm_size = fs::metadata(wasm_path)?.len();
    let br_size = fs::metadata(&br_path)?.len();
    let gz_size = fs::metadata(&gz_path)?.len();

    println!();
    println!("  File sizes:");
    println!("    WASM:    {} KB", wasm_size / 1024);
    println!(
        "    Brotli:  {} KB ({:.1}%)",
        br_size / 1024,
        (br_size as f64 / wasm_size as f64) * 100.0
    );
    println!(
        "    Gzip:    {} KB ({:.1}%)",
        gz_size / 1024,
        (gz_size as f64 / wasm_size as f64) * 100.0
    );

    Ok(())
}
