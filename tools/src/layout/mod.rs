use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutData {
    pub metadata: Metadata,
    pub elements: Vec<Element>,
    pub summary: Summary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub url: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "userAgent")]
    pub user_agent: Option<String>,
    pub timestamp: String,
    pub viewport: Viewport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "centeringNote")]
    pub centering_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    #[serde(rename = "devicePixelRatio")]
    pub device_pixel_ratio: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    pub index: usize,
    pub tag: String,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,

    // Typography
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "fontSize")]
    pub font_size: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "fontFamily")]
    pub font_family: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "fontWeight")]
    pub font_weight: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "fontStyle")]
    pub font_style: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "lineHeight")]
    pub line_height: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "letterSpacing")]
    pub letter_spacing: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "textAlign")]
    pub text_align: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "textDecoration")]
    pub text_decoration: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "textTransform")]
    pub text_transform: Option<String>,

    // Box model
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "paddingTop")]
    pub padding_top: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "paddingRight")]
    pub padding_right: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "paddingBottom")]
    pub padding_bottom: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "paddingLeft")]
    pub padding_left: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "marginTop")]
    pub margin_top: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "marginRight")]
    pub margin_right: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "marginBottom")]
    pub margin_bottom: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "marginLeft")]
    pub margin_left: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub border: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "borderRadius")]
    pub border_radius: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "borderColor")]
    pub border_color: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "borderTop")]
    pub border_top: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "borderBottom")]
    pub border_bottom: Option<String>,

    // Layout
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "flexDirection")]
    pub flex_direction: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "justifyContent")]
    pub justify_content: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "alignItems")]
    pub align_items: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gap: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "maxWidth")]
    pub max_width: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "minWidth")]
    pub min_width: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "maxHeight")]
    pub max_height: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "minHeight")]
    pub min_height: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "zIndex")]
    pub z_index: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub top: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub bottom: Option<String>,

    // Colors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "backgroundColor")]
    pub background_color: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "boxShadow")]
    pub box_shadow: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "textShadow")]
    pub text_shadow: Option<String>,

    // Content
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "textContent")]
    pub text_content: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,

    // Visibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "whiteSpace")]
    pub white_space: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "wordBreak")]
    pub word_break: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "listStyle")]
    pub list_style: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    #[serde(rename = "totalElements")]
    pub total_elements: usize,
    #[serde(rename = "byTag")]
    pub by_tag: HashMap<String, usize>,
    #[serde(rename = "byClass")]
    pub by_class: HashMap<String, usize>,
}

impl LayoutData {
    #[allow(dead_code)]
    pub fn find_element_by_index(&self, index: usize) -> Option<&Element> {
        self.elements.iter().find(|e| e.index == index)
    }

    #[allow(dead_code)]
    pub fn find_elements_by_tag(&self, tag: &str) -> Vec<&Element> {
        self.elements.iter().filter(|e| e.tag == tag).collect()
    }

    #[allow(dead_code)]
    pub fn find_elements_by_class(&self, class: &str) -> Vec<&Element> {
        self.elements
            .iter()
            .filter(|e| e.classes.contains(&class.to_string()))
            .collect()
    }

    #[allow(dead_code)]
    pub fn find_element_by_id(&self, id: &str) -> Option<&Element> {
        self.elements.iter().find(|e| e.id.as_deref() == Some(id))
    }
}
