use anyhow::{anyhow, Context, Result};
use flate2::read::GzDecoder;
use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;
use tar::Archive;

const VERSION: &str = "123";
const WASM_OPT_PATH: &str = "web/bin/binaryen/bin/wasm-opt";

/// Check if wasm-opt is installed, install if needed
pub fn check_or_install_wasm_opt() -> Result<()> {
    if check_wasm_opt().is_ok() {
        return Ok(());
    }

    let platform = get_platform()?;
    let download_url = format!(
        "https://github.com/WebAssembly/binaryen/releases/download/version_{}/binaryen-version_{}-{}.tar.gz",
        VERSION, VERSION, platform
    );

    println!("Downloading wasm-opt (binaryen {})...", VERSION);
    log::info!("Platform: {}", platform);

    let tar_gz = download(&download_url)
        .with_context(|| format!("Failed to download wasm-opt from {}", download_url))?;

    unpack_wasm_opt(tar_gz)
        .context("Failed to unpack wasm-opt")?;

    println!("✓ wasm-opt (binaryen {}) installed", VERSION);
    Ok(())
}

/// Run wasm-opt on WASM file
pub fn run_wasm_opt(wasm_path: &Path, release: bool) -> Result<()> {
    let mut args = vec![
        wasm_path.to_str().unwrap(),
        "--output",
        wasm_path.to_str().unwrap(),
        "--enable-reference-types",
    ];

    if release {
        args.push("-Oz"); // Maximum size optimization
    }

    log::info!("Running: {} {}", WASM_OPT_PATH, args.join(" "));

    let status = StdCommand::new(WASM_OPT_PATH)
        .args(&args)
        .status()
        .context("Failed to run wasm-opt")?;

    if !status.success() {
        anyhow::bail!("wasm-opt failed with status: {}", status);
    }

    Ok(())
}

// -- private --

fn check_wasm_opt() -> Result<()> {
    let expected_version = format!("wasm-opt version {}", VERSION);

    let output = StdCommand::new(WASM_OPT_PATH)
        .arg("--version")
        .output()?;

    let version_str = String::from_utf8_lossy(&output.stdout);

    if !version_str.starts_with(&expected_version) {
        anyhow::bail!(
            "wasm-opt version mismatch. Expected: {}, Found: {}",
            expected_version,
            version_str.trim()
        );
    }

    Ok(())
}

fn get_platform() -> Result<&'static str> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return Ok("x86_64-linux");

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return Ok("x86_64-macos");

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return Ok("arm64-macos");

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return Ok("x86_64-windows");

    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64"),
    )))]
    anyhow::bail!("Unsupported platform for wasm-opt pre-compiled binary");
}

fn download(url: &str) -> Result<Vec<u8>> {
    log::info!("Downloading: {}", url);

    let response = reqwest::blocking::get(url)
        .with_context(|| format!("Failed to GET {}", url))?;

    if !response.status().is_success() {
        anyhow::bail!("Download failed with status: {}", response.status());
    }

    let bytes = response.bytes()
        .context("Failed to read response bytes")?;

    Ok(bytes.to_vec())
}

fn unpack_wasm_opt(tar_gz: Vec<u8>) -> Result<()> {
    let tar = GzDecoder::new(tar_gz.as_slice());
    let mut archive = Archive::new(tar);

    // Create directories
    fs::create_dir_all("web/bin/binaryen/bin")
        .context("Failed to create web/bin/binaryen/bin directory")?;
    fs::create_dir_all("web/bin/binaryen/lib")
        .context("Failed to create web/bin/binaryen/lib directory")?;

    let mut found_wasm_opt = false;

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;

        let file_name = path
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or_else(|| anyhow!("Entry without a valid file name"))?;

        let output_dir = match file_name {
            // Windows | Linux + Mac
            "wasm-opt.exe" | "wasm-opt" => {
                found_wasm_opt = true;
                "web/bin/binaryen/bin"
            }
            // The lib is required on Mac
            "libbinaryen.dylib" => "web/bin/binaryen/lib",
            // Windows lib (not always needed but include if present)
            "binaryen.lib" => "web/bin/binaryen/lib",
            // Linux lib (not always needed but include if present)
            "libbinaryen.a" => "web/bin/binaryen/lib",
            _ => continue,
        };

        let destination = Path::new(output_dir).join(file_name);

        entry.unpack(&destination)
            .with_context(|| format!("Failed to unpack to {:?}", destination))?;

        // Make executable on Unix
        #[cfg(unix)]
        if output_dir.ends_with("/bin") {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&destination)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&destination, perms)?;
        }

        log::info!("Extracted: {:?}", destination);
    }

    if !found_wasm_opt {
        anyhow::bail!("wasm-opt binary not found in archive");
    }

    // Verify installation
    check_wasm_opt()
        .context("wasm-opt installation verification failed")?;

    Ok(())
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
