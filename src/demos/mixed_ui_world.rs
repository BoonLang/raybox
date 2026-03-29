use super::{
    todomvc_common::{
        create_todomvc_themed_ui_physical_host, create_todomvc_ui2d_deck,
        create_todomvc_ui2d_transparent_deck, TodoMvcRetainedState,
    },
    ui2d_runtime::StateBackedUi2dSceneDeck,
    ui_physical_runtime::ThemedUiPhysicalHost,
    ui_physical_theme::ThemeId,
    ComposedScene, Demo, DemoContext, DemoId, DemoType, FramePacket, ListCommandTarget,
    NamedScrollTarget, UiLayer, UiLayerPacket, UiLayerStyle, WorldView, WorldViewPacket,
};
use crate::camera::FlyCamera;
use crate::input::CameraConfig;
use anyhow::Result;
use glam::Vec3;
use winit::keyboard::KeyCode;

use super::objects::ObjectsDemo;

const KEYBINDINGS_MIXED_UI_WORLD: &[(&str, &str)] = &[
    ("WASD", "Move world camera"),
    ("Mouse", "Look world camera"),
    ("Space/Ctrl", "Up/Down"),
    ("Scroll", "Speed"),
    ("R", "Reset roll"),
    ("T", "Reset camera"),
    ("Tab", "Capture mouse"),
    ("B", "Toggle embedded preview mode"),
    ("P", "Toggle flat/physical UI overlay"),
    ("N", "Cycle physical overlay theme"),
    ("M", "Toggle physical overlay dark mode"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MixedUiWorldMode {
    WorldWithUiOverlay,
    UiWithEmbeddedWorld,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OverlayUiMode {
    Flat,
    Physical,
}

pub struct MixedUiWorldDemo {
    world: ObjectsDemo,
    overlay_ui: StateBackedUi2dSceneDeck<TodoMvcRetainedState>,
    physical_overlay: ThemedUiPhysicalHost<super::todomvc_common::TodoMvcUiPhysicalDeck>,
    embedded_ui: StateBackedUi2dSceneDeck<TodoMvcRetainedState>,
    mode: MixedUiWorldMode,
    overlay_ui_mode: OverlayUiMode,
    overlay_camera: FlyCamera,
}

impl MixedUiWorldDemo {
    const WORLD_TARGET: &'static str = "mixed_world";
    const UI_TARGET: &'static str = "mixed_ui";
    const PHYSICAL_UI_TARGET: &'static str = "mixed_ui_physical";
    const EMBEDDED_UI_TARGET: &'static str = "mixed_ui_embedded";
    const PREVIEW_TARGET: &'static str = "mixed_world_preview";
    const PREVIEW_RECT: super::NormalizedRect =
        super::NormalizedRect::new([0.66, 0.12], [0.24, 0.24]);

    pub fn new(ctx: &DemoContext) -> Result<Self> {
        let current_theme = ThemeId::Classic2D;
        let dark_mode = false;
        let physical_overlay = create_todomvc_themed_ui_physical_host(
            ctx,
            "Mixed UI Physical Overlay",
            current_theme,
            dark_mode,
            "Mixed UI Physical Primitives Buffer",
            super::ui_physical_runtime::UiPhysicalBackgroundMode::Transparent,
        )?;
        let mut overlay_camera = FlyCamera::new();
        overlay_camera.set_position(Vec3::new(0.0, 0.0, 8.5));
        overlay_camera.look_at(Vec3::ZERO);

        Ok(Self {
            world: ObjectsDemo::new(ctx)?,
            overlay_ui: create_todomvc_ui2d_transparent_deck(ctx)?,
            physical_overlay,
            embedded_ui: create_todomvc_ui2d_deck(ctx)?,
            mode: MixedUiWorldMode::WorldWithUiOverlay,
            overlay_ui_mode: OverlayUiMode::Flat,
            overlay_camera,
        })
    }

    fn active_overlay_style(&self) -> UiLayerStyle {
        match self.overlay_ui_mode {
            OverlayUiMode::Flat => UiLayerStyle::Flat,
            OverlayUiMode::Physical => UiLayerStyle::Physical,
        }
    }

    fn active_overlay_target(&self) -> &'static str {
        match self.overlay_ui_mode {
            OverlayUiMode::Flat => Self::UI_TARGET,
            OverlayUiMode::Physical => Self::PHYSICAL_UI_TARGET,
        }
    }

    fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            MixedUiWorldMode::WorldWithUiOverlay => MixedUiWorldMode::UiWithEmbeddedWorld,
            MixedUiWorldMode::UiWithEmbeddedWorld => MixedUiWorldMode::WorldWithUiOverlay,
        };
    }

    fn toggle_overlay_ui_mode(&mut self) {
        self.overlay_ui_mode = match self.overlay_ui_mode {
            OverlayUiMode::Flat => OverlayUiMode::Physical,
            OverlayUiMode::Physical => OverlayUiMode::Flat,
        };
    }
}

impl Demo for MixedUiWorldDemo {
    fn name(&self) -> &'static str {
        "Mixed UI World"
    }

    fn id(&self) -> DemoId {
        DemoId::MixedUiWorld
    }

    fn demo_type(&self) -> DemoType {
        DemoType::World3D
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        KEYBINDINGS_MIXED_UI_WORLD
    }

    fn camera_config(&self) -> CameraConfig {
        self.world.camera_config()
    }

    fn update(&mut self, dt: f32, camera: &mut FlyCamera) {
        self.world.update(dt, camera);
    }

    fn prepare_frame(&mut self, queue: &wgpu::Queue) {
        self.world.prepare_frame(queue);
        self.overlay_ui.prepare_frame();
        self.physical_overlay.prepare_frame(queue);
        self.embedded_ui.prepare_frame();
    }

    fn build_frame_packet(&self, _time: f32) -> FramePacket {
        match self.mode {
            MixedUiWorldMode::WorldWithUiOverlay => ComposedScene::from_world_with_ui_overlay(
                WorldView::new("Mixed World").to_offscreen(Self::WORLD_TARGET),
                UiLayer::new("Mixed UI Overlay", self.active_overlay_style())
                    .to_offscreen(self.active_overlay_target())
                    .transparent(),
            )
            .into_frame_packet(),
            MixedUiWorldMode::UiWithEmbeddedWorld => {
                ComposedScene::from_ui_with_embedded_world_view(
                    UiLayer::new("Mixed UI", UiLayerStyle::Flat)
                        .to_offscreen(Self::EMBEDDED_UI_TARGET)
                        .transparent(),
                    WorldView::new("Embedded World").to_offscreen(Self::PREVIEW_TARGET),
                    Self::PREVIEW_RECT,
                )
                .into_frame_packet()
            }
        }
    }

    fn update_camera_uniforms(&self, queue: &wgpu::Queue, camera: &FlyCamera, time: f32) {
        self.world.update_camera_uniforms(queue, camera, time);
        self.physical_overlay
            .update_uniforms(queue, &self.overlay_camera, time);
    }

    fn needs_redraw(&self) -> bool {
        let active_ui_needs_redraw = match self.mode {
            MixedUiWorldMode::WorldWithUiOverlay => match self.overlay_ui_mode {
                OverlayUiMode::Flat => self.overlay_ui.needs_redraw(),
                OverlayUiMode::Physical => self.physical_overlay.needs_redraw(),
            },
            MixedUiWorldMode::UiWithEmbeddedWorld => self.embedded_ui.needs_redraw(),
        };
        self.world.wants_continuous_redraw() || active_ui_needs_redraw
    }

    fn list_command_target(&mut self) -> Option<&mut dyn ListCommandTarget> {
        match self.mode {
            MixedUiWorldMode::WorldWithUiOverlay => match self.overlay_ui_mode {
                OverlayUiMode::Flat => Some(&mut self.overlay_ui),
                OverlayUiMode::Physical => Some(&mut self.physical_overlay),
            },
            MixedUiWorldMode::UiWithEmbeddedWorld => Some(&mut self.embedded_ui),
        }
    }

    fn has_list_command_target(&self) -> bool {
        true
    }

    fn named_scroll_target(&mut self) -> Option<&mut dyn NamedScrollTarget> {
        match self.mode {
            MixedUiWorldMode::WorldWithUiOverlay => match self.overlay_ui_mode {
                OverlayUiMode::Flat => Some(&mut self.overlay_ui),
                OverlayUiMode::Physical => Some(&mut self.physical_overlay),
            },
            MixedUiWorldMode::UiWithEmbeddedWorld => Some(&mut self.embedded_ui),
        }
    }

    fn has_named_scroll_target(&self) -> bool {
        true
    }

    fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        time: f32,
    ) {
        self.world.render(render_pass, queue, time);
    }

    fn render_world_view<'a>(
        &'a self,
        _packet: &WorldViewPacket,
        render_pass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        time: f32,
    ) {
        self.world.render(render_pass, queue, time);
    }

    fn render_ui_layer<'a>(
        &'a self,
        packet: &UiLayerPacket,
        render_pass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        _time: f32,
    ) {
        match self.mode {
            MixedUiWorldMode::WorldWithUiOverlay => match packet.style {
                UiLayerStyle::Flat => self.overlay_ui.render(render_pass, queue),
                UiLayerStyle::Physical => self.physical_overlay.render(render_pass),
            },
            MixedUiWorldMode::UiWithEmbeddedWorld => self.embedded_ui.render(render_pass, queue),
        }
    }

    fn handle_key_pressed(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::KeyB => {
                self.toggle_mode();
                true
            }
            KeyCode::KeyP => {
                self.toggle_overlay_ui_mode();
                true
            }
            KeyCode::KeyN if self.mode == MixedUiWorldMode::WorldWithUiOverlay
                && self.overlay_ui_mode == OverlayUiMode::Physical =>
            {
                self.physical_overlay.cycle_theme();
                true
            }
            KeyCode::KeyM if self.mode == MixedUiWorldMode::WorldWithUiOverlay
                && self.overlay_ui_mode == OverlayUiMode::Physical =>
            {
                self.physical_overlay.toggle_dark_mode();
                true
            }
            _ => false,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.world.resize(width, height);
        self.overlay_ui.resize(width, height);
        self.physical_overlay.resize(width, height);
        self.embedded_ui.resize(width, height);
    }
}
