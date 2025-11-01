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
        js_sys::Reflect::set(
            &context_options,
            &"willReadFrequently".into(),
            &true.into(),
        )
        .map_err(|_| "Failed to set context options")?;

        let context = canvas
            .get_context_with_context_options("2d", &context_options)
            .map_err(|_| "Failed to get 2d context")?
            .ok_or("No 2d context")?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| "Context is not 2d")?;

        Ok(Self { canvas, context })
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

        // Measure text dimensions
        let metrics = self.context.measure_text(text).ok()?;
        let text_width = metrics.width() as u32;

        // Add padding for better rendering
        let padding = 4;
        let canvas_width = text_width + padding * 2;
        let canvas_height = (font_size * 1.5) as u32 + padding * 2; // 1.5x for line height

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
        self.context.clear_rect(
            0.0,
            0.0,
            canvas_width as f64,
            canvas_height as f64,
        );

        // Render text at baseline (text goes up from baseline)
        let baseline_y = font_size + padding as f32;
        self.context
            .fill_text(text, padding as f64, baseline_y as f64)
            .ok()?;

        // Extract image data
        let image_data = self
            .context
            .get_image_data(0.0, 0.0, canvas_width as f64, canvas_height as f64)
            .ok()?;

        let rgba_data = image_data.data().0;

        Some(RenderedText {
            x: element.x,
            y: element.y,
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
}

/// WebGPU texture for rendered text
pub struct TextTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
    pub width: u32,
    pub height: u32,
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
