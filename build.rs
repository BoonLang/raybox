use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() -> Result<()> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let shaders_dir = manifest_dir.join("shaders");

    // Find slangc
    let slangc = find_slangc().context(
        "Could not find slangc. Please install it from https://github.com/shader-slang/slang/releases",
    )?;

    // Set LD_LIBRARY_PATH for slangc's shared libraries
    let slangc_dir = slangc.parent().unwrap();
    let lib_dir = slangc_dir.parent().unwrap().join("lib");
    let ld_library_path = if let Ok(existing) = env::var("LD_LIBRARY_PATH") {
        format!("{}:{}", lib_dir.display(), existing)
    } else {
        lib_dir.display().to_string()
    };

    // Compile all shaders
    let shaders = [
        ShaderConfig {
            name: "rectangle",
            entries: vec![("vs_main", "vertex"), ("fs_main", "fragment")],
        },
        ShaderConfig {
            name: "empty",
            entries: vec![("vs_main", "vertex"), ("fs_main", "fragment")],
        },
        ShaderConfig {
            name: "overlay",
            entries: vec![("vs_main", "vertex"), ("fs_main", "fragment")],
        },
        ShaderConfig {
            name: "present",
            entries: vec![("vs_main", "vertex"), ("fs_main", "fragment")],
        },
        ShaderConfig {
            name: "frame_composite",
            entries: vec![("vs_main", "vertex"), ("fs_main", "fragment")],
        },
        ShaderConfig {
            name: "sdf_raymarch",
            entries: vec![("vs_main", "vertex"), ("fs_main", "fragment")],
        },
        ShaderConfig {
            name: "sdf_spheres",
            entries: vec![("vs_main", "vertex"), ("fs_main", "fragment")],
        },
        ShaderConfig {
            name: "sdf_towers",
            entries: vec![("vs_main", "vertex"), ("fs_main", "fragment")],
        },
        ShaderConfig {
            name: "sdf_clay_vector",
            entries: vec![("vs_main", "vertex"), ("fs_main", "fragment")],
        },
        ShaderConfig {
            name: "sdf_text_shadow_vector",
            entries: vec![("vs_main", "vertex"), ("fs_main", "fragment")],
        },
        ShaderConfig {
            name: "sdf_text2d_vector",
            entries: vec![("vs_main", "vertex"), ("fs_main", "fragment")],
        },
        ShaderConfig {
            name: "sdf_todomvc",
            entries: vec![("vs_main", "vertex"), ("fs_main", "fragment")],
        },
        ShaderConfig {
            name: "sdf_todomvc_3d",
            entries: vec![("vs_main", "vertex"), ("fs_main", "fragment")],
        },
    ];

    for shader in &shaders {
        compile_shader(&shaders_dir, &out_dir, &slangc, &ld_library_path, shader)?;
    }

    // Generate combined Rust bindings
    generate_bindings(&out_dir, &shaders)?;

    Ok(())
}

struct ShaderConfig {
    name: &'static str,
    entries: Vec<(&'static str, &'static str)>, // (entry_point, stage)
}

fn compile_shader(
    shaders_dir: &PathBuf,
    out_dir: &PathBuf,
    slangc: &PathBuf,
    ld_library_path: &str,
    config: &ShaderConfig,
) -> Result<()> {
    let slang_file = shaders_dir.join(format!("{}.slang", config.name));
    let wgsl_file = out_dir.join(format!("{}.wgsl", config.name));

    // Tell Cargo to rerun if shader changes
    println!("cargo:rerun-if-changed={}", slang_file.display());

    let mut combined_wgsl = format!("// Generated from {}.slang - DO NOT EDIT\n\n", config.name);

    // Compile each entry point
    for (entry_point, stage) in &config.entries {
        let temp_file = out_dir.join(format!("{}_{}_temp.wgsl", config.name, entry_point));

        let status = Command::new(slangc)
            .env("LD_LIBRARY_PATH", ld_library_path)
            .args([
                slang_file.to_str().unwrap(),
                "-entry",
                entry_point,
                "-stage",
                stage,
                "-target",
                "wgsl",
                "-o",
                temp_file.to_str().unwrap(),
            ])
            .status()
            .with_context(|| format!("Failed to run slangc for {} {}", config.name, entry_point))?;

        if !status.success() {
            anyhow::bail!("slangc failed for {} {}", config.name, entry_point);
        }

        let wgsl = std::fs::read_to_string(&temp_file)
            .with_context(|| format!("Failed to read {} output", entry_point))?;

        combined_wgsl.push_str(&format!("// {} shader\n{}\n\n", stage, wgsl.trim()));
    }

    std::fs::write(&wgsl_file, &combined_wgsl)
        .with_context(|| format!("Failed to write {}.wgsl", config.name))?;

    Ok(())
}

fn generate_bindings(out_dir: &PathBuf, shaders: &[ShaderConfig]) -> Result<()> {
    let bindings_file = out_dir.join("shader_bindings.rs");

    let mut builder = wgsl_bindgen::WgslBindgenOptionBuilder::default();
    builder
        .workspace_root(out_dir.to_string_lossy().to_string())
        .serialization_strategy(wgsl_bindgen::WgslTypeSerializeStrategy::Bytemuck)
        .output(bindings_file.to_string_lossy().to_string())
        .emit_rerun_if_change(false);

    for shader in shaders {
        let wgsl_file = out_dir.join(format!("{}.wgsl", shader.name));
        builder.add_entry_point(wgsl_file.to_string_lossy().to_string());
    }

    builder.build()?.generate()?;

    // Post-process: convert inner attributes (#![...]) to outer attributes (#[...])
    let bindings_content =
        std::fs::read_to_string(&bindings_file).context("Failed to read generated bindings")?;
    let fixed_content = bindings_content.replace("#![allow(", "#[allow(");
    std::fs::write(&bindings_file, fixed_content).context("Failed to write fixed bindings")?;

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
