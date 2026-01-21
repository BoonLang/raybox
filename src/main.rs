// Include generated shader bindings
#[allow(unused, non_snake_case, non_camel_case_types, non_upper_case_globals)]
mod shader_bindings {
    include!(concat!(env!("OUT_DIR"), "/shader_bindings.rs"));
}

mod camera;
#[cfg(not(feature = "windowed"))]
mod capture;
mod constants;
#[cfg(not(feature = "windowed"))]
mod sdf_renderer;
mod window_mode;

use anyhow::Result;

#[cfg(not(feature = "windowed"))]
use std::path::Path;

fn main() -> Result<()> {
    env_logger::init();

    #[cfg(feature = "windowed")]
    {
        log::info!("Running in windowed mode...");
        window_mode::windowed::run()
    }

    #[cfg(not(feature = "windowed"))]
    {
        // Run the renderer in headless mode
        pollster::block_on(run_headless())
    }
}

#[cfg(not(feature = "windowed"))]
async fn run_headless() -> Result<()> {
    log::info!("Creating SDF renderer...");
    let renderer = sdf_renderer::SdfRenderer::new().await?;

    log::info!("Rendering SDF scene...");
    let texture = renderer.render();

    log::info!("Capturing framebuffer to PNG...");
    let output_path = Path::new("output/screenshot.png");
    capture::capture_texture_to_png(&renderer.device, &renderer.queue, &texture, output_path)
        .await?;

    println!("Screenshot saved to {}", output_path.display());
    Ok(())
}
