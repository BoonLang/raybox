use super::DemoType;
use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositeMode {
    Replace,
    Over,
}

impl CompositeMode {
    pub fn load_op(self, clear_color: Option<wgpu::Color>) -> wgpu::LoadOp<wgpu::Color> {
        match self {
            Self::Replace => wgpu::LoadOp::Clear(clear_color.unwrap_or(wgpu::Color::BLACK)),
            Self::Over => wgpu::LoadOp::Load,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameTarget {
    SceneColor,
    Offscreen(Cow<'static, str>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalizedRect {
    pub origin: [f32; 2],
    pub size: [f32; 2],
}

impl NormalizedRect {
    pub const FULLSCREEN: Self = Self {
        origin: [0.0, 0.0],
        size: [1.0, 1.0],
    };

    pub const fn new(origin: [f32; 2], size: [f32; 2]) -> Self {
        Self { origin, size }
    }

    pub const fn as_vec4(self) -> [f32; 4] {
        [self.origin[0], self.origin[1], self.size[0], self.size[1]]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiLayerStyle {
    Flat,
    Physical,
}

#[derive(Debug, Clone)]
pub struct WorldViewPacket {
    pub label: Cow<'static, str>,
    pub target: FrameTarget,
    pub composite_mode: CompositeMode,
    pub clear_color: Option<wgpu::Color>,
}

impl WorldViewPacket {
    pub fn new(label: impl Into<Cow<'static, str>>) -> Self {
        Self {
            label: label.into(),
            target: FrameTarget::SceneColor,
            composite_mode: CompositeMode::Replace,
            clear_color: Some(wgpu::Color::BLACK),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorldView {
    packet: WorldViewPacket,
}

impl WorldView {
    pub fn new(label: impl Into<Cow<'static, str>>) -> Self {
        Self {
            packet: WorldViewPacket::new(label),
        }
    }

    pub fn to_target(mut self, target: FrameTarget) -> Self {
        self.packet.target = target;
        self
    }

    pub fn to_offscreen(self, target: impl Into<Cow<'static, str>>) -> Self {
        self.to_target(FrameTarget::Offscreen(target.into()))
    }

    pub fn with_clear_color(mut self, clear_color: Option<wgpu::Color>) -> Self {
        self.packet.clear_color = clear_color;
        self
    }

    pub fn with_composite_mode(mut self, composite_mode: CompositeMode) -> Self {
        self.packet.composite_mode = composite_mode;
        self
    }

    pub fn into_packet(self) -> WorldViewPacket {
        self.packet
    }
}

#[derive(Debug, Clone)]
pub struct UiLayerPacket {
    pub label: Cow<'static, str>,
    pub style: UiLayerStyle,
    pub target: FrameTarget,
    pub composite_mode: CompositeMode,
    pub clear_color: Option<wgpu::Color>,
}

impl UiLayerPacket {
    pub fn new(label: impl Into<Cow<'static, str>>, style: UiLayerStyle) -> Self {
        Self {
            label: label.into(),
            style,
            target: FrameTarget::SceneColor,
            composite_mode: CompositeMode::Replace,
            clear_color: Some(wgpu::Color::BLACK),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UiLayer {
    packet: UiLayerPacket,
}

impl UiLayer {
    pub fn new(label: impl Into<Cow<'static, str>>, style: UiLayerStyle) -> Self {
        Self {
            packet: UiLayerPacket::new(label, style),
        }
    }

    pub fn to_target(mut self, target: FrameTarget) -> Self {
        self.packet.target = target;
        self
    }

    pub fn to_offscreen(self, target: impl Into<Cow<'static, str>>) -> Self {
        self.to_target(FrameTarget::Offscreen(target.into()))
    }

    pub fn transparent(mut self) -> Self {
        self.packet.clear_color = Some(wgpu::Color::TRANSPARENT);
        self
    }

    pub fn with_clear_color(mut self, clear_color: Option<wgpu::Color>) -> Self {
        self.packet.clear_color = clear_color;
        self
    }

    pub fn with_composite_mode(mut self, composite_mode: CompositeMode) -> Self {
        self.packet.composite_mode = composite_mode;
        self
    }

    pub fn into_packet(self) -> UiLayerPacket {
        self.packet
    }
}

#[derive(Debug, Clone)]
pub struct EffectPacket {
    pub label: Cow<'static, str>,
    pub source: FrameTarget,
    pub target: FrameTarget,
    pub composite_mode: CompositeMode,
    pub clear_color: Option<wgpu::Color>,
    pub source_rect: NormalizedRect,
    pub target_rect: NormalizedRect,
}

impl EffectPacket {
    pub fn composite(
        label: impl Into<Cow<'static, str>>,
        source: FrameTarget,
        target: FrameTarget,
    ) -> Self {
        Self {
            label: label.into(),
            source,
            target,
            composite_mode: CompositeMode::Over,
            clear_color: None,
            source_rect: NormalizedRect::FULLSCREEN,
            target_rect: NormalizedRect::FULLSCREEN,
        }
    }

    pub fn with_target_rect(mut self, target_rect: NormalizedRect) -> Self {
        self.target_rect = target_rect;
        self
    }

    pub fn with_source_rect(mut self, source_rect: NormalizedRect) -> Self {
        self.source_rect = source_rect;
        self
    }
}

#[derive(Debug, Clone)]
pub struct EffectLayer {
    packet: EffectPacket,
}

impl EffectLayer {
    pub fn composite(
        label: impl Into<Cow<'static, str>>,
        source: FrameTarget,
        target: FrameTarget,
    ) -> Self {
        Self {
            packet: EffectPacket::composite(label, source, target),
        }
    }

    pub fn with_target_rect(mut self, target_rect: NormalizedRect) -> Self {
        self.packet = self.packet.with_target_rect(target_rect);
        self
    }

    pub fn with_source_rect(mut self, source_rect: NormalizedRect) -> Self {
        self.packet = self.packet.with_source_rect(source_rect);
        self
    }

    pub fn with_composite_mode(mut self, composite_mode: CompositeMode) -> Self {
        self.packet.composite_mode = composite_mode;
        self
    }

    pub fn with_clear_color(mut self, clear_color: Option<wgpu::Color>) -> Self {
        self.packet.clear_color = clear_color;
        self
    }

    pub fn into_packet(self) -> EffectPacket {
        self.packet
    }
}

#[derive(Debug, Clone)]
pub struct OverlayPacket {
    pub label: Cow<'static, str>,
    pub target: FrameTarget,
}

impl OverlayPacket {
    pub fn new(label: impl Into<Cow<'static, str>>) -> Self {
        Self {
            label: label.into(),
            target: FrameTarget::SceneColor,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PresentPacket {
    pub target: FrameTarget,
}

impl PresentPacket {
    pub fn new() -> Self {
        Self {
            target: FrameTarget::SceneColor,
        }
    }
}

#[derive(Debug, Clone)]
pub enum FramePacketItem {
    WorldView(WorldViewPacket),
    UiLayer(UiLayerPacket),
    Effect(EffectPacket),
    Overlay(OverlayPacket),
    Present(PresentPacket),
}

#[derive(Debug, Clone, Default)]
pub struct FramePacket {
    items: Vec<FramePacketItem>,
}

impl FramePacket {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn for_demo(demo_type: DemoType, label: impl Into<Cow<'static, str>>) -> Self {
        let label = label.into();
        let mut packet = Self::new();
        match demo_type {
            DemoType::Ui2D => packet.push_ui_layer(UiLayerPacket::new(label, UiLayerStyle::Flat)),
            DemoType::UiPhysical => {
                packet.push_ui_layer(UiLayerPacket::new(label, UiLayerStyle::Physical))
            }
            DemoType::World3D => packet.push_world_view(WorldViewPacket::new(label)),
        }
        packet
    }

    pub fn items(&self) -> &[FramePacketItem] {
        &self.items
    }

    pub fn push_item(&mut self, item: FramePacketItem) {
        self.items.push(item);
    }

    pub fn push_world_view(&mut self, packet: WorldViewPacket) {
        self.push_item(FramePacketItem::WorldView(packet));
    }

    pub fn push_ui_layer(&mut self, packet: UiLayerPacket) {
        self.push_item(FramePacketItem::UiLayer(packet));
    }

    pub fn push_effect(&mut self, packet: EffectPacket) {
        self.push_item(FramePacketItem::Effect(packet));
    }

    pub fn push_overlay(&mut self, packet: OverlayPacket) {
        self.push_item(FramePacketItem::Overlay(packet));
    }

    pub fn push_present(&mut self, packet: PresentPacket) {
        self.push_item(FramePacketItem::Present(packet));
    }

    pub fn compile(&self) -> CompiledFrameGraph {
        CompiledFrameGraph::from_packet(self)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ComposedScene {
    packet: FramePacket,
}

impl ComposedScene {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_world_with_ui_overlay(world_view: WorldView, ui_layer: UiLayer) -> Self {
        let mut scene = Self::new();
        scene
            .push_fullscreen_world_with_composite(world_view)
            .push_fullscreen_ui_with_composite(ui_layer);
        scene
    }

    pub fn from_ui_with_embedded_world_view(
        ui_layer: UiLayer,
        world_view: WorldView,
        world_rect: NormalizedRect,
    ) -> Self {
        let mut scene = Self::new();
        scene
            .push_fullscreen_ui_with_composite(ui_layer)
            .push_embedded_world_with_composite(world_view, world_rect);
        scene
    }

    pub fn world_with_ui_overlay(
        world_label: impl Into<Cow<'static, str>>,
        world_target: impl Into<Cow<'static, str>>,
        ui_label: impl Into<Cow<'static, str>>,
        ui_style: UiLayerStyle,
        ui_target: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::from_world_with_ui_overlay(
            WorldView::new(world_label).to_offscreen(world_target),
            UiLayer::new(ui_label, ui_style)
                .to_offscreen(ui_target)
                .transparent(),
        )
    }

    pub fn ui_with_embedded_world_view(
        ui_label: impl Into<Cow<'static, str>>,
        ui_style: UiLayerStyle,
        ui_target: impl Into<Cow<'static, str>>,
        world_label: impl Into<Cow<'static, str>>,
        world_target: impl Into<Cow<'static, str>>,
        world_rect: NormalizedRect,
    ) -> Self {
        Self::from_ui_with_embedded_world_view(
            UiLayer::new(ui_label, ui_style)
                .to_offscreen(ui_target)
                .transparent(),
            WorldView::new(world_label).to_offscreen(world_target),
            world_rect,
        )
    }

    pub fn add_fullscreen_world_view(
        &mut self,
        label: impl Into<Cow<'static, str>>,
        offscreen_target: impl Into<Cow<'static, str>>,
    ) -> &mut Self {
        let label = label.into();
        let target = FrameTarget::Offscreen(offscreen_target.into());

        self.push_world(WorldView::new(label.clone()).to_target(target.clone()));
        self.push_effect_layer(
            EffectLayer::composite(
                format!("{label} Composite"),
                target,
                FrameTarget::SceneColor,
            )
            .with_composite_mode(CompositeMode::Replace)
            .with_clear_color(Some(wgpu::Color::BLACK)),
        );
        self
    }

    pub fn push_fullscreen_world_with_composite(&mut self, world_view: WorldView) -> &mut Self {
        let packet = world_view.into_packet();
        let target = packet.target.clone();
        let label = packet.label.clone();
        self.push_world_view(packet);
        self.push_effect(
            EffectLayer::composite(
                format!("{label} Composite"),
                target,
                FrameTarget::SceneColor,
            )
            .with_composite_mode(CompositeMode::Replace)
            .with_clear_color(Some(wgpu::Color::BLACK))
            .into_packet(),
        );
        self
    }

    pub fn add_fullscreen_ui_overlay(
        &mut self,
        label: impl Into<Cow<'static, str>>,
        style: UiLayerStyle,
        offscreen_target: impl Into<Cow<'static, str>>,
    ) -> &mut Self {
        let label = label.into();
        let target = FrameTarget::Offscreen(offscreen_target.into());

        self.push_ui(
            UiLayer::new(label.clone(), style)
                .to_target(target.clone())
                .transparent(),
        );
        self.push_effect_layer(EffectLayer::composite(
            format!("{label} Composite"),
            target,
            FrameTarget::SceneColor,
        ));
        self
    }

    pub fn push_fullscreen_ui_with_composite(&mut self, ui_layer: UiLayer) -> &mut Self {
        let packet = ui_layer.into_packet();
        let target = packet.target.clone();
        let label = packet.label.clone();
        self.push_ui_layer(packet);
        self.push_effect(EffectPacket::composite(
            format!("{label} Composite"),
            target,
            FrameTarget::SceneColor,
        ));
        self
    }

    pub fn add_embedded_world_view(
        &mut self,
        label: impl Into<Cow<'static, str>>,
        offscreen_target: impl Into<Cow<'static, str>>,
        target_rect: NormalizedRect,
    ) -> &mut Self {
        let label = label.into();
        let target = FrameTarget::Offscreen(offscreen_target.into());

        self.push_world(WorldView::new(label.clone()).to_target(target.clone()));
        self.push_effect_layer(
            EffectLayer::composite(
                format!("{label} Composite"),
                target,
                FrameTarget::SceneColor,
            )
            .with_target_rect(target_rect),
        );
        self
    }

    pub fn push_embedded_world_with_composite(
        &mut self,
        world_view: WorldView,
        target_rect: NormalizedRect,
    ) -> &mut Self {
        let packet = world_view.into_packet();
        let target = packet.target.clone();
        let label = packet.label.clone();
        self.push_world_view(packet);
        self.push_effect(
            EffectPacket::composite(
                format!("{label} Composite"),
                target,
                FrameTarget::SceneColor,
            )
            .with_target_rect(target_rect),
        );
        self
    }

    pub fn push_world(&mut self, world_view: WorldView) -> &mut Self {
        self.packet.push_world_view(world_view.into_packet());
        self
    }

    pub fn push_ui(&mut self, ui_layer: UiLayer) -> &mut Self {
        self.packet.push_ui_layer(ui_layer.into_packet());
        self
    }

    pub fn push_effect_layer(&mut self, effect_layer: EffectLayer) -> &mut Self {
        self.packet.push_effect(effect_layer.into_packet());
        self
    }

    pub fn push_world_view(&mut self, packet: WorldViewPacket) -> &mut Self {
        self.packet.push_world_view(packet);
        self
    }

    pub fn push_ui_layer(&mut self, packet: UiLayerPacket) -> &mut Self {
        self.packet.push_ui_layer(packet);
        self
    }

    pub fn push_effect(&mut self, packet: EffectPacket) -> &mut Self {
        self.packet.push_effect(packet);
        self
    }

    pub fn into_frame_packet(self) -> FramePacket {
        self.packet
    }
}

#[derive(Debug, Clone)]
pub enum CompiledFramePass {
    WorldView(WorldViewPacket),
    UiLayer(UiLayerPacket),
    Effect(EffectPacket),
    Overlay(OverlayPacket),
    Present(PresentPacket),
}

#[derive(Debug, Clone, Default)]
pub struct CompiledFrameGraph {
    passes: Vec<CompiledFramePass>,
}

impl CompiledFrameGraph {
    pub fn from_packet(packet: &FramePacket) -> Self {
        let mut passes = Vec::with_capacity(packet.items.len());
        for item in packet.items() {
            passes.push(match item {
                FramePacketItem::WorldView(packet) => CompiledFramePass::WorldView(packet.clone()),
                FramePacketItem::UiLayer(packet) => CompiledFramePass::UiLayer(packet.clone()),
                FramePacketItem::Effect(packet) => CompiledFramePass::Effect(packet.clone()),
                FramePacketItem::Overlay(packet) => CompiledFramePass::Overlay(packet.clone()),
                FramePacketItem::Present(packet) => CompiledFramePass::Present(packet.clone()),
            });
        }
        Self { passes }
    }

    pub fn passes(&self) -> &[CompiledFramePass] {
        &self.passes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_physical_demo_packet_starts_with_physical_ui_layer() {
        let packet = FramePacket::for_demo(DemoType::UiPhysical, "TodoMVC 3D");
        assert!(matches!(
            packet.items(),
            [FramePacketItem::UiLayer(UiLayerPacket {
                style: UiLayerStyle::Physical,
                ..
            })]
        ));
    }

    #[test]
    fn compile_preserves_packet_item_order() {
        let mut packet = FramePacket::for_demo(DemoType::World3D, "Objects");
        packet.push_effect(EffectPacket::composite(
            "Composite",
            FrameTarget::Offscreen("source".into()),
            FrameTarget::SceneColor,
        ));
        packet.push_overlay(OverlayPacket::new("Overlay"));
        packet.push_present(PresentPacket::new());

        let graph = packet.compile();
        assert!(matches!(graph.passes()[0], CompiledFramePass::WorldView(_)));
        assert!(matches!(graph.passes()[1], CompiledFramePass::Effect(_)));
        assert!(matches!(graph.passes()[2], CompiledFramePass::Overlay(_)));
        assert!(matches!(graph.passes()[3], CompiledFramePass::Present(_)));
    }

    #[test]
    fn composite_packet_defaults_to_fullscreen_rects() {
        let packet = EffectPacket::composite(
            "Composite",
            FrameTarget::Offscreen("source".into()),
            FrameTarget::SceneColor,
        );
        assert_eq!(packet.source_rect, NormalizedRect::FULLSCREEN);
        assert_eq!(packet.target_rect, NormalizedRect::FULLSCREEN);
    }

    #[test]
    fn wrapper_types_build_expected_packets() {
        let world = WorldView::new("World").to_offscreen("world").into_packet();
        let ui = UiLayer::new("UI", UiLayerStyle::Flat)
            .to_offscreen("ui")
            .transparent()
            .into_packet();
        let effect = EffectLayer::composite(
            "Composite",
            FrameTarget::Offscreen("world".into()),
            FrameTarget::SceneColor,
        )
        .with_target_rect(NormalizedRect::new([0.2, 0.3], [0.4, 0.5]))
        .into_packet();

        assert!(matches!(world.target, FrameTarget::Offscreen(_)));
        assert_eq!(ui.clear_color, Some(wgpu::Color::TRANSPARENT));
        assert_eq!(
            effect.target_rect,
            NormalizedRect::new([0.2, 0.3], [0.4, 0.5])
        );
    }

    #[test]
    fn composed_scene_fullscreen_helpers_emit_expected_packets() {
        let mut scene = ComposedScene::new();
        scene
            .add_fullscreen_world_view("World", "world")
            .add_fullscreen_ui_overlay("UI", UiLayerStyle::Flat, "ui");

        assert!(matches!(
            scene.into_frame_packet().items(),
            [
                FramePacketItem::WorldView(WorldViewPacket { .. }),
                FramePacketItem::Effect(EffectPacket {
                    composite_mode: CompositeMode::Replace,
                    ..
                }),
                FramePacketItem::UiLayer(UiLayerPacket {
                    style: UiLayerStyle::Flat,
                    ..
                }),
                FramePacketItem::Effect(EffectPacket {
                    composite_mode: CompositeMode::Over,
                    ..
                }),
            ]
        ));
    }

    #[test]
    fn composed_scene_embedded_world_view_sets_target_rect() {
        let rect = NormalizedRect::new([0.1, 0.2], [0.3, 0.4]);
        let mut scene = ComposedScene::new();
        scene.add_embedded_world_view("Preview", "preview", rect);

        assert!(matches!(
            scene.into_frame_packet().items(),
            [
                FramePacketItem::WorldView(_),
                FramePacketItem::Effect(EffectPacket { target_rect, .. }),
            ] if *target_rect == rect
        ));
    }

    #[test]
    fn composed_scene_preset_world_with_ui_overlay_orders_world_before_ui() {
        let packet =
            ComposedScene::world_with_ui_overlay("World", "world", "UI", UiLayerStyle::Flat, "ui")
                .into_frame_packet();

        assert!(matches!(
            packet.items(),
            [
                FramePacketItem::WorldView(_),
                FramePacketItem::Effect(EffectPacket {
                    composite_mode: CompositeMode::Replace,
                    ..
                }),
                FramePacketItem::UiLayer(_),
                FramePacketItem::Effect(EffectPacket {
                    composite_mode: CompositeMode::Over,
                    ..
                }),
            ]
        ));
    }

    #[test]
    fn composed_scene_direct_wrapper_overlay_matches_expected_order() {
        let packet = ComposedScene::from_world_with_ui_overlay(
            WorldView::new("World").to_offscreen("world"),
            UiLayer::new("UI", UiLayerStyle::Flat)
                .to_offscreen("ui")
                .transparent(),
        )
        .into_frame_packet();

        assert!(matches!(
            packet.items(),
            [
                FramePacketItem::WorldView(_),
                FramePacketItem::Effect(EffectPacket {
                    composite_mode: CompositeMode::Replace,
                    ..
                }),
                FramePacketItem::UiLayer(_),
                FramePacketItem::Effect(EffectPacket {
                    composite_mode: CompositeMode::Over,
                    ..
                }),
            ]
        ));
    }

    #[test]
    fn composed_scene_direct_wrapper_embedded_preview_sets_rect() {
        let rect = NormalizedRect::new([0.66, 0.12], [0.24, 0.24]);
        let packet = ComposedScene::from_ui_with_embedded_world_view(
            UiLayer::new("UI", UiLayerStyle::Flat)
                .to_offscreen("ui")
                .transparent(),
            WorldView::new("Preview").to_offscreen("preview"),
            rect,
        )
        .into_frame_packet();

        assert!(matches!(
            packet.items(),
            [
                FramePacketItem::UiLayer(_),
                FramePacketItem::Effect(EffectPacket {
                    composite_mode: CompositeMode::Over,
                    ..
                }),
                FramePacketItem::WorldView(_),
                FramePacketItem::Effect(EffectPacket { target_rect, .. }),
            ] if *target_rect == rect
        ));
    }
}
