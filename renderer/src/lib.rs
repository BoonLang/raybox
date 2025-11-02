mod border_pipeline;
mod layout;
mod pipeline;
mod rectangle_pipeline;
mod shadow_pipeline;
mod text_renderer;
mod textured_quad_pipeline;

pub use layout::*;
use border_pipeline::{BorderPipeline, create_border_edges};
use pipeline::TrianglePipeline;
use rectangle_pipeline::{RectanglePipeline, RectangleInstance};
use shadow_pipeline::{ShadowPipeline, ShadowInstance};
use text_renderer::{TextRenderer, TextTexture};
use textured_quad_pipeline::{TexturedQuadPipeline, TexturedQuadInstance};

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
    let mut gpu = initialize_webgpu(canvas_id).await?;
    log::info!("WebGPU initialized successfully");

    // Render layout elements as rectangles
    render_layout(&mut gpu, &layout)?;
    log::info!("Layout rendered with {} elements", layout.elements.len());

    Ok(())
}

struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    pipeline: TrianglePipeline,
    rectangle_pipeline: RectanglePipeline,
    shadow_pipeline: ShadowPipeline,
    border_pipeline: BorderPipeline,
    text_pipeline: TexturedQuadPipeline,
    text_renderer: TextRenderer,
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

    // Create render pipelines
    let pipeline = TrianglePipeline::new(&device, surface_format);
    let rectangle_pipeline = RectanglePipeline::new(
        &device,
        surface_format,
        700, // Canvas width (full viewport)
        700, // Canvas height (full viewport)
        100, // initial capacity for 100 rectangles
    );
    let shadow_pipeline = ShadowPipeline::new(
        &device,
        surface_format,
        700, // Canvas width (full viewport)
        700, // Canvas height (full viewport)
        50, // initial capacity for 50 shadow layers (2 shadows × ~25 layers avg)
    );
    let border_pipeline = BorderPipeline::new(
        &device,
        surface_format,
        700, // Canvas width (full viewport)
        700, // Canvas height (full viewport)
        400, // initial capacity for 400 border edges (100 elements × 4 edges)
    );
    let text_pipeline = TexturedQuadPipeline::new(
        &device,
        surface_format,
        700, // Canvas width (full viewport)
        700, // Canvas height (full viewport)
        100, // initial capacity for 100 text elements
    );

    // Create text renderer
    let text_renderer = TextRenderer::new()
        .map_err(|e| JsValue::from_str(&format!("Failed to create text renderer: {}", e)))?;

    Ok(GpuContext {
        device,
        queue,
        surface,
        surface_config,
        pipeline,
        rectangle_pipeline,
        shadow_pipeline,
        border_pipeline,
        text_pipeline,
        text_renderer,
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

/// Helper function to check if element is a footer child that needs vertical centering
fn is_footer_child(element: &Element, footer_y: f32) -> bool {
    // Footer children are at the same y position as footer but are NOT the footer itself
    element.y == footer_y && !element.has_class("footer")
}

/// Convert layout elements to rectangle instances and render them
fn render_layout(gpu: &mut GpuContext, layout: &LayoutData) -> Result<(), JsValue> {
    // Calculate vertical offset to bring h1 title to top of canvas
    // Keep horizontal centering from reference layout (don't offset x-axis)
    let offset_x = 0.0;
    let offset_y = layout.elements.iter()
        .find(|e| e.tag == "h1")
        .map(|e| e.y)
        .unwrap_or(0.0);

    log::info!("Content area offset: ({}, {})", offset_x, offset_y);

    // Fix CSS layout bug: vertically center footer children
    // Footer children are at y=427 (top edge) but should be centered within footer (y=437)
    let (footer_y, footer_y_adjustment) = if let Some(footer) = layout.elements.iter().find(|e| e.has_class("footer")) {
        let footer_y = footer.y;
        let footer_height = footer.height;

        // Calculate adjustment needed to center 20px tall elements in 40px footer
        // Centered y = footer_y + (footer_height - element_height) / 2
        // Adjustment = centered_y - current_y = (footer_height - element_height) / 2
        let adjustment = (footer_height - 20.0) / 2.0;  // Most footer children are 20px tall
        (footer_y, adjustment)
    } else {
        (f32::MIN, 0.0)  // Use MIN as sentinel value if no footer found
    };

    log::info!("Footer vertical centering adjustment: {}px", footer_y_adjustment);

    // Phase 0: Collect shadow instances (rendered first, behind everything)
    let mut shadow_instances = Vec::new();

    for element in &layout.elements {
        // Skip invisible elements
        if !element.is_visible() {
            continue;
        }

        // Parse box-shadow if present
        if let Some(box_shadow_str) = &element.box_shadow {
            let shadows = parse_box_shadow(box_shadow_str);
            for shadow in shadows {
                // Skip inset shadows for now (requires stencil buffer)
                if shadow.inset {
                    continue;
                }

                // Approximate blur by slightly expanding shadow and adjusting opacity
                // CSS blur creates a gradient edge, not a size increase
                // Expand by a fraction of blur radius for soft edge effect
                let blur_expansion = shadow.blur_radius * 0.5; // Only expand by half the blur radius

                // Calculate shadow size (element size + spread * 2 + small blur expansion)
                let shadow_width = element.width + (shadow.spread_radius * 2.0) + (blur_expansion * 2.0);
                let shadow_height = element.height + (shadow.spread_radius * 2.0) + (blur_expansion * 2.0);

                // Calculate shadow position (element position + offset - expansion for centering)
                let shadow_x = element.x - offset_x + shadow.offset_x - shadow.spread_radius - blur_expansion;
                let mut shadow_y = element.y - offset_y + shadow.offset_y - shadow.spread_radius - blur_expansion;

                // Apply footer vertical centering adjustment
                if is_footer_child(element, footer_y) {
                    shadow_y += footer_y_adjustment;
                }

                // Content size is the element size plus spread (before blur expansion)
                let content_width = element.width + (shadow.spread_radius * 2.0);
                let content_height = element.height + (shadow.spread_radius * 2.0);

                shadow_instances.push(ShadowInstance::new(
                    shadow_x,
                    shadow_y,
                    shadow_width,
                    shadow_height,
                    [shadow.color.0, shadow.color.1, shadow.color.2, shadow.color.3],
                    content_width,
                    content_height,
                    shadow.blur_radius,
                ));
            }
        }
    }

    // Phase 1: Collect rectangle instances (backgrounds)
    let mut rect_instances = Vec::new();

    for element in &layout.elements {
        // Skip invisible elements
        if !element.is_visible() {
            continue;
        }

        // Special case: input elements default to white background
        if element.tag == "input" {
            let y_pos = element.y - offset_y + if is_footer_child(element, footer_y) { footer_y_adjustment } else { 0.0 };
            rect_instances.push(RectangleInstance::new(
                element.x - offset_x,
                y_pos,
                element.width,
                element.height,
                [1.0, 1.0, 1.0, 1.0], // White background for inputs
            ));
            continue;
        }

        // Parse background color
        let color = if let Some(bg_color) = &element.background_color {
            let (r, g, b, a) = parse_color(bg_color).unwrap_or((1.0, 1.0, 1.0, 1.0));
            [r, g, b, a]
        } else {
            [1.0, 1.0, 1.0, 0.0] // Transparent
        };

        // Only render background rectangles if element has visible background
        if color[3] > 0.0 {
            // Parse border-radius (e.g., "3px" -> 3.0)
            let border_radius = element.border_radius.as_ref()
                .and_then(|s| {
                    let s = s.trim();
                    if s.ends_with("px") {
                        s[..s.len() - 2].parse::<f32>().ok()
                    } else {
                        s.parse::<f32>().ok()
                    }
                })
                .unwrap_or(0.0);

            let y_pos = element.y - offset_y + if is_footer_child(element, footer_y) { footer_y_adjustment } else { 0.0 };
            rect_instances.push(RectangleInstance::new_with_radius(
                element.x - offset_x,
                y_pos,
                element.width,
                element.height,
                color,
                border_radius,
            ));
        }
    }

    // Phase 1.5: Collect border instances
    let mut border_instances = Vec::new();

    for element in &layout.elements {
        if !element.is_visible() || !element.has_border() {
            continue;
        }

        // Parse border-radius first (needed to decide rendering method)
        let border_radius = element.border_radius.as_ref()
            .and_then(|s| {
                let s = s.trim();
                if s.ends_with("px") {
                    s[..s.len() - 2].parse::<f32>().ok()
                } else {
                    s.parse::<f32>().ok()
                }
            })
            .unwrap_or(0.0);

        // Try standard border properties first
        if let (Some(border_width), Some(border_color)) =
            (element.get_border_width(), &element.border_color)
        {
            if let Some((r, g, b, a)) = parse_color(border_color) {
                let y_pos = element.y - offset_y + if is_footer_child(element, footer_y) { footer_y_adjustment } else { 0.0 };
                if border_radius > 0.5 {
                    // Render as rounded outline using rectangle pipeline
                    rect_instances.push(RectangleInstance::new_border_outline(
                        element.x - offset_x,
                        y_pos,
                        element.width,
                        element.height,
                        [r, g, b, a],
                        border_radius,
                        border_width,
                    ));
                } else {
                    // Render as 4 edges using border pipeline
                    let edges = create_border_edges(
                        element.x - offset_x,
                        y_pos,
                        element.width,
                        element.height,
                        border_width,
                        [r, g, b, a],
                    );
                    border_instances.extend_from_slice(&edges);
                }
            }
        }
        // Try border shorthand (e.g., "1px solid #ce4646")
        else if let Some((border_width, border_color_str)) = element.parse_border() {
            if let Some((r, g, b, a)) = parse_color(&border_color_str) {
                let y_pos = element.y - offset_y + if is_footer_child(element, footer_y) { footer_y_adjustment } else { 0.0 };
                if border_radius > 0.5 {
                    // Render as rounded outline using rectangle pipeline
                    rect_instances.push(RectangleInstance::new_border_outline(
                        element.x - offset_x,
                        y_pos,
                        element.width,
                        element.height,
                        [r, g, b, a],
                        border_radius,
                        border_width,
                    ));
                } else {
                    // Render as 4 edges using border pipeline
                    let edges = create_border_edges(
                        element.x - offset_x,
                        y_pos,
                        element.width,
                        element.height,
                        border_width,
                        [r, g, b, a],
                    );
                    border_instances.extend_from_slice(&edges);
                }
            }
        }
        // Try borderBottom shorthand
        else if let Some((border_width, border_color_str)) = element.parse_border_bottom() {
            if let Some((r, g, b, a)) = parse_color(&border_color_str) {
                let y_pos = element.y - offset_y + if is_footer_child(element, footer_y) { footer_y_adjustment } else { 0.0 };
                // borderBottom always uses 4-edge rendering (no rounded corners for single edge)
                let bottom_edge = create_border_edges(
                    element.x - offset_x,
                    y_pos,
                    element.width,
                    element.height,
                    border_width,
                    [r, g, b, a],
                );
                // Only take the bottom edge (index 2 of the 4 edges)
                border_instances.push(bottom_edge[2]);
            }
        }
    }

    // Phase 2: Render text elements to textures
    let mut text_instances = Vec::new();
    let mut text_textures = Vec::new();

    for element in &layout.elements {
        if !element.is_visible() {
            continue;
        }

        // Render text if element has text content
        if let Some(rendered_text) = gpu.text_renderer.render_text(element) {
            let texture = TextTexture::from_rendered_text(
                &gpu.device,
                &gpu.queue,
                gpu.text_pipeline.bind_group_layout(),
                &rendered_text,
            );

            let y_pos = rendered_text.y - offset_y + if is_footer_child(element, footer_y) { footer_y_adjustment } else { 0.0 };
            text_instances.push(TexturedQuadInstance::new(
                rendered_text.x - offset_x,
                y_pos,
                texture.width as f32,
                texture.height as f32,
            ));

            text_textures.push(texture);
        }

        // Render placeholder for input elements
        if element.tag == "input" {
            if let Some(placeholder) = &element.placeholder {
                // Create modified element for placeholder rendering
                let mut placeholder_elem = element.clone();
                placeholder_elem.text = Some(placeholder.clone());
                placeholder_elem.color = Some("rgba(0, 0, 0, 0.4)".to_string()); // Gray placeholder

                // Adjust x position for padding-left (60px from CSS)
                placeholder_elem.x = element.x + 60.0;

                if let Some(rendered_text) = gpu.text_renderer.render_text(&placeholder_elem) {
                    let texture = TextTexture::from_rendered_text(
                        &gpu.device,
                        &gpu.queue,
                        gpu.text_pipeline.bind_group_layout(),
                        &rendered_text,
                    );

                    let y_pos = rendered_text.y - offset_y + if is_footer_child(element, footer_y) { footer_y_adjustment } else { 0.0 };
                    text_instances.push(TexturedQuadInstance::new(
                        rendered_text.x - offset_x,
                        y_pos,
                        texture.width as f32,
                        texture.height as f32,
                    ));

                    text_textures.push(texture);
                }
            }
        }

        // Render checkboxes for toggle inputs
        if element.tag == "input" && element.has_class("toggle") {
            if let Some(rendered_checkbox) = gpu.text_renderer.render_checkbox(element) {
                let texture = TextTexture::from_rendered_text(
                    &gpu.device,
                    &gpu.queue,
                    gpu.text_pipeline.bind_group_layout(),
                    &rendered_checkbox,
                );

                let y_pos = rendered_checkbox.y - offset_y + if is_footer_child(element, footer_y) { footer_y_adjustment } else { 0.0 };
                text_instances.push(TexturedQuadInstance::new(
                    rendered_checkbox.x - offset_x,
                    y_pos,
                    texture.width as f32,
                    texture.height as f32,
                ));

                text_textures.push(texture);
            }
        }

        // Render chevron icon for toggle-all label
        if element.tag == "label" && element.has_class("toggle-all-label") {
            if let Some(rendered_chevron) = gpu.text_renderer.render_chevron(element) {
                let texture = TextTexture::from_rendered_text(
                    &gpu.device,
                    &gpu.queue,
                    gpu.text_pipeline.bind_group_layout(),
                    &rendered_chevron,
                );

                let y_pos = rendered_chevron.y - offset_y + if is_footer_child(element, footer_y) { footer_y_adjustment } else { 0.0 };
                text_instances.push(TexturedQuadInstance::new(
                    rendered_chevron.x - offset_x,
                    y_pos,
                    texture.width as f32,
                    texture.height as f32,
                ));

                text_textures.push(texture);
            }
        }
    }

    log::info!(
        "Rendering {} shadow layers, {} rectangles, {} border edges, and {} text elements out of {} total elements",
        shadow_instances.len(),
        rect_instances.len(),
        border_instances.len(),
        text_instances.len(),
        layout.elements.len()
    );

    // Get current surface texture
    let frame = gpu
        .surface
        .get_current_texture()
        .map_err(|e| format!("Failed to get surface texture: {:?}", e))?;

    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

    // Render shadows first (behind everything)
    // Always render to ensure screen is cleared, even with 0 shadows
    gpu.shadow_pipeline.render(
        &gpu.device,
        &gpu.queue,
        &view,
        &shadow_instances,
    );

    // Render rectangles (backgrounds) on top of shadows
    gpu.rectangle_pipeline.render(
        &gpu.device,
        &gpu.queue,
        &view,
        &rect_instances,
    );

    // Render borders on top of backgrounds
    if !border_instances.is_empty() {
        gpu.border_pipeline.render(
            &gpu.device,
            &gpu.queue,
            &view,
            &border_instances,
        );
    }

    // Render text on top
    if !text_instances.is_empty() {
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Text Render Encoder"),
            });

        let bind_groups: Vec<&wgpu::BindGroup> = text_textures
            .iter()
            .map(|t| &t.bind_group)
            .collect();

        gpu.text_pipeline.render(
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            &view,
            &text_instances,
            &bind_groups,
        );

        gpu.queue.submit(std::iter::once(encoder.finish()));
    }

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
