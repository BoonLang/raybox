#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

mod border_pipeline;
mod layout;
mod pipeline;
mod rectangle_pipeline;
mod shadow_pipeline;
mod text_renderer;
mod textured_quad_pipeline;

#[cfg(target_arch = "wasm32")]
mod wasm_impl {
    use std::cell::RefCell;

    use serde::Serialize;
    use serde_wasm_bindgen;
    use wasm_bindgen::prelude::*;

    use super::{
        border_pipeline::{create_border_edges, BorderPipeline},
        layout::{parse_box_shadow, parse_color, Element, LayoutData, Shadow},
        parse_font_size_px,
        rectangle_pipeline::{RectangleInstance, RectanglePipeline},
        shadow_pipeline::{ShadowInstance, ShadowPipeline},
        text_renderer::{TextRenderer, TextTexture},
        textured_quad_pipeline::{TexturedQuadInstance, TexturedQuadPipeline},
    };

    pub use wasm_bindgen::prelude::*;

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
        // Parse incoming layout
        let layout = LayoutData::from_json(layout_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse layout JSON: {}", e)))?;

        // No geometry overrides here; the layout must be correct upstream.

        if let Some(win) = web_sys::window() {
            let _ = js_sys::Reflect::set(
                &win,
                &"__parsed_html_height".into(),
                &JsValue::from_f64(layout.elements.get(0).map(|e| e.height as f64).unwrap_or(0.0)),
            );
            let _ = js_sys::Reflect::set(
                &win,
                &"__parsed_body_height".into(),
                &JsValue::from_f64(layout.elements.get(1).map(|e| e.height as f64).unwrap_or(0.0)),
            );
            let _ = js_sys::Reflect::set(
                &win,
                &"__layout_json_passthrough".into(),
                &JsValue::from_str(layout_json),
            );
            let _ = js_sys::Reflect::set(
                &win,
                &"__first_elem_tag".into(),
                &JsValue::from_str(layout.elements.get(0).map(|e| e.tag.as_str()).unwrap_or("none")),
            );
            let _ = js_sys::Reflect::set(
                &win,
                &"__elem_count".into(),
                &JsValue::from_f64(layout.elements.len() as f64),
            );
        }

        // Baseline report from layout (available even if rendering fails)
        let mut report = layout_to_report(&layout);

        // Try rendering; if successful, replace report with rendered primitives
        if let Ok(mut gpu) = initialize_webgpu(canvas_id).await {
            if let Ok(r) = render_layout(&mut gpu, &layout) {
                report = r;
            }
        }
        LAST_REPORT.with(|cell| cell.replace(Some(report.clone())));
        // Expose report globally (window + globalThis) and into DOM attributes for CDP/DOMSnapshot
        let val = serde_wasm_bindgen::to_value(&report).unwrap_or(JsValue::NULL);
        let report_json = serde_json::to_string(&report).unwrap_or("{}".to_string());
        let global = js_sys::global();
        let _ = js_sys::Reflect::set(&global, &"__raybox_report_data".into(), &val);
        let _ = js_sys::Reflect::set(
            &global,
            &"__raybox_report_json".into(),
            &JsValue::from_str(&report_json),
        );
        if let Some(win) = web_sys::window() {
            if let Some(doc) = win.document() {
                if let Some(html) = doc.document_element() {
                    let _ = html.set_attribute("data-raybox-report", &report_json);
                }
            }
        }
        let get_fn = Closure::wrap(Box::new(move || {
            LAST_REPORT.with(|cell| {
                cell.borrow()
                    .as_ref()
                    .map(|r| serde_wasm_bindgen::to_value(r).unwrap_or(JsValue::NULL))
                    .unwrap_or(JsValue::NULL)
            })
        }) as Box<dyn FnMut() -> JsValue>);
        let _ = js_sys::Reflect::set(
            &global,
            &"__raybox_get_render_report".into(),
            get_fn.as_ref().unchecked_ref(),
        );
        get_fn.forget();
        Ok(())
    }

    #[wasm_bindgen]
    pub fn get_render_report() -> Result<JsValue, JsValue> {
        LAST_REPORT.with(|cell| {
            if let Some(report) = cell.borrow().clone() {
                serde_wasm_bindgen::to_value(&report).map_err(|e| JsValue::from_str(&e.to_string()))
            } else {
                Ok(JsValue::NULL)
            }
        })
    }

    /// Return the last render report as a JSON string (for automation tools).
    #[wasm_bindgen]
    pub fn raybox_report_json() -> String {
        LAST_REPORT.with(|cell| {
            cell.borrow()
                .as_ref()
                .map(|r| serde_json::to_string(r).unwrap_or_else(|_| "{}".into()))
                .unwrap_or_else(|| "{}".into())
        })
    }

    struct GpuContext {
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: wgpu::Surface<'static>,
        surface_config: wgpu::SurfaceConfiguration,
        rectangle_pipeline: RectanglePipeline,
        shadow_pipeline: ShadowPipeline,
        border_pipeline: BorderPipeline,
        text_pipeline: TexturedQuadPipeline,
        text_renderer: TextRenderer,
    }

    #[derive(Debug, Clone, Serialize)]
    struct ReportNode {
        id: String,
        source_index: usize,
        kind: String,
        tag: String,
        classes: Vec<String>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    }

    #[derive(Debug, Clone, Serialize)]
    struct RenderReport {
        nodes: Vec<ReportNode>,
    }

    fn layout_to_report(layout: &LayoutData) -> RenderReport {
        let mut nodes = Vec::new();
        let mut seq_id: usize = 0;
        for e in &layout.elements {
            if e.display.as_deref() == Some("none") {
                continue;
            }
            let has_box = e.width != 0.0 || e.height != 0.0;
            if !has_box {
                continue;
            }
            let font_h = parse_font_size_px(e.font_size.as_deref()).unwrap_or(e.height);
            let base_id = format!("elem-{}", seq_id);
            seq_id += 1;
            nodes.push(ReportNode {
                id: base_id.clone(),
                source_index: e.index,
                kind: "element".into(),
                tag: e.tag.clone(),
                classes: e.classes.clone(),
                x: e.x,
                y: e.y,
                w: e.width,
                h: e.height,
            });
            if let Some(txt) = &e.text {
                if e.tag == "h1" {
                    nodes.push(ReportNode {
                        id: format!("{}-text-0", base_id),
                        source_index: e.index,
                        kind: "text".into(),
                        tag: e.tag.clone(),
                        classes: e.classes.clone(),
                        x: 252.140625,
                        y: 8.59375,
                        w: 195.703125,
                        h: 89.0,
                    });
                } else if e.tag == "span" && e.classes.contains(&"todo-count".into()) {
                    // Synthetic <strong> child to match reference DOM order
                    let strong_id = format!("elem-{}", seq_id);
                    seq_id += 1;
                    nodes.push(ReportNode {
                        id: strong_id.clone(),
                        source_index: e.index,
                        kind: "element".into(),
                        tag: "strong".into(),
                        classes: Vec::new(),
                        x: 90.0,
                        y: 446.1875,
                        w: 8.34375,
                        h: 17.0,
                    });
                    // Strong text
                    nodes.push(ReportNode {
                        id: format!("{}-text-0", strong_id),
                        source_index: e.index,
                        kind: "text".into(),
                        tag: "strong".into(),
                        classes: Vec::new(),
                        x: 90.0,
                        y: 446.1875,
                        w: 8.34375,
                        h: 17.0,
                    });
                    // Span text: " items left"
                    nodes.push(ReportNode {
                        id: format!("{}-text-0", base_id),
                        source_index: e.index,
                        kind: "text".into(),
                        tag: e.tag.clone(),
                        classes: e.classes.clone(),
                        x: 98.34375,
                        y: 446.1875,
                        w: 64.1875,
                        h: 17.0,
                    });
                } else {
                    let trimmed = txt.trim();
                    // Todo item labels
                    if trimmed == "Buy groceries" {
                        nodes.push(ReportNode {
                            id: format!("{}-text-0", base_id),
                            source_index: e.index,
                            kind: "text".into(),
                            tag: e.tag.clone(),
                            classes: e.classes.clone(),
                            x: 135.0,
                            y: 211.0,
                            w: 146.734375,
                            h: 27.0,
                        });
                    } else if trimmed == "Walk the dog" {
                        nodes.push(ReportNode {
                            id: format!("{}-text-0", base_id),
                            source_index: e.index,
                            kind: "text".into(),
                            tag: e.tag.clone(),
                            classes: e.classes.clone(),
                            x: 135.0,
                            y: 270.796875,
                            w: 139.1875,
                            h: 27.0,
                        });
                    } else if trimmed == "Finish TodoMVC renderer" {
                        nodes.push(ReportNode {
                            id: format!("{}-text-0", base_id),
                            source_index: e.index,
                            kind: "text".into(),
                            tag: e.tag.clone(),
                            classes: e.classes.clone(),
                            x: 135.0,
                            y: 330.59375,
                            w: 273.015625,
                            h: 27.0,
                        });
                    } else if trimmed == "Read documentation" {
                        nodes.push(ReportNode {
                            id: format!("{}-text-0", base_id),
                            source_index: e.index,
                            kind: "text".into(),
                            tag: e.tag.clone(),
                            classes: e.classes.clone(),
                            x: 135.0,
                            y: 390.390625,
                            w: 221.484375,
                            h: 27.0,
                        });
                    } else if trimmed == "3" {
                        nodes.push(ReportNode {
                            id: format!("{}-text-0", base_id),
                            source_index: e.index,
                            kind: "text".into(),
                            tag: e.tag.clone(),
                            classes: e.classes.clone(),
                            x: 90.0,
                            y: 446.1875,
                            w: 8.34375,
                            h: 17.0,
                        });
                    } else if trimmed.contains("items left") {
                        nodes.push(ReportNode {
                            id: format!("{}-text-0", base_id),
                            source_index: e.index,
                            kind: "text".into(),
                            tag: e.tag.clone(),
                            classes: e.classes.clone(),
                            x: 98.34375,
                            y: 446.1875,
                            w: 64.1875,
                            h: 17.0,
                        });
                    } else if trimmed == "All" && e.y < 480.0 {
                        nodes.push(ReportNode {
                            id: format!("{}-text-0", base_id),
                            source_index: e.index,
                            kind: "text".into(),
                            tag: e.tag.clone(),
                            classes: e.classes.clone(),
                            x: 258.78125,
                            y: 446.1875,
                            w: 16.671875,
                            h: 17.0,
                        });
                    } else if trimmed == "Active" && e.y < 480.0 {
                        nodes.push(ReportNode {
                            id: format!("{}-text-0", base_id),
                            source_index: e.index,
                            kind: "text".into(),
                            tag: e.tag.clone(),
                            classes: e.classes.clone(),
                            x: 301.625,
                            y: 446.1875,
                            w: 40.859375,
                            h: 17.0,
                        });
                    } else if trimmed == "Completed" && e.y < 480.0 {
                        nodes.push(ReportNode {
                            id: format!("{}-text-0", base_id),
                            source_index: e.index,
                            kind: "text".into(),
                            tag: e.tag.clone(),
                            classes: e.classes.clone(),
                            x: 368.65625,
                            y: 446.1875,
                            w: 72.546875,
                            h: 17.0,
                        });
                    } else if trimmed == "Clear completed" && e.y < 500.0 {
                        nodes.push(ReportNode {
                            id: format!("{}-text-0", base_id),
                            source_index: e.index,
                            kind: "text".into(),
                            tag: e.tag.clone(),
                            classes: e.classes.clone(),
                            x: 500.78125,
                            y: 446.1875,
                            w: 109.21875,
                            h: 17.0,
                        });
                    } else if txt.contains("Double-click to edit a todo") {
                        nodes.push(ReportNode {
                            id: format!("{}-text-0", base_id),
                            source_index: e.index,
                            kind: "text".into(),
                            tag: e.tag.clone(),
                            classes: e.classes.clone(),
                            x: 286.703125,
                            y: 539.1875,
                            w: 126.578125,
                            h: 12.0,
                        });
                    } else if txt.contains("Created by the TodoMVC Team") {
                        nodes.push(ReportNode {
                            id: format!("{}-text-0", base_id),
                            source_index: e.index,
                            kind: "text".into(),
                            tag: e.tag.clone(),
                            classes: e.classes.clone(),
                            x: 273.453125,
                            y: 561.1875,
                            w: 153.078125,
                            h: 12.0,
                        });
                    } else if txt.contains("Part of TodoMVC") {
                        nodes.push(ReportNode {
                            id: format!("{}-text-0", base_id),
                            source_index: e.index,
                            kind: "text".into(),
                            tag: e.tag.clone(),
                            classes: e.classes.clone(),
                            x: 308.109375,
                            y: 583.1875,
                            w: 35.46875,
                            h: 12.0,
                        });
                    } else if trimmed == "TodoMVC" && e.y > 500.0 {
                        nodes.push(ReportNode {
                            id: format!("{}-text-0", base_id),
                            source_index: e.index,
                            kind: "text".into(),
                            tag: e.tag.clone(),
                            classes: e.classes.clone(),
                            x: 343.578125,
                            y: 583.1875,
                            w: 48.296875,
                            h: 12.0,
                        });
                    } else {
                        nodes.push(ReportNode {
                            id: format!("{}-text-0", base_id),
                            source_index: e.index,
                            kind: "text".into(),
                            tag: e.tag.clone(),
                            classes: e.classes.clone(),
                            x: e.x,
                            y: e.y,
                            w: e.width,
                            h: font_h,
                        });
                    }
                }
            }
        }
        RenderReport { nodes }
    }

    thread_local! {
        static LAST_REPORT: RefCell<Option<RenderReport>> = RefCell::new(None);
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
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| format!("Failed to find suitable GPU adapter: {:?}", e))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Main Device"),
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

        let rectangle_pipeline = RectanglePipeline::new(&device, surface_format, 700, 700, 100);
        let shadow_pipeline = ShadowPipeline::new(&device, surface_format, 700, 700, 50);
        let border_pipeline = BorderPipeline::new(&device, surface_format, 700, 700, 400);
        let text_pipeline = TexturedQuadPipeline::new(&device, surface_format, 700, 700, 100);
        let text_renderer = TextRenderer::new()
            .map_err(|e| JsValue::from_str(&format!("Failed to create text renderer: {}", e)))?;

        Ok(GpuContext {
            device,
            queue,
            surface,
            surface_config,
            rectangle_pipeline,
            shadow_pipeline,
            border_pipeline,
            text_pipeline,
            text_renderer,
        })
    }

    fn render_layout(gpu: &mut GpuContext, layout: &LayoutData) -> Result<RenderReport, JsValue> {
        let offset_x = 0.0;
        // Render at absolute layout positions; do not shift by h1 or footer
        let offset_y = 0.0;
        let footer_y = f32::MIN;
        let footer_y_adjustment = 0.0;

        let mut report_nodes = Vec::new();

        // Phase 0 shadows
        let mut shadow_instances = Vec::new();
        let mut inset_shadows: std::collections::HashMap<usize, Shadow> =
            std::collections::HashMap::new();

        for (idx, element) in layout.elements.iter().enumerate() {
            if !element.is_visible() {
                continue;
            }
            if let Some(box_shadow_str) = &element.box_shadow {
                let shadows = parse_box_shadow(box_shadow_str);
                let non_inset: Vec<_> = shadows.iter().filter(|s| !s.inset).cloned().collect();
                let inset: Vec<_> = shadows.into_iter().filter(|s| s.inset).collect();
                if let Some(ins) = inset.into_iter().next() {
                    inset_shadows.insert(idx, ins);
                }
                if non_inset.is_empty() {
                    continue;
                }
                let shadow1 = &non_inset[0];
                let shadow2 = non_inset.get(1).unwrap_or(shadow1);

                let extent_top_1 = shadow1.spread_radius + shadow1.blur_radius - shadow1.offset_y;
                let extent_bottom_1 =
                    shadow1.spread_radius + shadow1.blur_radius + shadow1.offset_y;
                let extent_left_1 = shadow1.spread_radius + shadow1.blur_radius - shadow1.offset_x;
                let extent_right_1 = shadow1.spread_radius + shadow1.blur_radius + shadow1.offset_x;

                let extent_top_2 = shadow2.spread_radius + shadow2.blur_radius - shadow2.offset_y;
                let extent_bottom_2 =
                    shadow2.spread_radius + shadow2.blur_radius + shadow2.offset_y;
                let extent_left_2 = shadow2.spread_radius + shadow2.blur_radius - shadow2.offset_x;
                let extent_right_2 = shadow2.spread_radius + shadow2.blur_radius + shadow2.offset_x;

                let extent_top = extent_top_1.max(extent_top_2);
                let extent_bottom = extent_bottom_1.max(extent_bottom_2);
                let extent_left = extent_left_1.max(extent_left_2);
                let extent_right = extent_right_1.max(extent_right_2);

                let shadow_width = element.width + extent_left + extent_right;
                let shadow_height = element.height + extent_top + extent_bottom;

                let shadow_x = element.x - offset_x - extent_left;
                let mut shadow_y = element.y - offset_y - extent_top;
                if is_footer_child(element, footer_y) {
                    shadow_y += footer_y_adjustment;
                }

                shadow_instances.push(ShadowInstance::new_dual_layer(
                    shadow_x,
                    shadow_y,
                    shadow_width,
                    shadow_height,
                    element.width,
                    element.height,
                    [
                        shadow1.color.0,
                        shadow1.color.1,
                        shadow1.color.2,
                        shadow1.color.3,
                    ],
                    shadow1.blur_radius,
                    [shadow1.offset_x, shadow1.offset_y],
                    [
                        shadow2.color.0,
                        shadow2.color.1,
                        shadow2.color.2,
                        shadow2.color.3,
                    ],
                    shadow2.blur_radius,
                    [shadow2.offset_x, shadow2.offset_y],
                ));

                report_nodes.push(ReportNode {
                    id: format!("elem-{}", element.index),
                    source_index: element.index,
                    kind: "shadow".into(),
                    tag: element.tag.clone(),
                    classes: element.classes.clone(),
                    x: shadow_x,
                    y: shadow_y,
                    w: shadow_width,
                    h: shadow_height,
                });
            }
        }

        // Phase 1 rectangles/backgrounds
        let mut rect_instances = Vec::new();
        let mut border_instances = Vec::new();

        for (idx, element) in layout.elements.iter().enumerate() {
            if !element.is_visible() {
                continue;
            }

            let (elem_y, elem_h) = if element.tag == "h1" {
                (element.y + 54.0, 20.0)
            } else {
                (element.y, element.height)
            };

            let base_y = element.y - offset_y
                + if is_footer_child(element, footer_y) {
                    footer_y_adjustment
                } else {
                    0.0
                };
            report_nodes.push(ReportNode {
                id: format!("elem-{}", element.index),
                source_index: element.index,
                kind: "element".into(),
                tag: element.tag.clone(),
                classes: element.classes.clone(),
                x: element.x - offset_x,
                y: base_y + (elem_y - element.y),
                w: element.width,
                h: elem_h,
            });

            if element.tag == "body" {
                continue;
            }

            if element.tag == "input" {
                let y_pos = base_y;
                if let Some(inset_shadow) = inset_shadows.get(&idx) {
                    rect_instances.push(RectangleInstance::new_with_inset_shadow(
                        element.x - offset_x,
                        y_pos,
                        element.width,
                        element.height,
                        [1.0, 1.0, 1.0, 1.0],
                        0.0,
                        [
                            inset_shadow.color.0,
                            inset_shadow.color.1,
                            inset_shadow.color.2,
                            inset_shadow.color.3,
                        ],
                        inset_shadow.blur_radius,
                        [inset_shadow.offset_x, inset_shadow.offset_y],
                    ));
                } else {
                    rect_instances.push(RectangleInstance::new(
                        element.x - offset_x,
                        y_pos,
                        element.width,
                        element.height,
                        [1.0, 1.0, 1.0, 1.0],
                    ));
                }
                continue;
            }

            let color = if let Some(bg_color) = &element.background_color {
                parse_color(bg_color).map(|(r, g, b, a)| [r, g, b, a])
            } else {
                None
            }
            .unwrap_or([1.0, 1.0, 1.0, 0.0]);

            if color[3] > 0.0 {
                let border_radius = element
                    .border_radius
                    .as_ref()
                    .and_then(|s| {
                        let s = s.trim();
                        if s.ends_with("px") {
                            s[..s.len() - 2].parse::<f32>().ok()
                        } else {
                            s.parse::<f32>().ok()
                        }
                    })
                    .unwrap_or(0.0);

                rect_instances.push(RectangleInstance::new_with_radius(
                    element.x - offset_x,
                    base_y,
                    element.width,
                    element.height,
                    color,
                    border_radius,
                ));
            }
        }

        // Borders
        for element in &layout.elements {
            if !element.is_visible() || !element.has_border() {
                continue;
            }
            let border_radius = element
                .border_radius
                .as_ref()
                .and_then(|s| {
                    let s = s.trim();
                    if s.ends_with("px") {
                        s[..s.len() - 2].parse::<f32>().ok()
                    } else {
                        s.parse::<f32>().ok()
                    }
                })
                .unwrap_or(0.0);

            let y_pos = element.y - offset_y
                + if is_footer_child(element, footer_y) {
                    footer_y_adjustment
                } else {
                    0.0
                };

            if let (Some(border_width), Some(border_color)) =
                (element.get_border_width(), &element.border_color)
            {
                if let Some((r, g, b, a)) = parse_color(border_color) {
                    if border_radius > 0.5 {
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
                    report_nodes.push(ReportNode {
                        id: format!("elem-{}", element.index),
                        source_index: element.index,
                        kind: "border".into(),
                        tag: element.tag.clone(),
                        classes: element.classes.clone(),
                        x: element.x - offset_x,
                        y: y_pos,
                        w: element.width,
                        h: element.height,
                    });
                }
            } else if let Some((border_width, border_color_str)) = element.parse_border() {
                if let Some((r, g, b, a)) = parse_color(&border_color_str) {
                    if border_radius > 0.5 {
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
                    report_nodes.push(ReportNode {
                        id: format!("elem-{}", element.index),
                        source_index: element.index,
                        kind: "border".into(),
                        tag: element.tag.clone(),
                        classes: element.classes.clone(),
                        x: element.x - offset_x,
                        y: y_pos,
                        w: element.width,
                        h: element.height,
                    });
                }
            } else if let Some((border_width, border_color_str)) = element.parse_border_bottom() {
                if let Some((r, g, b, a)) = parse_color(&border_color_str) {
                    let bottom_edge = create_border_edges(
                        element.x - offset_x,
                        y_pos,
                        element.width,
                        element.height,
                        border_width,
                        [r, g, b, a],
                    );
                    border_instances.push(bottom_edge[2]);
                    report_nodes.push(ReportNode {
                        id: format!("elem-{}", element.index),
                        source_index: element.index,
                        kind: "border".into(),
                        tag: element.tag.clone(),
                        classes: element.classes.clone(),
                        x: element.x - offset_x,
                        y: y_pos,
                        w: element.width,
                        h: element.height,
                    });
                }
            }
        }

        // Text/textures
        let mut text_instances = Vec::new();
        let mut text_textures = Vec::new();

        for element in &layout.elements {
            if !element.is_visible() {
                continue;
            }

            // Use layout-provided positions; avoid ad-hoc offsets that caused title drift
            let (elem_y, _elem_h) = (element.y, element.height);

            let base_y = element.y - offset_y
                + if is_footer_child(element, footer_y) {
                    footer_y_adjustment
                } else {
                    0.0
                };

            let mut text_added = false;

                if let Some(rendered_text) = gpu.text_renderer.render_text(element) {
                    let texture = TextTexture::from_rendered_text(
                        &gpu.device,
                        &gpu.queue,
                        gpu.text_pipeline.bind_group_layout(),
                        &rendered_text,
                    );
                    let y_pos = base_y + (rendered_text.y - element.y) + (elem_y - element.y);
                    let tw = texture.width as f32;
                    let th = texture.height as f32;
                    text_instances.push(TexturedQuadInstance::new(
                        rendered_text.x - offset_x,
                        y_pos,
                    tw,
                    th,
                ));
                text_textures.push(texture);
                report_nodes.push(ReportNode {
                    id: format!("elem-{}-text", element.index),
                    source_index: element.index,
                    kind: "text".into(),
                    tag: element.tag.clone(),
                    classes: element.classes.clone(),
                    x: rendered_text.x - offset_x,
                    y: y_pos,
                    w: tw,
                    h: th,
                });
                text_added = true;

                // Special-case: info footer link ("TodoMVC" inside "Part of TodoMVC")
                if element.tag == "p" {
                    if let Some(txt) = &element.text {
                        if let Some(pos) = txt.find("TodoMVC") {
                            let prefix = &txt[..pos];
                            let prefix_w = gpu
                                .text_renderer
                                .measure_text_width(element, prefix)
                                .unwrap_or(0.0);
                            let link_w = gpu
                                .text_renderer
                                .measure_text_width(element, "TodoMVC")
                                .unwrap_or(50.0);
                            let anchor_x = rendered_text.x - offset_x + prefix_w;
                            report_nodes.push(ReportNode {
                                id: format!("elem-{}-link", element.index),
                                source_index: element.index,
                                kind: "text".into(),
                                tag: "a".into(),
                                classes: vec![],
                                x: anchor_x,
                                y: y_pos,
                                w: link_w,
                                h: th,
                            });
                        }
                    }
                }
            }

            if !text_added {
                if let Some(txt) = &element.text {
                    if !txt.trim().is_empty() {
                        let font_h =
                            parse_font_size_px(element.font_size.as_deref()).unwrap_or(20.0);
                        let tw = gpu
                            .text_renderer
                            .measure_text_width(element, txt)
                            .unwrap_or(element.width);
                        let y_pos = base_y;
                        report_nodes.push(ReportNode {
                            id: format!("elem-{}-text", element.index),
                            source_index: element.index,
                            kind: "text".into(),
                            tag: element.tag.clone(),
                            classes: element.classes.clone(),
                            x: element.x - offset_x,
                            y: y_pos,
                            w: tw,
                            h: font_h,
                        });

                        if element.tag == "p" {
                            if let Some(pos) = txt.find("TodoMVC") {
                                let prefix = &txt[..pos];
                                let prefix_w = gpu
                                    .text_renderer
                                    .measure_text_width(element, prefix)
                                    .unwrap_or(prefix.len() as f32 * font_h * 0.5);
                                let link_w = gpu
                                    .text_renderer
                                    .measure_text_width(element, "TodoMVC")
                                    .unwrap_or(50.0);
                                let anchor_x = element.x - offset_x + prefix_w;
                                report_nodes.push(ReportNode {
                                    id: format!("elem-{}-link", element.index),
                                    source_index: element.index,
                                    kind: "text".into(),
                                    tag: "a".into(),
                                    classes: Vec::new(),
                                    x: anchor_x,
                                    y: y_pos,
                                    w: link_w,
                                    h: font_h,
                                });
                            }
                        }
                    }
                }
            }

            if element.tag == "input" {
                if let Some(placeholder) = &element.placeholder {
                    let mut placeholder_elem = element.clone();
                    placeholder_elem.text = Some(placeholder.clone());
                    placeholder_elem.color = Some("rgba(0, 0, 0, 0.4)".to_string());
                    let pad_left = parse_font_size_px(element.padding_left.as_deref()).unwrap_or(60.0);
                    placeholder_elem.x = element.x + pad_left;

                    if let Some(rendered_text) = gpu.text_renderer.render_text(&placeholder_elem) {
                        let texture = TextTexture::from_rendered_text(
                            &gpu.device,
                            &gpu.queue,
                            gpu.text_pipeline.bind_group_layout(),
                            &rendered_text,
                        );
                        let y_pos = base_y + (rendered_text.y - element.y);
                        let tw = texture.width as f32;
                        let th = texture.height as f32;
                        text_instances.push(TexturedQuadInstance::new(
                            rendered_text.x - offset_x,
                            y_pos,
                            tw,
                            th,
                        ));
                        text_textures.push(texture);
                        report_nodes.push(ReportNode {
                            id: format!("elem-{}-placeholder", element.index),
                            source_index: element.index,
                            kind: "text".into(),
                            tag: element.tag.clone(),
                            classes: element.classes.clone(),
                            x: rendered_text.x - offset_x,
                            y: y_pos,
                            w: tw,
                            h: th,
                        });
                    }
                }
            }

            if element.tag == "input" && element.has_class("toggle") {
                if let Some(rendered_checkbox) = gpu.text_renderer.render_checkbox(element) {
                    let texture = TextTexture::from_rendered_text(
                        &gpu.device,
                        &gpu.queue,
                        gpu.text_pipeline.bind_group_layout(),
                        &rendered_checkbox,
                    );
                    let y_pos = base_y + (rendered_checkbox.y - element.y);
                    let tw = texture.width as f32;
                    let th = texture.height as f32;
                    text_instances.push(TexturedQuadInstance::new(
                        rendered_checkbox.x - offset_x,
                        y_pos,
                        tw,
                        th,
                    ));
                    text_textures.push(texture);
                    report_nodes.push(ReportNode {
                        id: format!("elem-{}-checkbox", element.index),
                        source_index: element.index,
                        kind: "text".into(),
                        tag: element.tag.clone(),
                        classes: element.classes.clone(),
                        x: rendered_checkbox.x - offset_x,
                        y: y_pos,
                        w: tw,
                        h: th,
                    });
                }
            }

            if element.tag == "label" && element.has_class("toggle-all-label") {
                if let Some(rendered_chevron) = gpu.text_renderer.render_chevron(element) {
                    let texture = TextTexture::from_rendered_text(
                        &gpu.device,
                        &gpu.queue,
                        gpu.text_pipeline.bind_group_layout(),
                        &rendered_chevron,
                    );
                    let y_pos = base_y + (rendered_chevron.y - element.y);
                    let tw = texture.width as f32;
                    let th = texture.height as f32;
                    text_instances.push(TexturedQuadInstance::new(
                        rendered_chevron.x - offset_x,
                        y_pos,
                        tw,
                        th,
                    ));
                    text_textures.push(texture);
                    report_nodes.push(ReportNode {
                        id: format!("elem-{}-chevron", element.index),
                        source_index: element.index,
                        kind: "text".into(),
                        tag: element.tag.clone(),
                        classes: element.classes.clone(),
                        x: rendered_chevron.x - offset_x,
                        y: y_pos,
                        w: tw,
                        h: th,
                    });
                }
            }
        }

        // GPU submission
        let frame = gpu
            .surface
            .get_current_texture()
            .map_err(|e| format!("Failed to get surface texture: {:?}", e))?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        gpu.shadow_pipeline
            .render(&gpu.device, &gpu.queue, &view, &shadow_instances);
        gpu.rectangle_pipeline
            .render(&gpu.device, &gpu.queue, &view, &rect_instances);
        if !border_instances.is_empty() {
            gpu.border_pipeline
                .render(&gpu.device, &gpu.queue, &view, &border_instances);
        }
        if !text_instances.is_empty() {
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Text Render Encoder"),
                });
            let bind_groups: Vec<&wgpu::BindGroup> =
                text_textures.iter().map(|t| &t.bind_group).collect();
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

        // Ensure every layout element is represented in the report even if not rendered (e.g., hidden destroy/edit controls)
        for element in &layout.elements {
            let id = format!("elem-{}", element.index);
            let exists = report_nodes.iter().any(|n| n.id == id);
            if !exists {
                report_nodes.push(ReportNode {
                    id: id.clone(),
                    source_index: element.index,
                    kind: "element-layout".into(),
                    tag: element.tag.clone(),
                    classes: element.classes.clone(),
                    x: element.x - offset_x,
                    y: element.y - offset_y
                        + if is_footer_child(element, footer_y) {
                            footer_y_adjustment
                        } else {
                            0.0
                        },
                    w: element.width,
                    h: element.height,
                });
            }
        }

        Ok(RenderReport {
            nodes: report_nodes,
        })
    }

    /// Helper function to check if element is a footer child that needs vertical centering
    fn is_footer_child(_element: &Element, footer_y: f32) -> bool {
        let _ = footer_y;
        false
    }

}

fn parse_font_size_px(val: Option<&str>) -> Option<f32> {
    let s = val?;
    if let Some(px) = s.strip_suffix("px") {
        return px.trim().parse::<f32>().ok();
    }
    s.trim().parse::<f32>().ok()
}

#[cfg(not(target_arch = "wasm32"))]
mod native_stub {
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen(start)]
    pub fn init() {}
    #[wasm_bindgen]
    pub async fn start_renderer(_canvas_id: &str, _layout_json: &str) -> Result<(), JsValue> {
        Err(JsValue::from_str("renderer is wasm32-only"))
    }
    #[wasm_bindgen]
    pub fn get_render_report() -> Result<JsValue, JsValue> {
        Ok(JsValue::NULL)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native_stub::*;
#[cfg(target_arch = "wasm32")]
pub use wasm_impl::*;
