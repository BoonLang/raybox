//! Unified demo system for raybox
//!
//! Provides a trait-based abstraction for different rendering demos,
//! enabling runtime switching between demos (0-6) with a unified runner.

pub mod clay;
pub mod empty;
pub mod gpu_runtime_common;
pub mod objects;
pub mod retained_ui;
pub mod retained_ui_physical;
pub mod runner;
pub mod spheres;
pub mod text2d;
pub mod text_physical;
pub mod text_shadow;
pub mod todomvc;
pub mod todomvc_3d;
pub mod todomvc_common;
pub mod towers;
pub mod ui2d_runtime;
pub mod ui_physical_runtime;
pub mod ui_physical_theme;
pub mod world3d_runtime;

// Re-export from demo_core for backward compatibility
pub use crate::demo_core::{
    CameraConfig, CameraController, DemoContext, DemoId, DemoType, ListCommandTarget, ListFilter,
    NamedScrollTarget, UiPhysicalCameraPreset, KEYBINDINGS_2D, KEYBINDINGS_3D, KEYBINDINGS_COMMON,
};

use crate::camera::FlyCamera;
use winit::keyboard::KeyCode;

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

    /// Optional bounded inspect-camera preset for UiPhysical scenes.
    fn ui_physical_camera_preset(&self) -> Option<UiPhysicalCameraPreset> {
        None
    }

    /// Update demo state (called each frame)
    /// dt is delta time in seconds
    fn update(&mut self, dt: f32, camera: &mut FlyCamera);

    /// Render the demo scene
    /// The render pass is already begun with LoadOp::Clear
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue, time: f32);

    /// Optional per-frame preparation before render encoding.
    fn prepare_frame(&mut self, _queue: &wgpu::Queue) {}

    /// Optional shared 2D view controls for retained-style demos.
    fn apply_2d_view_controls(
        &mut self,
        _offset_delta: [f32; 2],
        _scale_factor: f32,
        _rotation_delta: f32,
    ) {
    }

    /// Optional shared 2D reset-rotation hook.
    fn reset_2d_rotation(&mut self) {}

    /// Optional shared 2D reset-all hook.
    fn reset_2d_all(&mut self) {}

    /// Optional retained list item mutation hook.
    fn toggle_list_item(&mut self, _index: usize) -> bool {
        self.list_command_target()
            .is_some_and(|target| target.toggle_item(_index))
    }

    /// Optional retained list completion mutation hook.
    fn set_list_item_completed(&mut self, _index: usize, _completed: bool) -> bool {
        self.list_command_target()
            .is_some_and(|target| target.set_item_completed(_index, _completed))
    }

    /// Optional retained list label mutation hook.
    fn set_list_item_label(&mut self, _index: usize, _label: &str) -> bool {
        self.list_command_target()
            .is_some_and(|target| target.set_item_label(_index, _label))
    }

    /// Optional retained list filter mutation hook.
    fn set_list_filter(&mut self, _filter: ListFilter) -> bool {
        self.list_command_target()
            .is_some_and(|target| target.set_filter(_filter))
    }

    /// Optional retained list scroll mutation hook.
    fn set_list_scroll_offset(&mut self, _offset_y: f32) {
        if let Some(target) = self.list_command_target() {
            target.set_scroll_offset(_offset_y);
        }
    }

    /// Optional shared retained list command target.
    fn list_command_target(&mut self) -> Option<&mut dyn ListCommandTarget> {
        None
    }

    /// Whether this demo can expose a shared retained list command target.
    fn has_list_command_target(&self) -> bool {
        false
    }

    /// Optional generic named scroll mutation hook.
    fn set_named_scroll_offset(&mut self, name: &str, offset_y: f32) -> bool {
        self.named_scroll_target()
            .is_some_and(|target| target.set_named_scroll_offset(name, offset_y))
    }

    /// Optional shared retained named scroll target.
    fn named_scroll_target(&mut self) -> Option<&mut dyn NamedScrollTarget> {
        None
    }

    /// Whether this demo can expose a generic named scroll target.
    fn has_named_scroll_target(&self) -> bool {
        false
    }

    /// Optional camera/uniform sync hook for 3D demos that need it.
    fn update_camera_uniforms(&self, _queue: &wgpu::Queue, _camera: &FlyCamera, _time: f32) {}

    /// Optional window-scale-factor hook.
    fn set_window_scale_factor(&mut self, _scale_factor: f32) {}

    /// Optional named theme hook.
    fn set_named_theme(
        &mut self,
        _theme: &str,
        _dark_mode: Option<bool>,
    ) -> Option<(&'static str, bool)> {
        None
    }

    /// Optional list of named themes accepted by this demo.
    fn named_theme_options(&self) -> &'static [&'static str] {
        &[]
    }

    /// Whether this demo needs continuous redraw even without input.
    fn wants_continuous_redraw(&self) -> bool {
        false
    }

    /// Whether this demo has pending retained/runtime work that requires another frame.
    fn needs_redraw(&self) -> bool {
        false
    }

    /// Optional per-key hook for demo-specific actions.
    fn handle_key_pressed(&mut self, _code: KeyCode) -> bool {
        false
    }

    /// Handle window resize
    fn resize(&mut self, width: u32, height: u32);

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
        DemoId::TodoMvc3D => Ok(Box::new(todomvc_3d::TodoMvc3DDemo::new(ctx)?)),
        DemoId::RetainedUi => Ok(Box::new(retained_ui::RetainedUiDemo::new(ctx)?)),
        DemoId::RetainedUiPhysical => Ok(Box::new(
            retained_ui_physical::RetainedUiPhysicalDemo::new(ctx)?,
        )),
        DemoId::TextPhysical => Ok(Box::new(text_physical::TextPhysicalDemo::new(ctx)?)),
    }
}
