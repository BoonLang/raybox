//! Unified demo system for raybox
//!
//! Provides a trait-based abstraction for different rendering demos,
//! enabling runtime switching between demos (0-6) with a unified runner.

pub mod empty;
pub mod objects;
pub mod spheres;
pub mod towers;
pub mod text2d;
pub mod clay;
pub mod text_shadow;
pub mod todomvc;
pub mod runner;

// Re-export from demo_core for backward compatibility
pub use crate::demo_core::{
    CameraConfig, CameraController, DemoContext, DemoId, DemoType,
    KEYBINDINGS_2D, KEYBINDINGS_3D, KEYBINDINGS_COMMON,
};

use crate::camera::FlyCamera;
use std::any::Any;

/// Trait for individual demo implementations (native version with FlyCamera)
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
    fn update(&mut self, dt: f32, camera: &mut FlyCamera);

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

/// Create a demo by ID
pub fn create_demo(id: DemoId, ctx: &DemoContext) -> anyhow::Result<Box<dyn Demo>> {
    match id {
        DemoId::Empty => Ok(Box::new(empty::EmptyDemo::new(ctx)?)),
        DemoId::Objects => Ok(Box::new(objects::ObjectsDemo::new(ctx)?)),
        DemoId::Spheres => Ok(Box::new(spheres::SpheresDemo::new(ctx)?)),
        DemoId::Towers => Ok(Box::new(towers::TowersDemo::new(ctx)?)),
        DemoId::Text2D => Ok(Box::new(text2d::Text2DDemo::new(ctx)?)),
        DemoId::Clay => Ok(Box::new(clay::ClayDemo::new(ctx)?)),
        DemoId::TextShadow => Ok(Box::new(text_shadow::TextShadowDemo::new(ctx)?)),
        DemoId::TodoMvc => Ok(Box::new(todomvc::TodoMvcDemo::new(ctx)?)),
    }
}
