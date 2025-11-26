use serde::{Deserialize, Serialize};

/// Precise layout capture schema optimized for renderer diffing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutCapture {
    pub metadata: Metadata,
    pub nodes: Vec<Node>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub url: String,
    pub viewport: Viewport,
    pub captured_at: String,
    pub chrome: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Viewport {
    pub w: u32,
    pub h: u32,
    pub dpr: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_index: Option<usize>,
    pub backend_node_id: Option<u64>,
    pub node_type: String,
    pub tag: Option<String>,
    pub classes: Vec<String>,
    pub pseudo: Option<String>,
    pub text: Option<String>,
    pub box_model: Option<Rect>,
    pub client_rects: Vec<Rect>,
    pub inline_text_boxes: Vec<Rect>,
    pub styles: serde_json::Map<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_metrics: Option<FontMetrics>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn from_tuple(r: (f64, f64, f64, f64)) -> Self {
        Rect {
            x: r.0,
            y: r.1,
            w: r.2,
            h: r.3,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FontMetrics {
    pub ascent: f32,
    pub descent: f32,
}
