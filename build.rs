use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() -> Result<()> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);

    let shaders_dir = manifest_dir.join("shaders");
    let slang_file = shaders_dir.join("rectangle.slang");
    let wgsl_file = out_dir.join("rectangle.wgsl");

    // Tell Cargo to rerun if shader changes
    println!("cargo:rerun-if-changed={}", slang_file.display());

    // Find slangc - check common locations
    let slangc = find_slangc().context("Could not find slangc. Please install it from https://github.com/shader-slang/slang/releases")?;

    // Set LD_LIBRARY_PATH for slangc's shared libraries
    let slangc_dir = slangc.parent().unwrap();
    let lib_dir = slangc_dir.parent().unwrap().join("lib");
    let ld_library_path = if let Ok(existing) = env::var("LD_LIBRARY_PATH") {
        format!("{}:{}", lib_dir.display(), existing)
    } else {
        lib_dir.display().to_string()
    };

    // Temp files for shader compilation (slangc doesn't support stdout for WGSL)
    let vs_temp = out_dir.join("vs_temp.wgsl");
    let fs_temp = out_dir.join("fs_temp.wgsl");

    // Compile vertex shader
    let vs_status = Command::new(&slangc)
        .env("LD_LIBRARY_PATH", &ld_library_path)
        .args([
            slang_file.to_str().unwrap(),
            "-entry", "vs_main",
            "-stage", "vertex",
            "-target", "wgsl",
            "-o", vs_temp.to_str().unwrap(),
        ])
        .status()
        .context("Failed to run slangc for vertex shader")?;

    if !vs_status.success() {
        anyhow::bail!("slangc failed for vertex shader");
    }

    // Compile fragment shader
    let fs_status = Command::new(&slangc)
        .env("LD_LIBRARY_PATH", &ld_library_path)
        .args([
            slang_file.to_str().unwrap(),
            "-entry", "fs_main",
            "-stage", "fragment",
            "-target", "wgsl",
            "-o", fs_temp.to_str().unwrap(),
        ])
        .status()
        .context("Failed to run slangc for fragment shader")?;

    if !fs_status.success() {
        anyhow::bail!("slangc failed for fragment shader");
    }

    // Read compiled shaders
    let vs_wgsl = std::fs::read_to_string(&vs_temp)
        .context("Failed to read vertex shader output")?;
    let fs_wgsl = std::fs::read_to_string(&fs_temp)
        .context("Failed to read fragment shader output")?;

    // Write combined WGSL
    let combined_wgsl = format!(
        "// Generated from rectangle.slang - DO NOT EDIT\n\n\
        // Vertex shader\n{}\n\n\
        // Fragment shader\n{}",
        vs_wgsl.trim(),
        fs_wgsl.trim()
    );

    std::fs::write(&wgsl_file, &combined_wgsl)
        .context("Failed to write combined WGSL file")?;

    // Generate Rust bindings using wgsl_bindgen
    let bindings_file = out_dir.join("shader_bindings.rs");
    wgsl_bindgen::WgslBindgenOptionBuilder::default()
        .workspace_root(out_dir.to_string_lossy().to_string())
        .add_entry_point(wgsl_file.to_string_lossy().to_string())
        .serialization_strategy(wgsl_bindgen::WgslTypeSerializeStrategy::Bytemuck)
        .output(bindings_file.to_string_lossy().to_string())
        .emit_rerun_if_change(false)  // We handle this manually
        .build()?
        .generate()?;

    // Post-process: convert inner attributes (#![...]) to outer attributes (#[...])
    // so the file can be used with include!()
    let bindings_content = std::fs::read_to_string(&bindings_file)
        .context("Failed to read generated bindings")?;
    let fixed_content = bindings_content.replace("#![allow(", "#[allow(");
    std::fs::write(&bindings_file, fixed_content)
        .context("Failed to write fixed bindings")?;

    Ok(())
}

fn find_slangc() -> Option<PathBuf> {
    // Check PATH first
    if let Ok(output) = Command::new("which").arg("slangc").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }

    // Check common locations
    let home = env::var("HOME").ok()?;
    let candidates = [
        format!("{}/.local/bin/slangc", home),
        "/usr/local/bin/slangc".to_string(),
        "/usr/bin/slangc".to_string(),
    ];

    for candidate in candidates {
        let path = PathBuf::from(&candidate);
        if path.exists() {
            return Some(path);
        }
    }

    None
}
