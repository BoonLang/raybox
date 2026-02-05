//! Build invocation for hot-reload
//!
//! Handles cargo build and shader compilation.

use std::process::{Command, Output, Stdio};

/// Build result
#[derive(Debug)]
pub struct BuildResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub duration: std::time::Duration,
}

impl BuildResult {
    fn from_output(output: Output, duration: std::time::Duration) -> Self {
        Self {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            duration,
        }
    }
}

/// Build mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildMode {
    /// Build for native windowed mode
    Native,
    /// Build for web/WASM
    Web,
}

/// Builder for invoking cargo and shader compilation
pub struct Builder {
    project_root: String,
}

impl Builder {
    /// Create a new builder
    pub fn new(project_root: impl Into<String>) -> Self {
        Self {
            project_root: project_root.into(),
        }
    }

    /// Build the project
    pub fn build(&self, mode: BuildMode) -> BuildResult {
        let start = std::time::Instant::now();

        let output = match mode {
            BuildMode::Native => self.build_native(),
            BuildMode::Web => self.build_web(),
        };

        let duration = start.elapsed();

        match output {
            Ok(output) => BuildResult::from_output(output, duration),
            Err(e) => BuildResult {
                success: false,
                stdout: String::new(),
                stderr: format!("Failed to run build command: {}", e),
                duration,
            },
        }
    }

    /// Build native version
    fn build_native(&self) -> std::io::Result<Output> {
        log::info!("Building native...");
        Command::new("cargo")
            .arg("build")
            .arg("--bin")
            .arg("demos")
            .arg("--features")
            .arg("windowed,overlay")
            .current_dir(&self.project_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
    }

    /// Build web/WASM version
    fn build_web(&self) -> std::io::Result<Output> {
        log::info!("Building web...");
        Command::new("cargo")
            .arg("build")
            .arg("--lib")
            .arg("--target")
            .arg("wasm32-unknown-unknown")
            .arg("--release")
            .env("RUSTFLAGS", "--cfg=web_sys_unstable_apis")
            .current_dir(&self.project_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
    }

    /// Run wasm-bindgen after WASM build
    pub fn run_wasm_bindgen(&self) -> BuildResult {
        let start = std::time::Instant::now();

        let output = Command::new("wasm-bindgen")
            .arg("target/wasm32-unknown-unknown/release/raybox.wasm")
            .arg("--out-dir")
            .arg("pkg")
            .arg("--target")
            .arg("web")
            .current_dir(&self.project_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();

        let duration = start.elapsed();

        match output {
            Ok(output) => BuildResult::from_output(output, duration),
            Err(e) => BuildResult {
                success: false,
                stdout: String::new(),
                stderr: format!("Failed to run wasm-bindgen: {}", e),
                duration,
            },
        }
    }

    /// Full web build (cargo + wasm-bindgen)
    pub fn build_web_full(&self) -> BuildResult {
        let cargo_result = self.build(BuildMode::Web);
        if !cargo_result.success {
            return cargo_result;
        }

        let bindgen_result = self.run_wasm_bindgen();
        BuildResult {
            success: bindgen_result.success,
            stdout: format!("{}\n{}", cargo_result.stdout, bindgen_result.stdout),
            stderr: format!("{}\n{}", cargo_result.stderr, bindgen_result.stderr),
            duration: cargo_result.duration + bindgen_result.duration,
        }
    }

    /// Compile shaders only (via build.rs)
    pub fn compile_shaders(&self) -> BuildResult {
        let start = std::time::Instant::now();

        // Shaders are compiled via build.rs, so we just need to touch a shader
        // to trigger rebuild, or we can run cargo build
        let output = Command::new("cargo")
            .arg("build")
            .arg("--lib")
            .current_dir(&self.project_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();

        let duration = start.elapsed();

        match output {
            Ok(output) => BuildResult::from_output(output, duration),
            Err(e) => BuildResult {
                success: false,
                stdout: String::new(),
                stderr: format!("Failed to compile shaders: {}", e),
                duration,
            },
        }
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new(".")
    }
}
