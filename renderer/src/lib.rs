mod layout;
mod pipeline;

pub use layout::*;
use pipeline::TrianglePipeline;

use wasm_bindgen::prelude::*;

/// Initialize panic hook for better error messages in the browser console
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
    log::info!("TodoMVC Canvas Renderer initialized - testing auto-reload!");
}

/// Entry point for the renderer
/// Called from JavaScript to start rendering
#[wasm_bindgen]
pub async fn start_renderer(canvas_id: &str, layout_json: &str) -> Result<(), JsValue> {
    log::info!("Starting renderer for canvas: {}", canvas_id);

    // Parse layout data
    let layout = LayoutData::from_json(layout_json)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse layout JSON: {}", e)))?;

    log::info!(
        "Loaded layout with {} elements",
        layout.elements.len()
    );

    // Initialize WebGPU
    let gpu = initialize_webgpu(canvas_id).await?;
    log::info!("WebGPU initialized successfully");

    // Render a test triangle
    render_triangle(&gpu)?;
    log::info!("Triangle rendered");

    Ok(())
}

struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    pipeline: TrianglePipeline,
}

async fn initialize_webgpu(canvas_id: &str) -> Result<GpuContext, JsValue> {
    use wasm_bindgen::JsCast;
    use web_sys::{HtmlCanvasElement, window};

    // Get the canvas element
    let window = window().ok_or("No window")?;
    let document = window.document().ok_or("No document")?;
    let canvas = document
        .get_element_by_id(canvas_id)
        .ok_or(format!("Canvas '{}' not found", canvas_id))?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| "Element is not a canvas")?;

    let width = canvas.width();
    let height = canvas.height();

    // Create WGPU instance
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::BROWSER_WEBGPU,
        ..Default::default()
    });

    // Create surface from canvas
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
        .map_err(|e| format!("Failed to create surface: {:?}", e))?;

    // Request adapter
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .map_err(|e| format!("Failed to find suitable GPU adapter: {:?}", e))?;

    // Request device
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("Main Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
            memory_hints: Default::default(),
            trace: Default::default(),
            experimental_features: Default::default(),
        })
        .await
        .map_err(|e| format!("Failed to create device: {:?}", e))?;

    // Configure surface
    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
        .formats
        .iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or(surface_caps.formats[0]);

    let surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width,
        height,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: surface_caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };

    surface.configure(&device, &surface_config);

    // Create render pipeline
    let pipeline = TrianglePipeline::new(&device, surface_format);

    Ok(GpuContext {
        device,
        queue,
        surface,
        surface_config,
        pipeline,
    })
}

fn render_triangle(gpu: &GpuContext) -> Result<(), JsValue> {
    let frame = gpu
        .surface
        .get_current_texture()
        .map_err(|e| format!("Failed to get surface texture: {:?}", e))?;

    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

    // Use the pipeline to render the triangle
    gpu.pipeline.render(&gpu.device, &gpu.queue, &view);

    frame.present();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_loading() {
        let json = r#"{
            "metadata": {
                "url": "http://example.com",
                "viewport": {
                    "width": 1920,
                    "height": 1080,
                    "devicePixelRatio": 1.0
                }
            },
            "elements": []
        }"#;

        let layout = LayoutData::from_json(json).expect("Failed to parse test JSON");
        assert_eq!(layout.metadata.viewport.width, 1920);
    }
}
