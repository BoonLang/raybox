//! Platform-agnostic demo core abstractions
//!
//! This module provides the core demo trait and types that work on both
//! native (windowed) and web platforms.

mod camera_config;
mod camera_impls;
mod context;

pub use camera_config::{
    ui_physical_card_camera_config, ui_physical_card_camera_preset, CameraConfig,
    UiPhysicalCameraPreset,
};
pub use context::DemoContext;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ListFilter {
    All,
    Active,
    Completed,
}

impl ListFilter {
    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "all" => Some(Self::All),
            "active" => Some(Self::Active),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Active => "active",
            Self::Completed => "completed",
        }
    }
}

pub trait ListCommandTarget {
    fn toggle_item(&mut self, index: usize) -> bool;
    fn set_item_completed(&mut self, index: usize, completed: bool) -> bool;
    fn set_item_label(&mut self, index: usize, label: &str) -> bool;
    fn set_filter(&mut self, filter: ListFilter) -> bool;
    fn set_scroll_offset(&mut self, offset_y: f32);
}

pub trait NamedScrollTarget {
    fn set_named_scroll_offset(&mut self, name: &str, offset_y: f32) -> bool;
}

/// Demo identifier (0-11)
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
    RetainedUi = 9,
    RetainedUiPhysical = 10,
    TextPhysical = 11,
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
            9 => Some(Self::RetainedUi),
            10 => Some(Self::RetainedUiPhysical),
            11 => Some(Self::TextPhysical),
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
            Self::RetainedUi => "Retained UI",
            Self::RetainedUiPhysical => "Retained UI Physical",
            Self::TextPhysical => "Text Physical",
        }
    }

    pub fn count() -> u8 {
        12
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
            DemoId::RetainedUi,
            DemoId::RetainedUiPhysical,
            DemoId::TextPhysical,
        ]
    }
}

/// Internal demo family used by the runner/runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DemoType {
    /// Retained 2D UI with pan/zoom/rotate controls.
    Ui2D,
    /// Physicalized retained UI with inspect-camera controls.
    UiPhysical,
    /// Free-camera world-space 3D scene.
    World3D,
}

impl DemoType {
    pub fn family_name(self) -> &'static str {
        match self {
            Self::Ui2D => "ui2d",
            Self::UiPhysical => "uiPhysical",
            Self::World3D => "world3d",
        }
    }

    pub fn uses_2d_view_controls(self) -> bool {
        matches!(self, Self::Ui2D)
    }

    pub fn uses_camera_controls(self) -> bool {
        matches!(self, Self::UiPhysical | Self::World3D)
    }

    pub fn is_world3d(self) -> bool {
        matches!(self, Self::World3D)
    }
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

    /// Get the internal demo family.
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
    ("0-9/-/=", "Switch demo"),
    ("F", "Toggle stats"),
    ("G", "Full stats"),
    ("K", "Keybindings"),
    ("Esc", "Exit"),
];
