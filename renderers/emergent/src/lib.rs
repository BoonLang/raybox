//! Emergent Renderer - SDF/Raymarching-based renderer
//!
//! This renderer uses Signed Distance Fields and raymarching to render UI elements.
//! Geometry "emerges" from spatial relationships rather than explicit declarations:
//! - Shadows emerge from real lighting
//! - Bevels emerge from smooth unions at contact zones
//! - Fillets emerge from smooth subtractions at recesses

mod font;
mod pipeline;
mod scene;
mod text_pipeline;

pub use font::{FontAtlas, TextLayout, PositionedGlyph, load_inter_atlas};

#[cfg(target_arch = "wasm32")]
mod wasm_impl {
    use wasm_bindgen::prelude::*;

    use super::pipeline::RaymarchPipeline;
    use super::scene::{Element, Scene};
    use super::text_pipeline::TextPipeline;

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
        text_pipeline: TextPipeline,
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

        let text_pipeline = TextPipeline::new(
            &device,
            &queue,
            surface_format,
            width as f32,
            height as f32,
        );

        Ok(GpuContext {
            device,
            queue,
            surface,
            surface_config,
            raymarch_pipeline,
            text_pipeline,
        })
    }

    fn create_demo_scene() -> Scene {
        let mut scene = Scene::new();

        // TodoMVC layout based on layout.json (700x700 viewport)
        // Body at x=75, y=130, w=550 -> center_x = 75 + 275 = 350

        // Background plane (#f5f5f5) - fills viewport
        scene.add_element(Element::new_box(
            [350.0, 350.0, -1.0],
            [1000.0, 1000.0, 0.01],
            [0.96, 0.96, 0.96], // rgb(245, 245, 245)
            0.0,
        ));

        // "todos" title - rendered via SDF text pipeline (not raymarched)
        // From layout.json: y=43.59, w=550, h=19.59, color rgb(184, 63, 69)
        // Rendered via text_pipeline.add_centered_text() in render_scene()

        // Main todoapp card (white)
        // From layout.json: x=75, y=130, w=550, h=345.1875
        // Center: (350, 302.6)
        scene.add_element(Element::new_box(
            [350.0, 302.6, 0.0],
            [275.0, 172.6, 0.01],
            [1.0, 1.0, 1.0], // white
            0.0,
        ));

        // Input field (new-todo)
        // From layout.json: x=75, y=130, w=550, h=65
        // Center: (350, 162.5)
        scene.add_element(Element::new_box(
            [350.0, 162.5, 0.1],
            [275.0, 32.5, 0.01],
            [1.0, 1.0, 1.0],
            0.0,
        ));

        // Chevron/toggle-all on left of input (x=75, w=45)
        // Center: x=75+22.5=97.5, y=162.5
        scene.add_element(Element::new_chevron(
            [97.5, 162.5, 0.2],
            20.0,  // size
            [0.68, 0.68, 0.68],  // gray
            0.0,
        ));

        // Input placeholder text bar - "What needs to be done?"
        // Positioned after paddingLeft: 60px, so x starts at ~135
        scene.add_element(Element::new_rounded_box(
            [310.0, 162.5, 0.2],
            [140.0, 8.0, 0.01],
            [0.90, 0.90, 0.90], // #e6e6e6 placeholder color
            3.0,
            0.0,
        ));

        // Todo items from layout.json
        // Checkboxes are 40x40, positioned at x=75
        // Checkbox centers: x=75+20=95, y=checkbox_y+20
        let todo_items = [
            // (item_y, checkbox_y, is_completed, has_border_below)
            (196.0, 205.4, false, true),   // "Buy groceries"
            (255.8, 265.2, false, true),   // "Walk the dog"
            (315.6, 325.0, true, true),    // "Finish TodoMVC renderer" - completed
            (375.4, 384.8, false, false),  // "Read documentation"
        ];

        for (item_y, checkbox_y, is_completed, has_border) in todo_items.iter() {
            // Checkbox center: x=95, y=checkbox_y+20
            let cb_center_x = 95.0;
            let cb_center_y = checkbox_y + 20.0;

            if *is_completed {
                // Green ring for completed (like reference)
                scene.add_element(Element::new_ring(
                    [cb_center_x, cb_center_y, 0.3],
                    12.0,
                    1.5,  // thin ring
                    [0.35, 0.72, 0.35], // green
                    0.0,
                ));
                // White checkmark inside
                scene.add_element(Element::new_checkmark(
                    [cb_center_x, cb_center_y, 0.35],
                    18.0,  // size
                    [0.35, 0.72, 0.35], // green checkmark
                    0.0,
                ));
            } else {
                // Gray hollow ring for unchecked (thinner)
                scene.add_element(Element::new_ring(
                    [cb_center_x, cb_center_y, 0.3],
                    12.0,
                    1.2,  // thinner ring like reference
                    [0.88, 0.88, 0.88], // lighter gray
                    0.0,
                ));
            }

            // Text label placeholder (after paddingLeft: 60px)
            // Label starts at x=135, center around x=320
            let text_y = item_y + 29.4; // Vertically centered in 58.8px height
            scene.add_element(Element::new_rounded_box(
                [320.0, text_y, 0.3],
                [120.0, 8.0, 0.01],
                if *is_completed {
                    [0.85, 0.85, 0.85] // lighter for completed (strikethrough effect)
                } else {
                    [0.72, 0.72, 0.72] // rgb(72, 72, 72) -> placeholder gray
                },
                3.0,
                0.0,
            ));

            // Border/separator line (1px solid #ededed)
            if *has_border {
                let border_y = item_y + 59.8; // Bottom of item
                scene.add_element(Element::new_box(
                    [350.0, border_y, 0.2],
                    [275.0, 0.5, 0.01],
                    [0.93, 0.93, 0.93], // #ededed
                    0.0,
                ));
            }
        }

        // Footer area
        // From layout.json: x=75, y=434.1875, w=550, h=41
        // Center: (350, 454.7)
        let footer_center_y = 454.7;

        // Footer background (subtle)
        scene.add_element(Element::new_box(
            [350.0, footer_center_y, 0.15],
            [275.0, 20.5, 0.01],
            [0.99, 0.99, 0.99],
            0.0,
        ));

        // "3 items left" text placeholder
        // From layout.json: x=90, y=445.19, w=72.5 -> center x=126
        scene.add_element(Element::new_rounded_box(
            [126.0, 455.0, 0.3],
            [36.0, 6.0, 0.01],
            [0.55, 0.55, 0.55],
            3.0,
            0.0,
        ));

        // Filter buttons - positioned per layout.json
        // "All" (selected): x=250.78, w=32.67 -> center x=267
        scene.add_element(Element::new_rounded_box(
            [267.0, 454.7, 0.3],
            [16.0, 12.5, 0.01],
            [0.95, 0.92, 0.92], // slight red tint for selected
            3.0,
            0.0,
        ));

        // "Active": x=293.63, w=56.86 -> center x=322
        scene.add_element(Element::new_rounded_box(
            [322.0, 454.7, 0.3],
            [28.0, 12.5, 0.01],
            [0.96, 0.96, 0.96],
            3.0,
            0.0,
        ));

        // "Completed": x=360.66, w=88.55 -> center x=405
        scene.add_element(Element::new_rounded_box(
            [405.0, 454.7, 0.3],
            [44.0, 12.5, 0.01],
            [0.96, 0.96, 0.96],
            3.0,
            0.0,
        ));

        scene
    }

    fn render_scene(gpu: &mut GpuContext, scene: &Scene) -> Result<(), JsValue> {
        // Update scene data on GPU
        gpu.raymarch_pipeline
            .update_scene(&gpu.device, &gpu.queue, scene);

        // Clear previous text batch
        gpu.text_pipeline.clear_batch();

        // Colors
        let title_color = [0.72, 0.25, 0.27, 1.0]; // rgb(184, 63, 69) - TodoMVC red
        let text_color = [0.31, 0.31, 0.31, 1.0];  // rgb(77, 77, 77) - dark gray
        let completed_color = [0.82, 0.82, 0.82, 1.0]; // rgb(210, 210, 210) - light gray for completed
        let placeholder_color = [0.90, 0.90, 0.90, 1.0]; // #e6e6e6 - placeholder
        let footer_color = [0.47, 0.47, 0.47, 1.0]; // rgb(119, 119, 119) - footer text

        // 1. "todos" title - SDF text (mathematically scaled for crisp rendering)
        // Reference: layout.json y=43.59, fontSize=80 (title), centered at x=350
        // Using SDF scaling fix: scale = font_size/glyph_size adjusts sdf_range
        gpu.text_pipeline.add_centered_text("todos", 350.0, 53.0, 80.0, title_color);

        // 2. Input placeholder - "What needs to be done?"
        // Reference: element y=130, paddingTop=16px, fontSize=24px
        // Text top = 130 + 16 = 146
        gpu.text_pipeline.add_text("What needs to be done?", 135.0, 146.0, 24.0, placeholder_color);

        // 3. Todo item labels (font size 24px, left aligned)
        // From layout.json: x=75, paddingLeft=60px → text x=135
        // Each item has paddingTop=15px, so text y = element y + 15
        // Item 1: y=196 → text y=211
        // Item 2: y=255.8 → text y=270.8
        // Item 3: y=315.6 → text y=330.6
        // Item 4: y=375.4 → text y=390.4
        let todo_items = [
            ("Buy groceries", 211.0, false),
            ("Walk the dog", 271.0, false),
            ("Finish TodoMVC renderer", 331.0, true),  // completed
            ("Read documentation", 390.0, false),
        ];

        for (text, y, is_completed) in todo_items.iter() {
            let color = if *is_completed { completed_color } else { text_color };
            gpu.text_pipeline.add_text(text, 135.0, *y, 24.0, color);
        }

        // 4. Footer text (fontSize=15px from layout.json)
        // Reference: "3 items left" at x=90, y=445.19
        gpu.text_pipeline.add_text("3 items left", 90.0, 445.0, 15.0, footer_color);

        // Filter buttons - fontSize=15px, each has padding 3px 7px
        // "All": x=250.78, width=32.67, so center_x = 250.78 + 16.34 ≈ 267
        // y=442.19, height=25, center_y = 442.19 + 12.5 ≈ 455
        gpu.text_pipeline.add_centered_text("All", 267.0, 455.0, 15.0, footer_color);
        // "Active": x=293.63, width=56.86, center_x ≈ 322
        gpu.text_pipeline.add_centered_text("Active", 322.0, 455.0, 15.0, footer_color);
        // "Completed": x=360.66, width=88.55, center_x ≈ 405
        gpu.text_pipeline.add_centered_text("Completed", 405.0, 455.0, 15.0, footer_color);

        // "Clear completed" - x=500.78, y=445.19
        gpu.text_pipeline.add_text("Clear completed", 501.0, 445.0, 15.0, footer_color);

        // Upload all text to GPU
        gpu.text_pipeline.flush_batch(&gpu.queue);

        // Get frame
        let frame = gpu
            .surface
            .get_current_texture()
            .map_err(|e| format!("Failed to get surface texture: {:?}", e))?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Render raymarched scene first
        gpu.raymarch_pipeline.render(&gpu.device, &gpu.queue, &view);

        // Render text on top
        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Text Encoder"),
        });
        gpu.text_pipeline.render(&mut encoder, &view);
        gpu.queue.submit(std::iter::once(encoder.finish()));

        frame.present();

        // Signal to screenshot tool that rendering is complete
        if let Some(win) = web_sys::window() {
            let _ = js_sys::Reflect::set(&win, &"__emergent_webgpu_ok".into(), &wasm_bindgen::JsValue::TRUE);
        }

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
