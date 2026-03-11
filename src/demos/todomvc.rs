//! Demo 7: TodoMVC SDF Rendering
//!
//! The retained TodoMVC scene now lives behind the shared state-backed `Ui2D` host.

use super::{
    todomvc_common::{create_todomvc_ui2d_deck, TodoMvcRetainedState},
    ui2d_runtime::StateBackedUi2dSceneDeck,
    Demo, DemoContext, DemoId, DemoType, ListCommandTarget, NamedScrollTarget, KEYBINDINGS_2D,
};
use crate::camera::FlyCamera;
use crate::input::CameraConfig;
use anyhow::Result;

pub struct TodoMvcDemo {
    host: StateBackedUi2dSceneDeck<TodoMvcRetainedState>,
}

impl TodoMvcDemo {
    pub fn new(ctx: &DemoContext) -> Result<Self> {
        Ok(Self {
            host: create_todomvc_ui2d_deck(ctx)?,
        })
    }
}

impl Demo for TodoMvcDemo {
    fn name(&self) -> &'static str {
        "TodoMVC"
    }

    fn id(&self) -> DemoId {
        DemoId::TodoMvc
    }

    fn demo_type(&self) -> DemoType {
        DemoType::Ui2D
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        KEYBINDINGS_2D
    }

    fn camera_config(&self) -> CameraConfig {
        CameraConfig::default()
    }

    fn update(&mut self, _dt: f32, _camera: &mut FlyCamera) {
        // 2D controls are handled by the runner.
    }

    fn prepare_frame(&mut self, queue: &wgpu::Queue) {
        let _ = queue;
        self.host.prepare_frame();
    }

    fn needs_redraw(&self) -> bool {
        self.host.needs_redraw()
    }

    fn apply_2d_view_controls(
        &mut self,
        offset_delta: [f32; 2],
        scale_factor: f32,
        rotation_delta: f32,
    ) {
        self.host
            .apply_view_controls(offset_delta, scale_factor, rotation_delta);
    }

    fn reset_2d_rotation(&mut self) {
        self.host.reset_rotation();
    }

    fn reset_2d_all(&mut self) {
        self.host.reset_all();
    }

    fn list_command_target(&mut self) -> Option<&mut dyn ListCommandTarget> {
        Some(&mut self.host)
    }

    fn has_list_command_target(&self) -> bool {
        true
    }

    fn named_scroll_target(&mut self) -> Option<&mut dyn NamedScrollTarget> {
        Some(&mut self.host)
    }

    fn has_named_scroll_target(&self) -> bool {
        true
    }

    fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        _time: f32,
    ) {
        self.host.render(render_pass, queue);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.host.resize(width, height);
    }
}
