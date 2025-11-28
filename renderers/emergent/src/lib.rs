//! Emergent Renderer - SDF/Raymarching-based renderer
//!
//! This renderer uses Signed Distance Fields and raymarching to render UI elements.
//! Geometry "emerges" from spatial relationships rather than explicit declarations:
//! - Shadows emerge from real lighting
//! - Bevels emerge from smooth unions at contact zones
//! - Fillets emerge from smooth subtractions at recesses

mod pipeline;
mod scene;

#[cfg(target_arch = "wasm32")]
mod wasm_impl {
    use wasm_bindgen::prelude::*;

    use super::pipeline::RaymarchPipeline;
    use super::scene::{Element, Scene};

    #[wasm_bindgen(start)]
    pub fn init() {
        console_error_panic_hook::set_once();
        log::info!("Emergent Renderer initialized");
    }

    /// Entry point for the renderer
    #[wasm_bindgen]
    pub async fn start_renderer(canvas_id: &str) -> Result<(), JsValue> {
        let mut gpu = initialize_webgpu(canvas_id).await?;

        // Create a demo scene with some basic elements
        let scene = create_demo_scene();

        // Render the scene
        render_scene(&mut gpu, &scene)?;

        Ok(())
    }

    struct GpuContext {
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: wgpu::Surface<'static>,
        #[allow(dead_code)]
        surface_config: wgpu::SurfaceConfiguration,
        raymarch_pipeline: RaymarchPipeline,
    }

    async fn initialize_webgpu(canvas_id: &str) -> Result<GpuContext, JsValue> {
        use wasm_bindgen::JsCast;
        use web_sys::{window, HtmlCanvasElement};

        let window = window().ok_or("No window")?;
        let document = window.document().ok_or("No document")?;
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or(format!("Canvas '{}' not found", canvas_id))?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| "Element is not a canvas")?;

        let width = canvas.width();
        let height = canvas.height();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });

        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| format!("Failed to create surface: {:?}", e))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| format!("Failed to find suitable GPU adapter: {:?}", e))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Emergent Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                ..Default::default()
            })
            .await
            .map_err(|e| format!("Failed to create device: {:?}", e))?;

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

        let raymarch_pipeline =
            RaymarchPipeline::new(&device, surface_format, width as f32, height as f32);

        Ok(GpuContext {
            device,
            queue,
            surface,
            surface_config,
            raymarch_pipeline,
        })
    }

    fn create_demo_scene() -> Scene {
        let mut scene = Scene::new();

        // TodoMVC-like layout for 700x700 viewport
        // Demonstrates emergent rendering: shadows from real lighting, no explicit shadow declarations

        // Background plane (the "surface" everything sits on)
        scene.add_element(Element::new_box(
            [350.0, 350.0, -20.0], // center, pushed back
            [400.0, 400.0, 5.0],   // half-extents
            [0.94, 0.94, 0.94],    // light gray background
            0.0,
        ));

        // Main card (white, raised above background)
        // The shadow emerges naturally from the lighting!
        scene.add_element(Element::new_box(
            [350.0, 300.0, 0.0],   // center
            [260.0, 200.0, 8.0],   // half-extents (thick card)
            [1.0, 1.0, 1.0],       // white
            15.0,                  // raised above background
        ));

        // Input field at top (slightly recessed)
        scene.add_element(Element::new_rounded_box(
            [350.0, 130.0, 12.0],  // positioned near top of card
            [240.0, 22.0, 3.0],    // half-extents
            [0.99, 0.99, 0.99],    // slightly off-white
            4.0,                   // corner radius
            5.0,                   // slightly raised
        ));

        // Todo items with checkboxes - positioned inside the card
        let todo_start_y = 190.0;
        let todo_spacing = 52.0;

        for i in 0..4 {
            let y = todo_start_y + (i as f32) * todo_spacing;
            let is_completed = i == 2; // Third item is completed

            // Checkbox - circular button (moved right to be inside card)
            let checkbox_x = 130.0;
            if is_completed {
                // Completed - green filled checkbox with shadow
                scene.add_element(Element::new_rounded_box(
                    [checkbox_x, y, 20.0],
                    [12.0, 12.0, 4.0],
                    [0.3, 0.72, 0.4], // green
                    12.0,             // fully rounded
                    10.0,             // raised - casts shadow
                ));
            } else {
                // Uncompleted - light gray circle
                scene.add_element(Element::new_rounded_box(
                    [checkbox_x, y, 18.0],
                    [11.0, 11.0, 3.0],
                    [0.82, 0.82, 0.82], // light gray
                    11.0,               // fully rounded
                    8.0,
                ));
            }

            // Todo item "text area" placeholder (shows where text would go)
            scene.add_element(Element::new_rounded_box(
                [350.0, y, 16.0],
                [180.0, 10.0, 2.0],
                [0.96, 0.96, 0.96], // very light gray
                3.0,
                6.0,
            ));

            // Separator line below each item (except last)
            if i < 3 {
                scene.add_element(Element::new_box(
                    [350.0, y + 24.0, 14.0],
                    [230.0, 0.4, 0.4],
                    [0.9, 0.9, 0.9], // separator color
                    4.0,
                ));
            }
        }

        // Footer area with filter buttons
        let footer_y = 430.0;

        // Footer background
        scene.add_element(Element::new_box(
            [350.0, footer_y, 13.0],
            [240.0, 16.0, 2.0],
            [0.98, 0.98, 0.98],
            4.0,
        ));

        // "All" filter button (selected - slightly raised)
        scene.add_element(Element::new_rounded_box(
            [265.0, footer_y, 17.0],
            [20.0, 10.0, 2.0],
            [1.0, 1.0, 1.0],
            4.0,
            6.0,
        ));

        // "Active" filter button
        scene.add_element(Element::new_rounded_box(
            [320.0, footer_y, 15.0],
            [28.0, 10.0, 1.5],
            [0.97, 0.97, 0.97],
            4.0,
            4.0,
        ));

        // "Completed" filter button
        scene.add_element(Element::new_rounded_box(
            [395.0, footer_y, 15.0],
            [40.0, 10.0, 1.5],
            [0.97, 0.97, 0.97],
            4.0,
            4.0,
        ));

        scene
    }

    fn render_scene(gpu: &mut GpuContext, scene: &Scene) -> Result<(), JsValue> {
        // Update scene data on GPU
        gpu.raymarch_pipeline
            .update_scene(&gpu.device, &gpu.queue, scene);

        // Get frame
        let frame = gpu
            .surface
            .get_current_texture()
            .map_err(|e| format!("Failed to get surface texture: {:?}", e))?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Render
        gpu.raymarch_pipeline.render(&gpu.device, &gpu.queue, &view);

        frame.present();
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native_stub {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(start)]
    pub fn init() {}

    #[wasm_bindgen]
    pub async fn start_renderer(_canvas_id: &str) -> Result<(), JsValue> {
        Err(JsValue::from_str("emergent-renderer is wasm32-only"))
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native_stub::*;
#[cfg(target_arch = "wasm32")]
pub use wasm_impl::*;
