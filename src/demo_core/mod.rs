//! Platform-agnostic demo core abstractions
//!
//! This module provides the core demo trait and types that work on both
//! native (windowed) and web platforms.

mod camera_config;
mod context;
mod camera_impls;

pub use camera_config::CameraConfig;
pub use context::DemoContext;

use std::any::Any;

/// Demo identifier (0-8)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DemoId {
    Empty = 0,
    Objects = 1,
    Spheres = 2,
    Towers = 3,
    Text2D = 4,
    Clay = 5,
    TextShadow = 6,
    TodoMvc = 7,
    TodoMvc3D = 8,
}

impl DemoId {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Empty),
            1 => Some(Self::Objects),
            2 => Some(Self::Spheres),
            3 => Some(Self::Towers),
            4 => Some(Self::Text2D),
            5 => Some(Self::Clay),
            6 => Some(Self::TextShadow),
            7 => Some(Self::TodoMvc),
            8 => Some(Self::TodoMvc3D),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Empty => "Empty",
            Self::Objects => "Objects",
            Self::Spheres => "Spheres",
            Self::Towers => "Towers",
            Self::Text2D => "2D Text",
            Self::Clay => "Clay Tablet",
            Self::TextShadow => "Text Shadow",
            Self::TodoMvc => "TodoMVC",
            Self::TodoMvc3D => "TodoMVC 3D",
        }
    }

    pub fn count() -> u8 {
        9
    }

    /// Get all demo IDs in order
    pub fn all() -> &'static [DemoId] {
        &[
            DemoId::Empty,
            DemoId::Objects,
            DemoId::Spheres,
            DemoId::Towers,
            DemoId::Text2D,
            DemoId::Clay,
            DemoId::TextShadow,
            DemoId::TodoMvc,
            DemoId::TodoMvc3D,
        ]
    }
}

/// Demo type enumeration for 2D vs 3D demos
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DemoType {
    /// 3D demo with FlyCamera (WASD + mouse look)
    Scene3D,
    /// 2D demo with pan/zoom/rotate controls
    Scene2D,
}

/// Overlay display mode
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OverlayMode {
    #[default]
    Off,
    App,  // App-only stats (F key)
    Full, // App + system stats (G key)
}

/// Trait for individual demo implementations
///
/// This is the core abstraction that all demos must implement.
/// It provides a common interface for initialization, update, rendering,
/// and cleanup that works across both native and web platforms.
pub trait Demo: Send {
    /// Get the demo name
    fn name(&self) -> &'static str;

    /// Get the demo ID
    fn id(&self) -> DemoId;

    /// Get the demo type (2D or 3D)
    fn demo_type(&self) -> DemoType;

    /// Get keybindings specific to this demo
    fn keybindings(&self) -> &[(&'static str, &'static str)];

    /// Get camera configuration for this demo
    fn camera_config(&self) -> CameraConfig;

    /// Update demo state (called each frame)
    /// dt is delta time in seconds
    fn update(&mut self, dt: f32, camera: &mut dyn CameraController);

    /// Render the demo scene
    /// The render pass is already begun with LoadOp::Clear
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue, time: f32);

    /// Handle window resize
    fn resize(&mut self, width: u32, height: u32);

    /// Get as Any for downcasting
    fn as_any(&self) -> &dyn Any;

    /// Get as Any mut for downcasting
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Optional: Get shader name for hot-reload support
    fn shader_name(&self) -> Option<&'static str> {
        None
    }

    /// Optional: Called when shader is hot-reloaded
    fn on_shader_reload(&mut self, _pipeline: wgpu::RenderPipeline) {}
}

/// Camera controller trait for abstracting FlyCamera
///
/// This allows demos to work with different camera implementations
/// (native FlyCamera, web simple camera, etc.)
pub trait CameraController {
    fn position(&self) -> glam::Vec3;
    fn set_position(&mut self, pos: glam::Vec3);
    fn get_yaw(&self) -> f32;
    fn get_pitch(&self) -> f32;
    fn get_roll(&self) -> f32;
    fn forward(&self) -> glam::Vec3;
    fn right(&self) -> glam::Vec3;
    fn up(&self) -> glam::Vec3;
    fn view_matrix(&self) -> glam::Mat4;
    fn inv_view_projection_matrix(&self, aspect_ratio: f32) -> glam::Mat4;
}

/// Standard 3D keybindings
pub const KEYBINDINGS_3D: &[(&str, &str)] = &[
    ("WASD", "Move"),
    ("Mouse", "Look"),
    ("Space/Ctrl", "Up/Down"),
    ("Q/E", "Roll"),
    ("Scroll", "Speed"),
    ("R", "Reset roll"),
    ("T", "Reset camera"),
    ("Tab", "Capture mouse"),
];

/// Standard 2D keybindings
pub const KEYBINDINGS_2D: &[(&str, &str)] = &[
    ("WASD", "Pan"),
    ("Arrows", "Zoom"),
    ("Q/E", "Rotate"),
    ("R", "Reset rotation"),
    ("T", "Reset all"),
];

/// Common keybindings shown for all demos
pub const KEYBINDINGS_COMMON: &[(&str, &str)] = &[
    ("0-8", "Switch demo"),
    ("F", "Toggle stats"),
    ("G", "Full stats"),
    ("K", "Keybindings"),
    ("Esc", "Exit"),
];
