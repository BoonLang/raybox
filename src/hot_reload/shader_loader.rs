//! Runtime shader loading for hot-reload
//!
//! Compiles shaders at runtime using slangc and loads them into wgpu.

use std::path::Path;
use std::process::{Command, Stdio};

/// Result of runtime shader compilation
#[derive(Debug)]
pub struct ShaderCompileResult {
    pub success: bool,
    pub wgsl_source: Option<String>,
    pub error: Option<String>,
}

/// Runtime shader loader
pub struct ShaderLoader {
    shaders_dir: String,
}

impl ShaderLoader {
    pub fn new(shaders_dir: impl Into<String>) -> Self {
        Self {
            shaders_dir: shaders_dir.into(),
        }
    }

    /// Compile a .slang shader to WGSL at runtime
    pub fn compile_shader(&self, shader_name: &str) -> ShaderCompileResult {
        let input_path = format!("{}/{}.slang", self.shaders_dir, shader_name);
        let output_path = format!("/tmp/{}.wgsl", shader_name);

        // Check if source exists
        if !Path::new(&input_path).exists() {
            return ShaderCompileResult {
                success: false,
                wgsl_source: None,
                error: Some(format!("Shader source not found: {}", input_path)),
            };
        }

        // Run slangc
        let result = Command::new("slangc")
            .args([
                &input_path,
                "-profile",
                "wgsl",
                "-target",
                "wgsl",
                "-o",
                &output_path,
                "-entry",
                "vs_main",
                "-entry",
                "fs_main",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();

        match result {
            Ok(output) => {
                if output.status.success() {
                    // Read the compiled WGSL
                    match std::fs::read_to_string(&output_path) {
                        Ok(wgsl) => ShaderCompileResult {
                            success: true,
                            wgsl_source: Some(wgsl),
                            error: None,
                        },
                        Err(e) => ShaderCompileResult {
                            success: false,
                            wgsl_source: None,
                            error: Some(format!("Failed to read compiled shader: {}", e)),
                        },
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    ShaderCompileResult {
                        success: false,
                        wgsl_source: None,
                        error: Some(format!("Shader compilation failed:\n{}", stderr)),
                    }
                }
            }
            Err(e) => ShaderCompileResult {
                success: false,
                wgsl_source: None,
                error: Some(format!("Failed to run slangc: {}", e)),
            },
        }
    }

    /// Create a wgpu shader module from WGSL source
    pub fn create_shader_module(
        device: &wgpu::Device,
        wgsl_source: &str,
        label: Option<&str>,
    ) -> wgpu::ShaderModule {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label,
            source: wgpu::ShaderSource::Wgsl(wgsl_source.into()),
        })
    }

    /// Compile and create shader module in one step
    pub fn load_shader(
        &self,
        device: &wgpu::Device,
        shader_name: &str,
    ) -> Result<wgpu::ShaderModule, String> {
        let result = self.compile_shader(shader_name);

        if !result.success {
            return Err(result
                .error
                .unwrap_or_else(|| "Unknown compilation error".to_string()));
        }

        let wgsl = result.wgsl_source.ok_or("No WGSL source produced")?;
        let module = Self::create_shader_module(device, &wgsl, Some(shader_name));

        Ok(module)
    }
}

impl Default for ShaderLoader {
    fn default() -> Self {
        Self::new("shaders")
    }
}

/// Trait for demos that support shader hot-reload
pub trait HotReloadable {
    /// Get the shader name used by this demo
    fn shader_name(&self) -> Option<&'static str>;

    /// Recreate the render pipeline with a new shader module
    fn recreate_pipeline(
        &mut self,
        device: &wgpu::Device,
        shader_module: &wgpu::ShaderModule,
        surface_format: wgpu::TextureFormat,
    );
}
