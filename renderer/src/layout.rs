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

    // Position and dimensions
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,

    // Text content
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

    // Display and positioning
    pub display: Option<String>,
    pub position: Option<String>,

    // Colors
    #[serde(rename = "backgroundColor")]
    pub background_color: Option<String>,
    pub color: Option<String>,

    // Borders
    #[serde(rename = "borderWidth")]
    pub border_width: Option<String>,
    #[serde(rename = "borderColor")]
    pub border_color: Option<String>,
    #[serde(rename = "borderStyle")]
    pub border_style: Option<String>,
    #[serde(rename = "borderRadius")]
    pub border_radius: Option<String>,

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
}

/// Element position and dimensions (helper struct)
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
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

    None
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
}
