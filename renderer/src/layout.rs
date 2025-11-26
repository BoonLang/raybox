use serde::{Deserialize, Serialize};

/// Root layout data structure loaded from JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutData {
    pub metadata: Metadata,
    pub elements: Vec<Element>,
}

/// Metadata about how the layout was captured
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub url: String,
    pub viewport: Viewport,
    #[serde(rename = "centeringNote")]
    pub centering_note: Option<String>,
}

/// Viewport dimensions and pixel ratio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    #[serde(rename = "devicePixelRatio")]
    pub device_pixel_ratio: f32,
}

/// A single DOM element with its computed layout and styles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    // Core fields
    pub index: usize,
    pub tag: String,
    pub classes: Vec<String>,
    pub id: Option<String>,
    #[serde(rename = "fontMetrics")]
    pub font_metrics: Option<FontMetrics>,

    // Position and dimensions
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,

    // Text content
    #[serde(rename = "textContent")]
    pub text: Option<String>,

    // Font properties (all optional)
    #[serde(rename = "fontSize")]
    pub font_size: Option<String>,
    #[serde(rename = "fontFamily")]
    pub font_family: Option<String>,
    #[serde(rename = "fontWeight")]
    pub font_weight: Option<String>,
    #[serde(rename = "fontStyle")]
    pub font_style: Option<String>,
    #[serde(rename = "lineHeight")]
    pub line_height: Option<String>,
    #[serde(rename = "textAlign")]
    pub text_align: Option<String>,
    #[serde(rename = "textDecoration")]
    pub text_decoration: Option<String>,

    // Display and positioning
    pub display: Option<String>,
    pub position: Option<String>,

    // Colors
    #[serde(rename = "backgroundColor")]
    pub background_color: Option<String>,
    pub color: Option<String>,

    // Padding (full shorthand and specific sides we rely on for text positioning)
    pub padding: Option<String>,
    #[serde(rename = "paddingLeft")]
    pub padding_left: Option<String>,
    #[serde(rename = "paddingRight")]
    pub padding_right: Option<String>,
    #[serde(rename = "paddingTop")]
    pub padding_top: Option<String>,
    #[serde(rename = "paddingBottom")]
    pub padding_bottom: Option<String>,

    // Borders
    #[serde(rename = "borderWidth")]
    pub border_width: Option<String>,
    #[serde(rename = "borderColor")]
    pub border_color: Option<String>,
    #[serde(rename = "borderStyle")]
    pub border_style: Option<String>,
    #[serde(rename = "borderRadius")]
    pub border_radius: Option<String>,
    #[serde(rename = "borderBottom")]
    pub border_bottom: Option<String>,
    pub border: Option<String>,

    // Other styles
    #[serde(rename = "boxShadow")]
    pub box_shadow: Option<String>,
    pub opacity: Option<String>,
    pub visibility: Option<String>,
    #[serde(rename = "zIndex")]
    pub z_index: Option<String>,
    pub margin: Option<String>,
    #[serde(rename = "maxWidth")]
    pub max_width: Option<String>,

    // Input element properties
    pub placeholder: Option<String>,
    pub checked: Option<bool>,
}

/// Element position and dimensions (helper struct)
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Optional font metrics captured from the DOM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontMetrics {
    pub ascent: f32,
    pub descent: f32,
}

impl LayoutData {
    /// Load layout data from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Get element by index
    pub fn get_element(&self, index: usize) -> Option<&Element> {
        self.elements.iter().find(|e| e.index == index)
    }

    /// Get all elements with a specific tag
    pub fn elements_by_tag(&self, tag: &str) -> Vec<&Element> {
        self.elements.iter().filter(|e| e.tag == tag).collect()
    }

    /// Get all elements with a specific class
    pub fn elements_by_class(&self, class: &str) -> Vec<&Element> {
        self.elements
            .iter()
            .filter(|e| e.classes.contains(&class.to_string()))
            .collect()
    }
}

impl Element {
    /// Check if element has a specific class
    pub fn has_class(&self, class: &str) -> bool {
        self.classes.contains(&class.to_string())
    }

    /// Check if element is visible
    pub fn is_visible(&self) -> bool {
        let is_visible = self.visibility.as_deref() != Some("hidden");
        let has_opacity = self.opacity.as_deref() != Some("0");
        let has_dimensions = self.width > 0.0 && self.height > 0.0;
        is_visible && has_opacity && has_dimensions
    }

    /// Get rectangle as a Rect struct
    pub fn rect(&self) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }

    /// Parse border width from CSS string (e.g., "1px" -> 1.0)
    pub fn get_border_width(&self) -> Option<f32> {
        let border_str = self.border_width.as_deref()?;
        if border_str == "0px" || border_str == "0" {
            return None;
        }
        if border_str.ends_with("px") {
            border_str[..border_str.len() - 2].parse::<f32>().ok()
        } else {
            border_str.parse::<f32>().ok()
        }
    }

    /// Check if element has a visible border
    pub fn has_border(&self) -> bool {
        // Check standard border properties
        if let Some(width) = self.get_border_width() {
            if width > 0.0 {
                if let Some(border_color) = &self.border_color {
                    if parse_color(border_color).is_some() {
                        return true;
                    }
                }
            }
        }

        // Check border shorthand
        if self.border.is_some() {
            return true;
        }

        // Check borderBottom shorthand
        if self.border_bottom.is_some() {
            return true;
        }

        false
    }

    /// Parse borderBottom shorthand (e.g., "1px solid #ededed" -> (width, color))
    pub fn parse_border_bottom(&self) -> Option<(f32, String)> {
        let border_str = self.border_bottom.as_ref()?;
        let parts: Vec<&str> = border_str.split_whitespace().collect();

        if parts.len() >= 3 {
            // Format: "1px solid #ededed"
            let width_str = parts[0];
            let color_str = parts[2];

            let width = if width_str.ends_with("px") {
                width_str[..width_str.len() - 2].parse::<f32>().ok()?
            } else {
                width_str.parse::<f32>().ok()?
            };

            Some((width, color_str.to_string()))
        } else {
            None
        }
    }

    /// Parse border shorthand (e.g., "1px solid #ce4646" -> (width, color))
    pub fn parse_border(&self) -> Option<(f32, String)> {
        let border_str = self.border.as_ref()?;
        let parts: Vec<&str> = border_str.split_whitespace().collect();

        if parts.len() >= 3 {
            // Format: "1px solid #ce4646"
            let width_str = parts[0];
            let color_str = parts[2];

            let width = if width_str.ends_with("px") {
                width_str[..width_str.len() - 2].parse::<f32>().ok()?
            } else {
                width_str.parse::<f32>().ok()?
            };

            Some((width, color_str.to_string()))
        } else {
            None
        }
    }

    /// Check if element is header or footer (should be filtered out)
    pub fn is_header_or_footer(&self) -> bool {
        // Filter out header with class "header" and footer with classes "footer" or "info"
        self.has_class("header") || self.has_class("footer") || self.has_class("info")
    }
}

/// Parse RGB color string to (r, g, b, a) normalized floats
pub fn parse_color(color_str: &str) -> Option<(f32, f32, f32, f32)> {
    // Handle rgb() and rgba() formats
    if color_str.starts_with("rgb(") || color_str.starts_with("rgba(") {
        let start = color_str.find('(')? + 1;
        let end = color_str.find(')')?;
        let inner = &color_str[start..end];
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();

        if parts.len() >= 3 {
            let r = parts[0].parse::<f32>().ok()? / 255.0;
            let g = parts[1].parse::<f32>().ok()? / 255.0;
            let b = parts[2].parse::<f32>().ok()? / 255.0;
            let a = if parts.len() > 3 {
                parts[3].parse::<f32>().ok()?
            } else {
                1.0
            };
            return Some((r, g, b, a));
        }
    }

    // Handle hex colors (#RGB, #RRGGBB, #RRGGBBAA)
    if color_str.starts_with('#') {
        let hex = &color_str[1..];

        match hex.len() {
            3 => {
                // #RGB -> #RRGGBB
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()? as f32 / 255.0;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()? as f32 / 255.0;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()? as f32 / 255.0;
                return Some((r, g, b, 1.0));
            }
            6 => {
                // #RRGGBB
                let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
                return Some((r, g, b, 1.0));
            }
            8 => {
                // #RRGGBBAA
                let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()? as f32 / 255.0;
                return Some((r, g, b, a));
            }
            _ => {}
        }
    }

    None
}

/// A single shadow layer parsed from CSS box-shadow
#[derive(Debug, Clone)]
pub struct Shadow {
    pub inset: bool,
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_radius: f32,
    pub spread_radius: f32,
    pub color: (f32, f32, f32, f32), // RGBA normalized
}

/// Parse box-shadow CSS property
/// Format: [inset] offset-x offset-y [blur-radius] [spread-radius] color
/// Multiple shadows separated by commas
pub fn parse_box_shadow(box_shadow_str: &str) -> Vec<Shadow> {
    let mut shadows = Vec::new();

    // Split by commas (multiple shadow layers)
    for layer in box_shadow_str.split(',') {
        let layer = layer.trim();
        if layer.is_empty() {
            continue;
        }

        // Check for inset keyword
        let inset = layer.starts_with("inset");
        let layer = if inset {
            layer.strip_prefix("inset").unwrap().trim()
        } else {
            layer
        };

        // Split into tokens
        let tokens: Vec<&str> = layer.split_whitespace().collect();
        if tokens.len() < 3 {
            continue; // Need at least offset-x, offset-y, color
        }

        // Parse numeric values (offset-x, offset-y, blur, spread)
        let mut values = Vec::new();
        let mut color_start = 0;

        for (i, token) in tokens.iter().enumerate() {
            // Try to parse as number with optional unit
            if let Some(num) = parse_css_length(token) {
                values.push(num);
            } else {
                // Not a number, must be color
                color_start = i;
                break;
            }
        }

        // Need at least 2 values (offset-x, offset-y)
        if values.len() < 2 {
            continue;
        }

        let offset_x = values[0];
        let offset_y = values[1];
        let blur_radius = values.get(2).copied().unwrap_or(0.0);
        let spread_radius = values.get(3).copied().unwrap_or(0.0);

        // Parse color (rest of tokens from color_start)
        let color_str = tokens[color_start..].join(" ");
        let color = parse_color(&color_str).unwrap_or((0.0, 0.0, 0.0, 0.2));

        shadows.push(Shadow {
            inset,
            offset_x,
            offset_y,
            blur_radius,
            spread_radius,
            color,
        });
    }

    shadows
}

/// Parse CSS length value (e.g., "10px", "0", "-2px")
fn parse_css_length(s: &str) -> Option<f32> {
    if s.ends_with("px") {
        s.strip_suffix("px")?.parse::<f32>().ok()
    } else {
        s.parse::<f32>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_color_rgb() {
        assert_eq!(
            parse_color("rgb(255, 128, 0)"),
            Some((1.0, 128.0 / 255.0, 0.0, 1.0))
        );
    }

    #[test]
    fn test_parse_color_rgba() {
        assert_eq!(
            parse_color("rgba(255, 128, 0, 0.5)"),
            Some((1.0, 128.0 / 255.0, 0.0, 0.5))
        );
    }

    #[test]
    fn test_parse_box_shadow_simple() {
        let shadows = parse_box_shadow("0 2px 4px rgba(0,0,0,0.2)");
        assert_eq!(shadows.len(), 1);
        assert_eq!(shadows[0].offset_x, 0.0);
        assert_eq!(shadows[0].offset_y, 2.0);
        assert_eq!(shadows[0].blur_radius, 4.0);
        assert_eq!(shadows[0].spread_radius, 0.0);
        assert!(!shadows[0].inset);
    }

    #[test]
    fn test_parse_box_shadow_inset() {
        let shadows = parse_box_shadow("inset 0 -2px 1px rgba(0,0,0,0.03)");
        assert_eq!(shadows.len(), 1);
        assert!(shadows[0].inset);
        assert_eq!(shadows[0].offset_x, 0.0);
        assert_eq!(shadows[0].offset_y, -2.0);
        assert_eq!(shadows[0].blur_radius, 1.0);
    }

    #[test]
    fn test_parse_box_shadow_multiple() {
        let shadows =
            parse_box_shadow("0 2px 4px 0 rgba(0,0,0,0.2), 0 25px 50px 0 rgba(0,0,0,0.1)");
        assert_eq!(shadows.len(), 2);
        assert_eq!(shadows[0].offset_y, 2.0);
        assert_eq!(shadows[0].blur_radius, 4.0);
        assert_eq!(shadows[0].spread_radius, 0.0);
        assert_eq!(shadows[1].offset_y, 25.0);
        assert_eq!(shadows[1].blur_radius, 50.0);
    }
}
