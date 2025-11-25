use anyhow::{anyhow, Context, Result};
use flate2::read::GzDecoder;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use tar::Archive;

// Match renderer's wasm-bindgen version
const VERSION: &str = "0.2.105";
const WASM_BINDGEN_PATH: &str = "web/bin/wasm-bindgen";

/// Check if wasm-bindgen is installed, install if needed
pub fn check_or_install_wasm_bindgen() -> Result<()> {
    if check_wasm_bindgen().is_ok() {
        return Ok(());
    }

    let platform = get_platform()?;
    let download_url = format!(
        "https://github.com/rustwasm/wasm-bindgen/releases/download/{}/wasm-bindgen-{}-{}.tar.gz",
        VERSION, VERSION, platform
    );

    println!("Downloading wasm-bindgen {}...", VERSION);
    log::info!("Platform: {}", platform);

    let tar_gz = download(&download_url)
        .with_context(|| format!("Failed to download wasm-bindgen from {}", download_url))?;

    unpack_wasm_bindgen(tar_gz).context("Failed to unpack wasm-bindgen")?;

    println!("✓ wasm-bindgen {} installed", VERSION);
    Ok(())
}

/// Run wasm-bindgen on compiled WASM
pub fn run_wasm_bindgen(wasm_path: &Path, out_dir: &Path, target: &str, debug: bool) -> Result<()> {
    let mut args = vec![
        "--target",
        target,
        "--no-typescript",
        "--weak-refs",
        "--out-dir",
        out_dir.to_str().unwrap(),
    ];

    if debug {
        args.push("--debug");
    }

    args.push(wasm_path.to_str().unwrap());

    log::info!("Running: {} {}", WASM_BINDGEN_PATH, args.join(" "));

    let status = StdCommand::new(WASM_BINDGEN_PATH)
        .args(&args)
        .status()
        .context("Failed to run wasm-bindgen")?;

    if !status.success() {
        anyhow::bail!("wasm-bindgen failed with status: {}", status);
    }

    Ok(())
}

// -- private --

fn check_wasm_bindgen() -> Result<()> {
    let expected_version = format!("wasm-bindgen {}", VERSION);

    let output = StdCommand::new(WASM_BINDGEN_PATH).arg("-V").output()?;

    let version_str = String::from_utf8_lossy(&output.stdout);

    if !version_str.starts_with(&expected_version) {
        anyhow::bail!(
            "wasm-bindgen version mismatch. Expected: {}, Found: {}",
            expected_version,
            version_str.trim()
        );
    }

    Ok(())
}

fn get_platform() -> Result<&'static str> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return Ok("x86_64-unknown-linux-musl");

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return Ok("aarch64-unknown-linux-gnu");

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return Ok("x86_64-apple-darwin");

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return Ok("aarch64-apple-darwin");

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return Ok("x86_64-pc-windows-msvc");

    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64"),
    )))]
    anyhow::bail!("Unsupported platform for wasm-bindgen pre-compiled binary");
}

fn download(url: &str) -> Result<Vec<u8>> {
    log::info!("Downloading: {}", url);

    let response = reqwest::blocking::get(url).with_context(|| format!("Failed to GET {}", url))?;

    if !response.status().is_success() {
        anyhow::bail!("Download failed with status: {}", response.status());
    }

    let bytes = response.bytes().context("Failed to read response bytes")?;

    Ok(bytes.to_vec())
}

fn unpack_wasm_bindgen(tar_gz: Vec<u8>) -> Result<()> {
    let tar = GzDecoder::new(tar_gz.as_slice());
    let mut archive = Archive::new(tar);

    // Create web/bin directory
    fs::create_dir_all("web/bin").context("Failed to create web/bin directory")?;

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;

        let file_stem = path
            .file_stem()
            .ok_or_else(|| anyhow!("Entry without a file name"))?;

        if file_stem != "wasm-bindgen" {
            continue;
        }

        let file_name = path
            .file_name()
            .ok_or_else(|| anyhow!("Entry without a file name"))?;

        let destination = PathBuf::from("web/bin").join(file_name);

        entry
            .unpack(&destination)
            .with_context(|| format!("Failed to unpack to {:?}", destination))?;

        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&destination)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&destination, perms)?;
        }

        log::info!("Extracted: {:?}", destination);
        return Ok(());
    }

    anyhow::bail!("wasm-bindgen binary not found in archive")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_platform() {
        // Just verify it doesn't panic
        let platform = get_platform();
        assert!(platform.is_ok());
    }
}
