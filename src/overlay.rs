//! Overlay renderer for stats and keybindings display
//!
//! Uses VectorTextRenderer for GPU-rendered text overlays.

use crate::text::{VectorFont, VectorGlyphInstance, VectorTextRenderer};
use anyhow::{Context, Result};

/// Overlay renderer for displaying stats and keybindings
pub struct OverlayRenderer {
    text_renderer: VectorTextRenderer,
    stats_instances: Vec<VectorGlyphInstance>,
    keybindings_instances: Vec<VectorGlyphInstance>,
    width: u32,
    height: u32,
}

impl OverlayRenderer {
    /// Create a new overlay renderer
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        // Load font
        let font_data = std::fs::read("assets/fonts/DejaVuSans.ttf")
            .context("Failed to load font file")?;
        let font = VectorFont::from_ttf(&font_data)
            .map_err(|e| anyhow::anyhow!("Failed to parse font: {}", e))?;

        let text_renderer = VectorTextRenderer::new(device, queue, surface_format, font, 8)?;

        Ok(Self {
            text_renderer,
            stats_instances: Vec::new(),
            keybindings_instances: Vec::new(),
            width,
            height,
        })
    }

    /// Update the overlay content
    pub fn update(
        &mut self,
        queue: &wgpu::Queue,
        stats: &str,
        keybindings: Option<&[(&str, &str)]>,
        width: u32,
        height: u32,
    ) {
        self.width = width;
        self.height = height;
        self.text_renderer.update_screen_size(queue, width as f32, height as f32);

        // Layout stats text (top-left)
        let font_size = 14.0;
        let line_height = font_size * 1.4;
        let margin = 10.0;
        let text_color = [1.0, 1.0, 1.0, 0.9];

        if !stats.is_empty() {
            self.stats_instances = self.text_renderer.layout_text_block(
                stats,
                margin,
                margin,
                font_size,
                line_height,
                text_color,
            );
        } else {
            self.stats_instances.clear();
        }

        // Layout keybindings (top-right corner)
        if let Some(bindings) = keybindings {
            self.keybindings_instances.clear();

            // Format keybindings as multi-line text
            let keybindings_text: String = bindings
                .iter()
                .map(|(key, desc)| format!("{}: {}", key, desc))
                .collect::<Vec<_>>()
                .join("\n");

            // Estimate width for right-alignment (rough estimate)
            let max_line_width = bindings
                .iter()
                .map(|(k, d)| format!("{}: {}", k, d).len())
                .max()
                .unwrap_or(0) as f32 * font_size * 0.5;

            let x = width as f32 - max_line_width - margin;
            let y = margin;

            self.keybindings_instances = self.text_renderer.layout_text_block(
                &keybindings_text,
                x,
                y,
                font_size,
                line_height,
                text_color,
            );
        } else {
            self.keybindings_instances.clear();
        }
    }

    /// Render the overlay
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue) {
        // Combine all instances
        let mut all_instances = Vec::with_capacity(
            self.stats_instances.len() + self.keybindings_instances.len()
        );
        all_instances.extend_from_slice(&self.stats_instances);
        all_instances.extend_from_slice(&self.keybindings_instances);

        if !all_instances.is_empty() {
            self.text_renderer.render(render_pass, queue, &all_instances);
        }
    }

    /// Handle resize
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
}
