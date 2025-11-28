use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};
use wgpu;

use crate::layout::Element;

/// Rendered text as an image with position information
pub struct RenderedText {
    pub x: f32,
    pub y: f32,
    pub width: u32,
    pub height: u32,
    pub image_data: Vec<u8>, // RGBA pixels
}

/// Helper to render text using Canvas2D API
pub struct TextRenderer {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
}

impl TextRenderer {
    pub fn new() -> Result<Self, String> {
        let document = web_sys::window()
            .ok_or("No window")?
            .document()
            .ok_or("No document")?;

        let canvas = document
            .create_element("canvas")
            .map_err(|_| "Failed to create canvas")?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| "Element is not a canvas")?;

        // Create context with willReadFrequently for better getImageData performance
        let context_options = js_sys::Object::new();
        js_sys::Reflect::set(&context_options, &"willReadFrequently".into(), &true.into())
            .map_err(|_| "Failed to set context options")?;

        let context = canvas
            .get_context_with_context_options("2d", &context_options)
            .map_err(|_| "Failed to get 2d context")?
            .ok_or("No 2d context")?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| "Context is not 2d")?;

        Ok(Self { canvas, context })
    }

    /// Measure text width with the element's font settings without rendering.
    pub fn measure_text_width(&mut self, element: &Element, text: &str) -> Option<f32> {
        let font_size = Self::parse_font_size(element.font_size.as_deref())?;
        let font_family = element
            .font_family
            .as_deref()
            .unwrap_or("Helvetica Neue, Helvetica, Arial, sans-serif");
        let font_weight = element.font_weight.as_deref().unwrap_or("300");
        let font_str = format!("{} {}px {}", font_weight, font_size, font_family);
        self.context.set_font(&font_str);
        let metrics = self.context.measure_text(text).ok()?;
        Some(metrics.width() as f32)
    }

    /// Render text element to a bitmap
    pub fn render_text(&mut self, element: &Element) -> Option<RenderedText> {
        let text = element.text.as_ref()?;
        if text.trim().is_empty() {
            return None;
        }

        // Parse font properties
        let font_size = Self::parse_font_size(element.font_size.as_deref())?;
        let font_family = element
            .font_family
            .as_deref()
            .unwrap_or("Helvetica Neue, Helvetica, Arial, sans-serif");
        let font_weight = element.font_weight.as_deref().unwrap_or("300");

        // Build CSS font string
        let font_str = format!("{} {}px {}", font_weight, font_size, font_family);

        // Set font for measurement
        self.context.set_font(&font_str);

        // Measure text dimensions, prefer provided font metrics when available
        let metrics = self.context.measure_text(text).ok()?;
        let text_width = metrics.width() as u32;
        let ascent = element
            .font_metrics
            .as_ref()
            .map(|m| m.ascent)
            .unwrap_or_else(|| {
                let raw = metrics.actual_bounding_box_ascent();
                if raw.is_nan() {
                    font_size * 0.8
                } else {
                    raw as f32
                }
            });
        let descent = element
            .font_metrics
            .as_ref()
            .map(|m| m.descent)
            .unwrap_or_else(|| {
                let raw = metrics.actual_bounding_box_descent();
                if raw.is_nan() {
                    font_size * 0.2
                } else {
                    raw as f32
                }
            });
        let text_height = ascent + descent;

        // Calculate x position based on text alignment
        let should_center = element.text_align.as_deref() == Some("center")
            || (element.tag == "a" && element.classes.contains(&"selected".to_string()));

        // Default x positioning
        let mut x_position = if should_center {
            // Center text within element width
            element.x + (element.width - text_width as f32) / 2.0
        } else {
            element.x
        };
        // Todo item labels should start to the right of the checkbox
        if element.tag == "label" && !element.classes.contains(&"toggle-all-label".to_string()) {
            // Prefer explicit padding-left from captured layout; fallback to proportional offset
            if let Some(pad) = Self::parse_px(element.padding_left.as_deref()) {
                x_position = element.x + pad;
            } else {
                x_position = element.x + element.width * 0.109;
            }
        }

        // Add padding for better rendering
        let padding = 4;
        let canvas_width = text_width + padding * 2;
        let canvas_height = (text_height.ceil() as u32) + padding * 2;

        // Resize canvas
        self.canvas.set_width(canvas_width);
        self.canvas.set_height(canvas_height);

        // Re-set font after canvas resize (canvas resets context)
        self.context.set_font(&font_str);

        // Parse text color
        let color = element.color.as_deref().unwrap_or("rgb(72, 72, 72)");
        self.context.set_fill_style_str(color);

        // Enable anti-aliasing
        self.context.set_image_smoothing_enabled(true);

        // Clear canvas
        self.context
            .clear_rect(0.0, 0.0, canvas_width as f64, canvas_height as f64);

        // Render text at baseline using measured ascent
        let baseline_y = padding as f32 + ascent;
        self.context
            .fill_text(text, padding as f64, baseline_y as f64)
            .ok()?;

        // Render text decoration (strikethrough)
        if let Some(decoration) = &element.text_decoration {
            if decoration == "line-through" {
                // Calculate line position (middle of text, slightly above center)
                let line_y = baseline_y - (font_size / 3.0);

                self.context.set_stroke_style_str(color);
                self.context.set_line_width(1.0);
                self.context.begin_path();
                self.context.move_to(padding as f64, line_y as f64);
                self.context
                    .line_to((padding + text_width) as f64, line_y as f64);
                self.context.stroke();
            }
        }

        // Extract image data
        let image_data = self
            .context
            .get_image_data(0.0, 0.0, canvas_width as f64, canvas_height as f64)
            .ok()?;

        let rgba_data = image_data.data().0;

        // Center text within the element's content box (accounting for padding if present).
        // If text is taller than the content, allow symmetric overflow.
        let pad_top = Self::parse_px(element.padding_top.as_deref()).unwrap_or(0.0);
        let pad_bottom = Self::parse_px(element.padding_bottom.as_deref()).unwrap_or(0.0);
        let content_height = (element.height - pad_top - pad_bottom).max(0.0);
        let text_top = element.y + pad_top + (content_height - text_height) / 2.0;
        let y_position = text_top - padding as f32;

        Some(RenderedText {
            x: x_position - padding as f32, // Subtract padding because text is drawn at offset `padding` inside canvas
            y: y_position,
            width: canvas_width,
            height: canvas_height,
            image_data: rgba_data,
        })
    }

    /// Parse font size from CSS string (e.g., "24px" -> 24.0)
    fn parse_font_size(font_size: Option<&str>) -> Option<f32> {
        let size_str = font_size?;
        if size_str.ends_with("px") {
            size_str[..size_str.len() - 2].parse::<f32>().ok()
        } else {
            None
        }
    }

    /// Parse a CSS px string into a float.
    fn parse_px(px: Option<&str>) -> Option<f32> {
        let s = px?;
        if s.ends_with("px") {
            s[..s.len() - 2].parse::<f32>().ok()
        } else {
            s.parse::<f32>().ok()
        }
    }

    /// Render a checkbox (circle with optional checkmark) to a bitmap
    pub fn render_checkbox(&mut self, element: &Element) -> Option<RenderedText> {
        let checked = element.checked.unwrap_or(false);
        // Render parameters kept together for easy tuning
        let spec = CheckboxSpec {
            size: 40,
            center_offset: 2.0,
            radius: 15.0,
            stroke_width: 1.0,
            check_stroke_width: 1.5,
            check_points: [(14.0, 24.0), (19.0, 28.0), (28.0, 14.0)],
        };
        let size = spec.size;

        // Set canvas size
        self.canvas.set_width(size);
        self.canvas.set_height(size);

        // Clear canvas
        self.context.clear_rect(0.0, 0.0, size as f64, size as f64);

        let center = size as f64 / 2.0 + spec.center_offset as f64;
        let radius = spec.radius as f64; // matches reference visual

        // Draw circle border
        self.context.begin_path();
        self.context
            .arc(center, center, radius, 0.0, 2.0 * std::f64::consts::PI)
            .ok()?;
        self.context.set_stroke_style_str("#dddddd");
        self.context.set_line_width(spec.stroke_width);
        self.context.stroke();

        if checked {
            // Draw checkmark
            self.context.set_stroke_style_str("#5dc2af");
            self.context.set_line_width(spec.check_stroke_width);
            self.context.begin_path();
            // Sharper V with shorter left arm
            self.context.move_to(spec.check_points[0].0, spec.check_points[0].1);
            self.context.line_to(spec.check_points[1].0, spec.check_points[1].1);
            self.context.line_to(spec.check_points[2].0, spec.check_points[2].1);
            self.context.stroke();
        }

        // Extract image data
        let image_data = self
            .context
            .get_image_data(0.0, 0.0, size as f64, size as f64)
            .ok()?;

        let rgba_data = image_data.data().0;

        Some(RenderedText {
            x: element.x + 1.0, // subtle nudge right without changing layout box
            y: element.y - 4.0, // fine-tune vertical centering
            width: size,
            height: size,
            image_data: rgba_data,
        })
    }

    /// Render a chevron icon (downward-pointing arrow) for toggle-all button
    pub fn render_chevron(&mut self, element: &Element) -> Option<RenderedText> {
        // Canvas size matches element size (45x65 from layout)
        let width = element.width as u32;
        let height = element.height as u32;

        // Set canvas size
        self.canvas.set_width(width);
        self.canvas.set_height(height);

        // Clear canvas
        self.context
            .clear_rect(0.0, 0.0, width as f64, height as f64);

        // Set font for chevron character
        let font_size = 22.0;
        self.context
            .set_font(&format!("{}px sans-serif", font_size));
        self.context.set_fill_style_str("#e6e6e6"); // Light gray

        // Render downward-pointing chevron
        // Use '❯' (U+276F) rotated 90 degrees
        let chevron_char = "❯";

        // Save context state
        self.context.save();

        // Move to center of canvas
        let center_x = width as f64 / 2.0;
        let center_y = height as f64 / 2.0;
        self.context.translate(center_x, center_y).ok()?;

        // Rotate 90 degrees clockwise to point down
        self.context.rotate(std::f64::consts::PI / 2.0).ok()?;

        // Measure text for centering
        let metrics = self.context.measure_text(chevron_char).ok()?;
        let text_width = metrics.width();

        // Draw chevron centered (baseline adjustment for vertical centering)
        self.context
            .fill_text(chevron_char, -text_width / 2.0, font_size as f64 / 3.0)
            .ok()?;

        // Restore context state
        self.context.restore();

        // Extract image data
        let image_data = self
            .context
            .get_image_data(0.0, 0.0, width as f64, height as f64)
            .ok()?;

        let rgba_data = image_data.data().0;

        Some(RenderedText {
            x: element.x,
            y: element.y,
            width,
            height,
            image_data: rgba_data,
        })
    }
}

/// WebGPU texture for rendered text
pub struct TextTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
    pub width: u32,
    pub height: u32,
}

/// Tunable checkbox drawing parameters derived from the 40x40 reference asset.
struct CheckboxSpec {
    size: u32,
    center_offset: f32,
    radius: f32,
    stroke_width: f64,
    check_stroke_width: f64,
    check_points: [(f64, f64); 3],
}

impl TextTexture {
    /// Create a texture from rendered text bitmap
    pub fn from_rendered_text(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bind_group_layout: &wgpu::BindGroupLayout,
        rendered_text: &RenderedText,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: rendered_text.width,
            height: rendered_text.height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Text Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload text bitmap to GPU
        queue.write_texture(
            texture.as_image_copy(),
            &rendered_text.image_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * rendered_text.width),
                rows_per_image: Some(rendered_text.height),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create sampler for texture filtering
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Text Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Text Bind Group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            texture,
            view,
            bind_group,
            width: rendered_text.width,
            height: rendered_text.height,
        }
    }
}
