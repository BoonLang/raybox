//! Web (WASM) version of the unified demo system
//!
//! Supports demo switching, overlay, keybindings, and web control commands.
//! Uses OrbitalCamera for simpler 3D navigation on web.
//! Optionally connects to control server when ?control=1 URL parameter is present.
//! Supports hot-reload via WASM module reloading with state preservation.

#![allow(dead_code)]

use crate::camera::{OrbitalCamera, Uniforms};
use crate::constants::{HEIGHT, WIDTH};
use crate::demo_core::{
    ui_physical_card_camera_preset, DemoId, DemoType, ListFilter, OverlayMode,
    UiPhysicalCameraPreset, KEYBINDINGS_COMMON,
};
use crate::gpu_runtime_common::{
    build_font_gpu_data, create_bind_group_layout_with_storage, create_bind_group_with_storage,
    create_fullscreen_pipeline, create_storage_buffers, UiStorageBuffers,
};
use crate::retained::fixed_scene::{
    BuiltFixedUi2dScene, FixedUi2dSceneModelBuilder, FixedUi2dSceneModelCapture,
    FixedUi2dSceneState,
};
use crate::retained::showcase::{ShowcaseSceneAction, ShowcaseSceneModel};
use crate::retained::text::{FixedTextSceneData, GpuCharInstanceEx, TextColors, TextRenderSpace};
use crate::retained::text_scene::{OwnedTextSceneBlock, WrappedTextSceneModel};
use crate::retained::ui::{GpuUiPrimitive, UiRenderSpace};
use crate::retained::{
    scroll_offset_for_node, set_named_scroll_offset, NamedScrollSceneModel, Rect, RetainedScene,
    SceneMode, TextRole, UiVisualRole,
};
use crate::shader_bindings::{
    sdf_clay_vector, sdf_raymarch, sdf_spheres, sdf_text2d_vector, sdf_text_shadow_vector,
    sdf_towers,
};
use crate::text::{VectorFont, VectorFontAtlas};
use crate::todomvc_retained::TodoMvcRetainedScene;
use crate::todomvc_shared::{
    build_text_scene_data_from_scene as build_todomvc_text_scene_data_from_scene,
    build_ui_primitives_from_scene as build_todomvc_ui_primitives_from_scene,
    classic_text_colors as todomvc_classic_text_colors,
};
use crate::ui2d_shader_bindings as retained_ui2d_shader;
use crate::ui_physical_shader_bindings as retained_ui_physical_shader;
use crate::ui_physical_theme::{
    ThemeId, ThemeUniforms, UiPhysicalThemeState, PHYSICAL_THEME_OPTIONS,
};
use crate::web_control::{
    self, error_response, pong_response, screenshot_response, status_response, success_response,
    SharedWebControlState, WebCommand, WebWsClient,
};
use crate::web_input::WebInputHandler;
use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wgpu::util::DeviceExt;

// Global state for hot-reload preservation
thread_local! {
    static SAVED_STATE: RefCell<Option<WebReloadableState>> = const { RefCell::new(None) };
    static RENDERER_REF: RefCell<Option<Rc<RefCell<WebRenderer>>>> = const { RefCell::new(None) };
}

/// Serializable state for web hot-reload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebReloadableState {
    pub current_demo: u8,
    pub camera_distance: f32,
    pub camera_azimuth: f32,
    pub camera_elevation: f32,
    pub camera_target: [f32; 3],
    pub overlay_mode: String,
    pub show_keybindings: bool,
    pub text2d_offset: [f32; 2],
    pub text2d_scale: f32,
    pub text2d_rotation: f32,
    pub time_offset: f32,
}

impl Default for WebReloadableState {
    fn default() -> Self {
        Self {
            current_demo: 1,
            camera_distance: 5.0,
            camera_azimuth: 0.0,
            camera_elevation: 0.0,
            camera_target: [0.0, 0.0, 0.0],
            overlay_mode: "off".to_string(),
            show_keybindings: false,
            text2d_offset: [0.0, 0.0],
            text2d_scale: 1.0,
            text2d_rotation: 0.0,
            time_offset: 0.0,
        }
    }
}

/// Web demo implementation (simplified version of native Demo trait)
trait WebDemo {
    fn name(&self) -> &'static str;
    fn id(&self) -> DemoId;
    fn demo_type(&self) -> DemoType;
    fn keybindings(&self) -> &'static [(&'static str, &'static str)];
    fn camera_config(&self) -> (glam::Vec3, glam::Vec3); // (position, target)
    fn ui_physical_camera_preset(&self) -> Option<UiPhysicalCameraPreset> {
        None
    }
    fn update(&mut self, dt: f32);
    fn update_uniforms(&self, queue: &wgpu::Queue, camera: &OrbitalCamera, time: f32);
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>);
    fn resize(&mut self, width: u32, height: u32);
    fn set_ui2d_view_state(&mut self, _offset: [f32; 2], _scale: f32, _rotation: f32) {}
    fn handle_key_pressed(&mut self, _code: &str) -> bool {
        false
    }
    fn handle_key_held(&mut self, _code: &str) -> bool {
        false
    }
    fn toggle_list_item(&mut self, _index: u32) -> bool {
        false
    }
    fn set_list_item_completed(&mut self, _index: u32, _completed: bool) -> bool {
        false
    }
    fn set_list_item_label(&mut self, _index: u32, _label: &str) -> bool {
        false
    }
    fn set_list_filter(&mut self, _filter: &str) -> Option<String> {
        None
    }
    fn set_list_scroll_offset(&mut self, _offset_y: f32) -> bool {
        false
    }
    fn set_named_scroll(&mut self, _name: &str, _offset_y: f32) -> bool {
        false
    }
    fn set_named_theme(
        &mut self,
        _theme: &str,
        _dark_mode: Option<bool>,
    ) -> Option<(&'static str, bool)> {
        None
    }
    fn named_theme_options(&self) -> &'static [&'static str] {
        &[]
    }
}

type World3dShaderFactory = fn(&wgpu::Device) -> wgpu::ShaderModule;

fn create_web_world3d_bind_group_layout(
    device: &wgpu::Device,
    label: &str,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(&format!("{label} Bind Group Layout")),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

fn create_web_world3d_bind_group(
    device: &wgpu::Device,
    label: &str,
    bind_group_layout: &wgpu::BindGroupLayout,
    uniform_buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(&format!("{label} Bind Group")),
        layout: bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    })
}

fn create_web_world3d_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    label: &str,
    shader_module: &wgpu::ShaderModule,
    bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&format!("{label} Pipeline Layout")),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(&format!("{label} Pipeline")),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader_module,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader_module,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

struct WebWorld3dUniformHost<U: Pod> {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    _marker: std::marker::PhantomData<U>,
}

impl<U: Pod> WebWorld3dUniformHost<U> {
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        label: &str,
        shader_factory: World3dShaderFactory,
        initial_uniforms: &U,
    ) -> Self {
        let shader_module = shader_factory(device);

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label} Uniform Buffer")),
            contents: bytemuck::bytes_of(initial_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = create_web_world3d_bind_group_layout(device, label);
        let bind_group =
            create_web_world3d_bind_group(device, label, &bind_group_layout, &uniform_buffer);
        let pipeline =
            create_web_world3d_pipeline(device, format, label, &shader_module, &bind_group_layout);

        Self {
            pipeline,
            uniform_buffer,
            bind_group,
            width: WIDTH,
            height: HEIGHT,
            _marker: std::marker::PhantomData,
        }
    }

    fn write_uniforms(&self, queue: &wgpu::Queue, uniforms: &U) {
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
}

type WebWorld3dStorageHost<U> = WebWorld3dUniformHost<U>;

struct SimpleWorld3dDemo {
    name: &'static str,
    id: DemoId,
    camera_position: glam::Vec3,
    camera_target: glam::Vec3,
    host: WebWorld3dUniformHost<Uniforms>,
}

impl SimpleWorld3dDemo {
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        label: &str,
        shader_factory: World3dShaderFactory,
        name: &'static str,
        id: DemoId,
        camera_position: glam::Vec3,
        camera_target: glam::Vec3,
    ) -> Self {
        let host =
            WebWorld3dUniformHost::new(device, format, label, shader_factory, &Uniforms::default());
        Self {
            name,
            id,
            camera_position,
            camera_target,
            host,
        }
    }
}

impl WebDemo for SimpleWorld3dDemo {
    fn name(&self) -> &'static str {
        self.name
    }

    fn id(&self) -> DemoId {
        self.id
    }

    fn demo_type(&self) -> DemoType {
        DemoType::World3D
    }

    fn keybindings(&self) -> &'static [(&'static str, &'static str)] {
        KEYBINDINGS_3D_WEB
    }

    fn camera_config(&self) -> (glam::Vec3, glam::Vec3) {
        (self.camera_position, self.camera_target)
    }

    fn update(&mut self, _dt: f32) {}

    fn update_uniforms(&self, queue: &wgpu::Queue, camera: &OrbitalCamera, time: f32) {
        let mut uniforms = Uniforms::default();
        uniforms.update_from_camera(camera, self.host.width, self.host.height, time);
        self.host.write_uniforms(queue, &uniforms);
    }

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.host.render(render_pass);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.host.resize(width, height);
    }
}

// Standard 3D keybindings for web (orbital camera)
const KEYBINDINGS_3D_WEB: &[(&str, &str)] = &[
    ("A/D", "Rotate horizontal"),
    ("W/S", "Zoom"),
    ("Q/E", "Rotate vertical"),
];

// Standard 2D keybindings for web
const KEYBINDINGS_2D_WEB: &[(&str, &str)] = &[
    ("A/D", "Pan horizontal"),
    ("W/S", "Pan vertical"),
    ("Arrows", "Zoom"),
    ("Q/E", "Rotate"),
];

fn demo_family_name(demo_type: DemoType) -> &'static str {
    match demo_type {
        DemoType::Ui2D => "ui2d",
        DemoType::UiPhysical => "uiPhysical",
        DemoType::World3D => "world3d",
    }
}

fn ui_physical_camera_target(demo: &dyn WebDemo) -> glam::Vec3 {
    demo.camera_config().1
}

// ============== EMPTY DEMO ==============

struct PlaceholderDemo {
    pipeline: wgpu::RenderPipeline,
    name: &'static str,
    id: DemoId,
    demo_type: DemoType,
    keybindings: &'static [(&'static str, &'static str)],
    camera_position: glam::Vec3,
    camera_target: glam::Vec3,
    width: u32,
    height: u32,
}

impl PlaceholderDemo {
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        name: &'static str,
        id: DemoId,
        demo_type: DemoType,
        keybindings: &'static [(&'static str, &'static str)],
        camera_position: glam::Vec3,
        camera_target: glam::Vec3,
    ) -> Self {
        // Simple pass-through shader for empty scene
        let shader_module = sdf_raymarch::create_shader_module_embed_source(device);

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Empty Bind Group Layout"),
            entries: &[],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Empty Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Empty Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            name,
            id,
            demo_type,
            keybindings,
            camera_position,
            camera_target,
            width: WIDTH,
            height: HEIGHT,
        }
    }
}

impl WebDemo for PlaceholderDemo {
    fn name(&self) -> &'static str {
        self.name
    }
    fn id(&self) -> DemoId {
        self.id
    }
    fn demo_type(&self) -> DemoType {
        self.demo_type
    }
    fn keybindings(&self) -> &'static [(&'static str, &'static str)] {
        self.keybindings
    }
    fn camera_config(&self) -> (glam::Vec3, glam::Vec3) {
        (self.camera_position, self.camera_target)
    }
    fn update(&mut self, _dt: f32) {}
    fn update_uniforms(&self, _queue: &wgpu::Queue, _camera: &OrbitalCamera, _time: f32) {}
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        // Don't draw anything - just show the clear color
    }
    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
}

// ============== TEXT2D DEMO ==============
// Note: The Text2D demo uses a vector text renderer which requires storage buffers.
// WebGL2 may not support this. For now, we use the same shader pattern.

struct Text2DDemo {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    offset: [f32; 2],
    scale: f32,
    rotation: f32,
    width: u32,
    height: u32,
}

impl Text2DDemo {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        // Use the text2d vector shader
        let shader_module = sdf_text2d_vector::create_shader_module_embed_source(device);

        // Create uniform buffer with 2D transform data
        let uniform_data: [f32; 8] = [
            0.0,
            0.0, // offset
            1.0,
            0.0, // scale, rotation
            WIDTH as f32,
            HEIGHT as f32, // resolution
            0.0,
            0.0, // padding
        ];
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Text2D Uniform Buffer"),
            contents: bytemuck::cast_slice(&uniform_data),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Text2D Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Text2D Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Text2D Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Text2D Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            uniform_buffer,
            bind_group,
            offset: [0.0, 0.0],
            scale: 1.0,
            rotation: 0.0,
            width: WIDTH,
            height: HEIGHT,
        }
    }
}

impl WebDemo for Text2DDemo {
    fn name(&self) -> &'static str {
        "2D Text"
    }
    fn id(&self) -> DemoId {
        DemoId::Text2D
    }
    fn demo_type(&self) -> DemoType {
        DemoType::Ui2D
    }
    fn keybindings(&self) -> &'static [(&'static str, &'static str)] {
        KEYBINDINGS_2D_WEB
    }
    fn camera_config(&self) -> (glam::Vec3, glam::Vec3) {
        (glam::Vec3::ZERO, glam::Vec3::ZERO)
    }
    fn update(&mut self, _dt: f32) {}
    fn update_uniforms(&self, queue: &wgpu::Queue, _camera: &OrbitalCamera, _time: f32) {
        let uniform_data: [f32; 8] = [
            self.offset[0],
            self.offset[1],
            self.scale,
            self.rotation,
            self.width as f32,
            self.height as f32,
            0.0,
            0.0,
        ];
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&uniform_data));
    }
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
    fn set_ui2d_view_state(&mut self, offset: [f32; 2], scale: f32, rotation: f32) {
        self.offset = offset;
        self.scale = scale;
        self.rotation = rotation;
    }
}

const WEB_UI2D_STORAGE_BINDINGS: [u32; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
const WEB_RETAINED_SCROLL_STEP: f32 = 24.0;
const RETAINED_UI_KEYBINDINGS_WEB: &[(&str, &str)] = &[
    ("WASD", "Pan"),
    ("Arrows", "Zoom"),
    ("Q/E", "Rotate"),
    ("Y", "Next retained scene"),
    ("O", "Toggle active scene state"),
    ("U/J", "Scroll active retained scene"),
];
const TODOMVC_KEYBINDINGS_WEB: &[(&str, &str)] = &[
    ("A/D", "Pan horizontal"),
    ("W/S", "Pan vertical"),
    ("Arrows", "Zoom"),
    ("Q/E", "Rotate"),
    ("Y", "Cycle filter"),
    ("O", "Toggle first item"),
    ("U/J", "Scroll list"),
];
const RETAINED_UI_PHYSICAL_KEYBINDINGS_WEB: &[(&str, &str)] = &[
    ("A/D", "Rotate horizontal"),
    ("W/S", "Zoom"),
    ("Q/E", "Rotate vertical"),
    ("Y", "Next retained scene"),
    ("O", "Toggle active scene state"),
    ("U/J", "Scroll active retained scene"),
    ("N", "Cycle theme"),
    ("M", "Toggle dark mode"),
];
const TEXT_PHYSICAL_KEYBINDINGS_WEB: &[(&str, &str)] = &[
    ("A/D", "Rotate horizontal"),
    ("W/S", "Zoom"),
    ("Q/E", "Rotate vertical"),
    ("Y", "Toggle heading emphasis"),
    ("U/J", "Scroll text"),
    ("N", "Cycle theme"),
    ("M", "Toggle dark mode"),
];
const TODOMVC_3D_KEYBINDINGS_WEB: &[(&str, &str)] = &[
    ("A/D", "Rotate horizontal"),
    ("W/S", "Zoom"),
    ("Q/E", "Rotate vertical"),
    ("N", "Cycle theme"),
    ("M", "Toggle dark mode"),
];
const WEB_TODOMVC_ITALIC_CODEPOINT_OFFSET: u32 = 0x10000;
const WEB_UI_PHYSICAL_STORAGE_BINDINGS: [u32; 8] = [2, 3, 4, 5, 6, 7, 8, 9];
const SHOWCASE_PHYSICAL_CARD_SIZE: [f32; 2] = [392.0, 224.0];
const TEXT_PHYSICAL_FRAME_SIZE: [f32; 2] = [760.0, 560.0];
const TEXT_PHYSICAL_TEXT_MARGIN: f32 = 20.0;
const TEXT_PHYSICAL_BODY_FONT_SIZE: f32 = 20.0;
const TEXT_PHYSICAL_BODY_LINE_HEIGHT: f32 = 31.0;
const TEXT_PHYSICAL_HEADING_FONT_SIZE: f32 = 34.0;
const TEXT_PHYSICAL_GRID_DIMS: [u32; 2] = [64, 48];
const TEXT_PHYSICAL_GRID_CELL_CAPACITY: usize = 64;
const TEXT_PHYSICAL_HEADING_TOP_PADDING: f32 = 12.0;
const TEXT_PHYSICAL_SCROLL_STEP: f32 = 48.0;
const TEXT_PHYSICAL_PARAGRAPH_COUNT: usize = 5;
const TODO_CLASSIC_DECAL_PRIM_START: f32 = 7.0;
const TODO_PHYSICAL_SCREEN_H: f32 = 700.0;
const TODO_PHYSICAL_CARD_BOUNDS: [f32; 4] = [75.0, 225.8, 625.0, 570.0];
const TODO_PHYSICAL_FILL_COLOR: [f32; 4] = [248.0 / 255.0, 250.0 / 255.0, 252.0 / 255.0, 1.0];
const TODO_PHYSICAL_OUTLINE_COLOR: [f32; 4] = [203.0 / 255.0, 213.0 / 255.0, 225.0 / 255.0, 1.0];
const TODO_PHYSICAL_SHADOW_COLOR: [f32; 4] = [15.0 / 255.0, 23.0 / 255.0, 42.0 / 255.0, 0.16];
const TEXT_PHYSICAL_LOREM: &str = "Retained physical UI should support text-heavy scenes without collapsing back into Todo-shaped assumptions. This demo exercises wrapped retained text, scrolling, and semantic text mutation through the shared UiPhysical runtime path.\n\nA retained physical scene should stay stable while idle, rebuild only what changed, and let the runtime choose how to realize the card, lighting, and text presentation.\n\nScrolling this text should work through the same retained model + named scroll infrastructure that powers other scenes.";

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, bytemuck::Zeroable)]
struct WebUi2dUniforms {
    screen_params: [f32; 4],
    offset: [f32; 2],
    _pad0: [f32; 2],
    text_params: [f32; 4],
    char_grid_params: [f32; 4],
    char_grid_bounds: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct WebUi2dViewState {
    offset: [f32; 2],
    scale: f32,
    rotation: f32,
}

impl Default for WebUi2dViewState {
    fn default() -> Self {
        Self {
            offset: [0.0, 0.0],
            scale: 1.0,
            rotation: 0.0,
        }
    }
}

struct WebRetainedUiPass {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    storage_buffers: UiStorageBuffers,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    char_count: u32,
    ui_prim_count: u32,
    char_grid_params: [f32; 4],
    char_grid_bounds: [f32; 4],
    text_capacity: usize,
    grid_index_capacity: usize,
    primitive_capacity: usize,
    view_state: WebUi2dViewState,
}

struct WebRetainedUiRuntimeHost {
    pass: WebRetainedUiPass,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_format: wgpu::TextureFormat,
    label: String,
    width: u32,
    height: u32,
}

struct WebRetainedUiDemo {
    models: Vec<ShowcaseSceneModel>,
    active_scene: usize,
    scene_state: FixedUi2dSceneState,
    runtime_host: WebRetainedUiRuntimeHost,
    atlas: Arc<VectorFontAtlas>,
    text_colors: TextColors,
    width: u32,
    height: u32,
}

struct WebTodoMvcDemo {
    retained_scene: TodoMvcRetainedScene,
    runtime_host: WebRetainedUiRuntimeHost,
    atlas: Arc<VectorFontAtlas>,
    text_colors: TextColors,
    width: u32,
    height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WebUiPhysicalLayout {
    center_px: [f32; 2],
    bounds_px: [f32; 4],
    corner_radius_px: f32,
    content_inset_px: f32,
    elevation_px: f32,
    depth_px: f32,
    fill_color: [f32; 4],
    accent_color: [f32; 4],
    detail_color: [f32; 4],
    outline_color: [f32; 4],
    outline_width_px: f32,
    shadow_color: [f32; 4],
    shadow_offset_px: [f32; 2],
    shadow_extra_size_px: [f32; 2],
    pixel_to_world: f32,
    geometry_mode: f32,
}

impl Default for WebUiPhysicalLayout {
    fn default() -> Self {
        Self {
            center_px: [350.0, 398.0],
            bounds_px: [75.0, 225.8, 625.0, 570.0],
            corner_radius_px: 12.0,
            content_inset_px: 0.0,
            elevation_px: 6.0,
            depth_px: 10.0,
            fill_color: [248.0 / 255.0, 250.0 / 255.0, 252.0 / 255.0, 1.0],
            accent_color: [0.0; 4],
            detail_color: [0.0; 4],
            outline_color: [203.0 / 255.0, 213.0 / 255.0, 225.0 / 255.0, 0.0],
            outline_width_px: 0.0,
            shadow_color: [15.0 / 255.0, 23.0 / 255.0, 42.0 / 255.0, 0.16],
            shadow_offset_px: [0.0, 14.0],
            shadow_extra_size_px: [12.0, 12.0],
            pixel_to_world: 0.01,
            geometry_mode: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct WebUiPhysicalUniforms {
    inv_view_proj: [[f32; 4]; 4],
    camera_pos_time: [f32; 4],
    light_dir_intensity: [f32; 4],
    render_params: [f32; 4],
    text_params: [f32; 4],
    char_grid_params: [f32; 4],
    char_grid_bounds: [f32; 4],
    layout_params0: [f32; 4],
    layout_params1: [f32; 4],
    layout_params2: [f32; 4],
    layout_params3: [f32; 4],
    layout_params4: [f32; 4],
    layout_params5: [f32; 4],
    layout_params6: [f32; 4],
    layout_params7: [f32; 4],
    layout_params8: [f32; 4],
    layout_params9: [f32; 4],
    layout_bounds: [f32; 4],
}

impl Default for WebUiPhysicalUniforms {
    fn default() -> Self {
        Self {
            inv_view_proj: [[0.0; 4]; 4],
            camera_pos_time: [0.0, 3.5, 3.5, 0.0],
            light_dir_intensity: [0.5, 0.8, 0.3, 1.5],
            render_params: [800.0, 600.0, 0.08, 1.0],
            text_params: [0.0; 4],
            char_grid_params: [0.0; 4],
            char_grid_bounds: [0.0; 4],
            layout_params0: [350.0, 398.0, 0.01, 1.0],
            layout_params1: [12.0, 0.0, 8.0, 0.0],
            layout_params2: [248.0 / 255.0, 250.0 / 255.0, 252.0 / 255.0, 1.0],
            layout_params3: [203.0 / 255.0, 213.0 / 255.0, 225.0 / 255.0, 1.0],
            layout_params4: [15.0 / 255.0, 23.0 / 255.0, 42.0 / 255.0, 0.16],
            layout_params5: [0.0, 14.0, 0.0, 0.0],
            layout_params6: [12.0, 12.0, 0.0, 0.0],
            layout_params7: [1.0, 0.0, 0.0, 0.0],
            layout_params8: [0.0; 4],
            layout_params9: [0.0; 4],
            layout_bounds: [75.0, 225.8, 625.0, 570.0],
        }
    }
}

struct WebUiPhysicalPass {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    theme_buffer: wgpu::Buffer,
    storage_buffers: UiStorageBuffers,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    char_count: u32,
    ui_prim_count: u32,
    char_grid_params: [f32; 4],
    char_grid_bounds: [f32; 4],
    layout: WebUiPhysicalLayout,
    text_capacity: usize,
    grid_index_capacity: usize,
    primitive_capacity: usize,
}

struct WebUiPhysicalRuntimeHost {
    pass: WebUiPhysicalPass,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_format: wgpu::TextureFormat,
    label: String,
    width: u32,
    height: u32,
}

struct WebRetainedUiPhysicalDemo {
    models: Vec<ShowcaseSceneModel>,
    active_scene: usize,
    scene_state: FixedUi2dSceneState,
    runtime_host: WebUiPhysicalRuntimeHost,
    atlas: Arc<VectorFontAtlas>,
    theme_state: UiPhysicalThemeState,
    width: u32,
    height: u32,
}

struct WebTextPhysicalDemo {
    model: WrappedTextSceneModel,
    scene_state: FixedUi2dSceneState,
    runtime_host: WebUiPhysicalRuntimeHost,
    atlas: Arc<VectorFontAtlas>,
    theme_state: UiPhysicalThemeState,
    width: u32,
    height: u32,
}

struct WebTodoMvc3DDemo {
    retained_scene: TodoMvcRetainedScene,
    runtime_host: WebUiPhysicalRuntimeHost,
    atlas: Arc<VectorFontAtlas>,
    theme_state: UiPhysicalThemeState,
    width: u32,
    height: u32,
}

fn virtual_size_from_bounds(bounds: [f32; 4]) -> [f32; 2] {
    let width = (bounds[2] - bounds[0]).max(1.0);
    let height = (bounds[3] - bounds[1]).max(1.0);
    [width, height]
}

fn web_retained_text_colors() -> TextColors {
    TextColors {
        heading: [0.13, 0.16, 0.23],
        active: [0.20, 0.24, 0.31],
        completed: [0.39, 0.45, 0.55],
        placeholder: [0.58, 0.64, 0.72],
        body: [0.23, 0.29, 0.36],
        info: [0.42, 0.48, 0.58],
    }
}

fn web_retained_text_space(screen_height: f32) -> TextRenderSpace {
    TextRenderSpace {
        x_offset: 0.0,
        screen_height,
        italic_codepoint_offset: 0x10000,
    }
}

fn web_retained_ui_space(screen_height: f32) -> UiRenderSpace {
    UiRenderSpace {
        x_offset: 0.0,
        screen_height,
    }
}

fn load_web_retained_font_atlas() -> Arc<VectorFontAtlas> {
    let regular = include_bytes!("../assets/fonts/LiberationSans-Regular.ttf");
    let italic = include_bytes!("../assets/fonts/LiberationSans-Italic.ttf");
    let mut font = VectorFont::from_ttf(regular).expect("parse retained web regular font");
    font.merge_from_ttf(italic, 0x10000)
        .expect("merge retained web italic font");
    Arc::new(VectorFontAtlas::from_font(&font, 32))
}

fn web_retained_init_from_built(
    built: BuiltFixedUi2dScene,
) -> (FixedUi2dSceneState, FixedTextSceneData, Vec<GpuUiPrimitive>) {
    let BuiltFixedUi2dScene { state, init } = built;
    (state, init.text_data, init.ui_data.primitives)
}

fn web_generic_physical_init_from_built(
    built: BuiltFixedUi2dScene,
) -> (
    FixedUi2dSceneState,
    FixedTextSceneData,
    Vec<GpuUiPrimitive>,
    WebUiPhysicalLayout,
) {
    let BuiltFixedUi2dScene { state, init } = built;
    let layout =
        web_generic_physical_layout(&state.scene, &init.text_data, &init.ui_data.primitives);
    (state, init.text_data, init.ui_data.primitives, layout)
}

fn web_todomvc_scene_buffers(
    retained_scene: &TodoMvcRetainedScene,
    atlas: &VectorFontAtlas,
    text_colors: &TextColors,
) -> (FixedTextSceneData, Vec<GpuUiPrimitive>) {
    (
        build_todomvc_text_scene_data_from_scene(
            retained_scene.scene(),
            atlas,
            text_colors,
            WEB_TODOMVC_ITALIC_CODEPOINT_OFFSET,
        ),
        build_todomvc_ui_primitives_from_scene(retained_scene.scene()),
    )
}

fn tune_generic_ui_physical_text_color(color: [f32; 3]) -> [f32; 3] {
    let luminance = 0.2126 * color[0] + 0.7152 * color[1] + 0.0722 * color[2];
    let target = if luminance > 0.5 {
        [0.07, 0.07, 0.08]
    } else {
        [0.94, 0.95, 0.97]
    };
    let strength = if luminance > 0.5 { 0.22 } else { 0.12 };
    [
        color[0] * (1.0 - strength) + target[0] * strength,
        color[1] * (1.0 - strength) + target[1] * strength,
        color[2] * (1.0 - strength) + target[2] * strength,
    ]
}

fn tune_generic_ui_physical_text_colors(colors: TextColors) -> TextColors {
    TextColors {
        heading: tune_generic_ui_physical_text_color(colors.heading),
        active: tune_generic_ui_physical_text_color(colors.active),
        completed: tune_generic_ui_physical_text_color(colors.completed),
        placeholder: tune_generic_ui_physical_text_color(colors.placeholder),
        body: tune_generic_ui_physical_text_color(colors.body),
        info: tune_generic_ui_physical_text_color(colors.info),
    }
}

fn text_physical_scene_model() -> WrappedTextSceneModel {
    WrappedTextSceneModel {
        scene_mode: SceneMode::UiPhysical,
        heading: Some(OwnedTextSceneBlock {
            text: "RETAINED UI PHYSICAL TEXT".to_string(),
            font_size: TEXT_PHYSICAL_HEADING_FONT_SIZE,
            role: TextRole::Heading,
        }),
        body: OwnedTextSceneBlock {
            text: std::iter::repeat(TEXT_PHYSICAL_LOREM)
                .take(TEXT_PHYSICAL_PARAGRAPH_COUNT)
                .collect::<Vec<_>>()
                .join("\n\n"),
            font_size: TEXT_PHYSICAL_BODY_FONT_SIZE,
            role: TextRole::Body,
        },
        frame_size: Some(TEXT_PHYSICAL_FRAME_SIZE),
        margin: TEXT_PHYSICAL_TEXT_MARGIN,
        body_line_height: TEXT_PHYSICAL_BODY_LINE_HEIGHT,
        body_top_padding: TEXT_PHYSICAL_HEADING_TOP_PADDING,
        scroll_offset: 0.0,
        grid_dims: TEXT_PHYSICAL_GRID_DIMS,
        grid_cell_capacity: TEXT_PHYSICAL_GRID_CELL_CAPACITY,
        clip_name: "text_physical_clip",
        scroll_name: "text_physical_scroll",
        heading_name: "text_physical_heading",
        line_name_prefix: "text_physical_line_",
    }
}

fn gpu_ui_primitive_bounds(primitive: &GpuUiPrimitive) -> [f32; 4] {
    let prim_type = primitive.params[3];
    if prim_type < 0.5 || (prim_type >= 4.5 && prim_type < 5.5) {
        let x = primitive.pos_size[0];
        let y = primitive.pos_size[1];
        let w = primitive.pos_size[2];
        let h = primitive.pos_size[3];
        [x, y, x + w, y + h]
    } else if prim_type < 1.5 {
        let x = primitive.pos_size[0];
        let y = primitive.pos_size[1];
        let w = primitive.pos_size[2];
        let h = primitive.pos_size[3];
        [x, y, x + w, y + h]
    } else if prim_type < 3.5 {
        let cx = primitive.pos_size[0];
        let cy = primitive.pos_size[1];
        let r = primitive.pos_size[2].abs();
        [cx - r, cy - r, cx + r, cy + r]
    } else if prim_type < 4.5 {
        let x0 = primitive.pos_size[0].min(primitive.pos_size[2]);
        let y0 = primitive.pos_size[1].min(primitive.pos_size[3]);
        let x1 = primitive.pos_size[0].max(primitive.pos_size[2]);
        let y1 = primitive.pos_size[1].max(primitive.pos_size[3]);
        [x0, y0, x1, y1]
    } else if prim_type < 6.5 {
        let x0 = primitive.pos_size[0]
            .min(primitive.pos_size[2])
            .min(primitive.extra[0]);
        let y0 = primitive.pos_size[1]
            .min(primitive.pos_size[3])
            .min(primitive.extra[1]);
        let x1 = primitive.pos_size[0]
            .max(primitive.pos_size[2])
            .max(primitive.extra[0]);
        let y1 = primitive.pos_size[1]
            .max(primitive.pos_size[3])
            .max(primitive.extra[1]);
        [x0, y0, x1, y1]
    } else {
        [0.0; 4]
    }
}

fn text_scene_bounds(text_data: &FixedTextSceneData) -> Option<[f32; 4]> {
    let count = (text_data.char_count as usize).min(text_data.char_instances.len());
    if count == 0 {
        return None;
    }

    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    let mut found = false;

    for inst in text_data.char_instances.iter().take(count) {
        let x = inst.pos_and_char[0];
        let y = inst.pos_and_char[1];
        let font_size = inst.pos_and_char[2].abs();
        if font_size <= 0.0 {
            continue;
        }

        let width = font_size * 0.7;
        let height = font_size * 1.15;
        min_x = min_x.min(x);
        min_y = min_y.min(y - height);
        max_x = max_x.max(x + width);
        max_y = max_y.max(y);
        found = true;
    }

    found.then_some([min_x, min_y, max_x, max_y])
}

fn web_generic_physical_layout(
    scene: &RetainedScene,
    text_data: &FixedTextSceneData,
    ui_primitives: &[GpuUiPrimitive],
) -> WebUiPhysicalLayout {
    let semantic_surface = scene
        .nodes()
        .values()
        .filter(|node| node.ui_visual_role == Some(UiVisualRole::FilledSurface))
        .filter_map(|node| scene.resolved_bounds(node.id).map(|bounds| (node, bounds)))
        .max_by(|a, b| {
            let a_area = a.1.width * a.1.height;
            let b_area = b.1.width * b.1.height;
            a_area.total_cmp(&b_area)
        });
    let semantic_outline = scene
        .nodes()
        .values()
        .filter(|node| node.ui_visual_role == Some(UiVisualRole::OutlineRect))
        .filter_map(|node| scene.resolved_bounds(node.id).map(|bounds| (node, bounds)))
        .max_by(|a, b| {
            let a_area = a.1.width * a.1.height;
            let b_area = b.1.width * b.1.height;
            a_area.total_cmp(&b_area)
        });
    let semantic_shadow = scene
        .nodes()
        .values()
        .filter(|node| node.ui_visual_role == Some(UiVisualRole::BoxShadow))
        .filter_map(|node| scene.resolved_bounds(node.id).map(|bounds| (node, bounds)))
        .max_by(|a, b| {
            let a_area = a.1.width * a.1.height;
            let b_area = b.1.width * b.1.height;
            a_area.total_cmp(&b_area)
        });

    let clip_bounds = scene
        .nodes()
        .values()
        .filter(|node| node.clip || node.element == crate::retained::ElementKind::Clip)
        .filter_map(|clip| scene.resolved_bounds(clip.id))
        .reduce(|acc, bounds| {
            Rect::new(
                acc.x.min(bounds.x),
                acc.y.min(bounds.y),
                (acc.x + acc.width).max(bounds.x + bounds.width) - acc.x.min(bounds.x),
                (acc.y + acc.height).max(bounds.y + bounds.height) - acc.y.min(bounds.y),
            )
        });
    let text_bounds = text_scene_bounds(text_data)
        .and_then(|bounds| {
            let rect = Rect::new(
                bounds[0],
                bounds[1],
                bounds[2] - bounds[0],
                bounds[3] - bounds[1],
            );
            match clip_bounds {
                Some(clip) => rect.intersect(clip).or(Some(clip)),
                None => Some(rect),
            }
        })
        .map(|bounds| {
            [
                bounds.x,
                bounds.y,
                bounds.x + bounds.width,
                bounds.y + bounds.height,
            ]
        });
    let base_bounds = semantic_surface
        .map(|(_, bounds)| bounds)
        .or_else(|| semantic_outline.map(|(_, bounds)| bounds))
        .or(clip_bounds)
        .or_else(|| {
            text_bounds.map(|bounds| {
                Rect::new(
                    bounds[0],
                    bounds[1],
                    bounds[2] - bounds[0],
                    bounds[3] - bounds[1],
                )
            })
        });

    let mut min_x = base_bounds.map(|b| b.x).unwrap_or(f32::INFINITY);
    let mut min_y = base_bounds.map(|b| b.y).unwrap_or(f32::INFINITY);
    let mut max_x = base_bounds
        .map(|b| b.x + b.width)
        .unwrap_or(f32::NEG_INFINITY);
    let mut max_y = base_bounds
        .map(|b| b.y + b.height)
        .unwrap_or(f32::NEG_INFINITY);
    let has_semantic_frame = semantic_surface.is_some() || semantic_outline.is_some();

    let mut layout = WebUiPhysicalLayout::default();

    if let Some((node, bounds)) = semantic_surface {
        min_x = min_x.min(bounds.x);
        min_y = min_y.min(bounds.y);
        max_x = max_x.max(bounds.x + bounds.width);
        max_y = max_y.max(bounds.y + bounds.height);
        if let Some(style) = node.ui_visual_style {
            layout.corner_radius_px = style.corner_radius.max(0.0);
            layout.fill_color = style.base_color;
            layout.accent_color = style.accent_color;
            layout.detail_color = style.detail_color;
        }
        layout.elevation_px = node.elevation.max(0.0);
        layout.depth_px = node.depth.max(0.0);
    }
    if let Some((node, bounds)) = semantic_outline {
        min_x = min_x.min(bounds.x);
        min_y = min_y.min(bounds.y);
        max_x = max_x.max(bounds.x + bounds.width);
        max_y = max_y.max(bounds.y + bounds.height);
        if let Some(style) = node.ui_visual_style {
            layout.outline_color = style.base_color;
            layout.outline_width_px = style.stroke_width.max(0.0);
            layout.corner_radius_px = layout.corner_radius_px.max(style.corner_radius.max(0.0));
        }
    }

    let semantic_alpha = layout.fill_color[3].clamp(0.0, 1.0);
    let detail_mix = layout.detail_color[3].clamp(0.0, 1.0) * 0.25;
    let accent_mix = layout.accent_color[3].clamp(0.0, 1.0) * 0.12;
    let shadow_rgb = [
        layout.fill_color[0] * 0.20 * (1.0 - detail_mix)
            + layout.detail_color[0] * detail_mix
            + layout.accent_color[0] * accent_mix,
        layout.fill_color[1] * 0.22 * (1.0 - detail_mix)
            + layout.detail_color[1] * detail_mix
            + layout.accent_color[1] * accent_mix,
        layout.fill_color[2] * 0.28 * (1.0 - detail_mix)
            + layout.detail_color[2] * detail_mix
            + layout.accent_color[2] * accent_mix,
    ];
    layout.shadow_color = [
        shadow_rgb[0],
        shadow_rgb[1],
        shadow_rgb[2],
        0.10 + semantic_alpha * 0.06,
    ];
    layout.shadow_offset_px = [0.0, layout.elevation_px * 1.2 + layout.depth_px * 0.35];
    layout.shadow_extra_size_px = [
        layout.depth_px * 0.45,
        layout.elevation_px * 0.25 + layout.depth_px * 0.35,
    ];

    if let Some((node, _bounds)) = semantic_shadow {
        if let Some(style) = node.ui_visual_style {
            layout.shadow_color = style.base_color;
            layout.shadow_offset_px = style.offset;
            layout.shadow_extra_size_px =
                [style.extra_size[0].max(0.0), style.extra_size[1].max(0.0)];
            layout.corner_radius_px = layout.corner_radius_px.max(style.corner_radius.max(0.0));
        }
    }

    if !has_semantic_frame {
        if let Some(bounds) = clip_bounds {
            min_x = min_x.min(bounds.x);
            min_y = min_y.min(bounds.y);
            max_x = max_x.max(bounds.x + bounds.width);
            max_y = max_y.max(bounds.y + bounds.height);
        }
        if let Some(bounds) = text_bounds {
            min_x = min_x.min(bounds[0]);
            min_y = min_y.min(bounds[1]);
            max_x = max_x.max(bounds[2]);
            max_y = max_y.max(bounds[3]);
        }
        for primitive in ui_primitives {
            let [x0, y0, x1, y1] = gpu_ui_primitive_bounds(primitive);
            min_x = min_x.min(x0);
            min_y = min_y.min(y0);
            max_x = max_x.max(x1);
            max_y = max_y.max(y1);
        }
    } else {
        let mut update_inset_from_rect = |rect: Rect| {
            let left = (rect.x - min_x).max(0.0);
            let top = (rect.y - min_y).max(0.0);
            let right = (max_x - (rect.x + rect.width)).max(0.0);
            let bottom = (max_y - (rect.y + rect.height)).max(0.0);
            let semantic_inset = left.min(top).min(right).min(bottom);
            if semantic_inset.is_finite() {
                layout.content_inset_px = layout.content_inset_px.max(semantic_inset.max(0.0));
            }
        };

        if let Some(bounds) = clip_bounds {
            update_inset_from_rect(bounds);
        }
        if let Some(bounds) = text_bounds {
            update_inset_from_rect(Rect::new(
                bounds[0],
                bounds[1],
                bounds[2] - bounds[0],
                bounds[3] - bounds[1],
            ));
        }
        for primitive in ui_primitives {
            let [x0, y0, x1, y1] = gpu_ui_primitive_bounds(primitive);
            update_inset_from_rect(Rect::new(x0, y0, x1 - x0, y1 - y0));
        }

        if layout.content_inset_px <= 0.0 {
            layout.content_inset_px =
                (layout.corner_radius_px * 0.16 + layout.outline_width_px * 0.75).max(0.0);
        }
    }

    if !min_x.is_finite()
        || !min_y.is_finite()
        || !max_x.is_finite()
        || !max_y.is_finite()
        || min_x >= max_x
        || min_y >= max_y
    {
        min_x = 100.0;
        min_y = 100.0;
        max_x = 600.0;
        max_y = 600.0;
    }

    layout.center_px = [(min_x + max_x) * 0.5, (min_y + max_y) * 0.5];
    layout.bounds_px = [min_x, min_y, max_x, max_y];
    layout.geometry_mode = 0.0;
    layout
}

fn web_todomvc_physical_layout(scene: &RetainedScene) -> WebUiPhysicalLayout {
    let Some(card) = scene.node_named("card") else {
        return WebUiPhysicalLayout {
            geometry_mode: 1.0,
            fill_color: TODO_PHYSICAL_FILL_COLOR,
            outline_color: TODO_PHYSICAL_OUTLINE_COLOR,
            shadow_color: TODO_PHYSICAL_SHADOW_COLOR,
            ..WebUiPhysicalLayout::default()
        };
    };
    let Some(bounds) = scene.resolved_bounds(card.id) else {
        return WebUiPhysicalLayout {
            geometry_mode: 1.0,
            fill_color: TODO_PHYSICAL_FILL_COLOR,
            outline_color: TODO_PHYSICAL_OUTLINE_COLOR,
            shadow_color: TODO_PHYSICAL_SHADOW_COLOR,
            bounds_px: TODO_PHYSICAL_CARD_BOUNDS,
            ..WebUiPhysicalLayout::default()
        };
    };

    let min_y_up = TODO_PHYSICAL_SCREEN_H - (bounds.y + bounds.height);
    let max_y_up = TODO_PHYSICAL_SCREEN_H - bounds.y;
    WebUiPhysicalLayout {
        center_px: [bounds.x + bounds.width * 0.5, (min_y_up + max_y_up) * 0.5],
        bounds_px: [bounds.x, min_y_up, bounds.x + bounds.width, max_y_up],
        corner_radius_px: 12.0,
        content_inset_px: 0.0,
        elevation_px: 0.0,
        depth_px: 8.0,
        fill_color: TODO_PHYSICAL_FILL_COLOR,
        accent_color: [0.0; 4],
        detail_color: [0.0; 4],
        outline_color: TODO_PHYSICAL_OUTLINE_COLOR,
        outline_width_px: 1.0,
        shadow_color: TODO_PHYSICAL_SHADOW_COLOR,
        shadow_offset_px: [0.0, 14.0],
        shadow_extra_size_px: [12.0, 12.0],
        pixel_to_world: 0.01,
        geometry_mode: 1.0,
    }
}

impl WebUiPhysicalPass {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        label: &str,
        atlas: &VectorFontAtlas,
        text_data: &FixedTextSceneData,
        ui_primitives: &[GpuUiPrimitive],
        theme_uniforms: &ThemeUniforms,
        layout: WebUiPhysicalLayout,
    ) -> Self {
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label} Uniform Buffer")),
            contents: bytemuck::cast_slice(&[WebUiPhysicalUniforms::default()]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let theme_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label} Theme Buffer")),
            contents: bytemuck::cast_slice(std::slice::from_ref(theme_uniforms)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let gpu_font_data = build_font_gpu_data(atlas);
        let storage_buffers = create_storage_buffers(
            device,
            queue,
            &gpu_font_data,
            bytemuck::cast_slice(&text_data.char_instances),
            text_data.char_instances.len().max(1) * std::mem::size_of::<GpuCharInstanceEx>(),
            &text_data.char_grid_cells,
            &text_data.char_grid_indices,
            text_data.char_grid_indices.len().max(1),
            bytemuck::cast_slice(ui_primitives),
            ui_primitives.len().max(1) * std::mem::size_of::<GpuUiPrimitive>(),
            &format!("{label} UI Primitives Buffer"),
        );
        let bind_group_layout = create_bind_group_layout_with_storage(
            device,
            &format!("{label} Bind Group Layout"),
            &[
                (0, wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT),
                (1, wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT),
            ],
            &WEB_UI_PHYSICAL_STORAGE_BINDINGS,
            wgpu::ShaderStages::FRAGMENT,
        );
        let bind_group = create_bind_group_with_storage(
            device,
            &format!("{label} Bind Group"),
            &bind_group_layout,
            &[(0, &uniform_buffer), (1, &theme_buffer)],
            &storage_buffers,
            &WEB_UI_PHYSICAL_STORAGE_BINDINGS,
        );
        let shader_module = retained_ui_physical_shader::create_shader_module_embed_source(device);
        let pipeline = create_fullscreen_pipeline(
            device,
            format,
            &format!("{label} Pipeline"),
            &[&bind_group_layout],
            &shader_module,
        );

        Self {
            pipeline,
            uniform_buffer,
            theme_buffer,
            storage_buffers,
            bind_group_layout,
            bind_group,
            width,
            height,
            char_count: text_data.char_count,
            ui_prim_count: ui_primitives.len() as u32,
            char_grid_params: text_data.char_grid_params,
            char_grid_bounds: text_data.char_grid_bounds,
            layout,
            text_capacity: text_data.char_instances.len().max(1),
            grid_index_capacity: text_data.char_grid_indices.len().max(1),
            primitive_capacity: ui_primitives.len().max(1),
        }
    }

    fn can_fit_scene_data(
        &self,
        text_data: &FixedTextSceneData,
        ui_primitives: &[GpuUiPrimitive],
    ) -> bool {
        text_data.char_instances.len() <= self.text_capacity
            && text_data.char_grid_indices.len() <= self.grid_index_capacity
            && ui_primitives.len() <= self.primitive_capacity
    }

    fn sync_scene_data(
        &mut self,
        queue: &wgpu::Queue,
        text_data: &FixedTextSceneData,
        ui_primitives: &[GpuUiPrimitive],
        theme_uniforms: &ThemeUniforms,
        layout: WebUiPhysicalLayout,
    ) {
        queue.write_buffer(
            &self.storage_buffers.char_instances_buffer,
            0,
            bytemuck::cast_slice(&text_data.char_instances),
        );
        queue.write_buffer(
            &self.storage_buffers.char_grid_cells_buffer,
            0,
            bytemuck::cast_slice(&text_data.char_grid_cells),
        );
        queue.write_buffer(
            &self.storage_buffers.char_grid_indices_buffer,
            0,
            bytemuck::cast_slice(&text_data.char_grid_indices),
        );
        queue.write_buffer(
            &self.storage_buffers.ui_primitives_buffer,
            0,
            bytemuck::cast_slice(ui_primitives),
        );
        queue.write_buffer(
            &self.theme_buffer,
            0,
            bytemuck::cast_slice(std::slice::from_ref(theme_uniforms)),
        );
        self.char_count = text_data.char_count;
        self.ui_prim_count = ui_primitives.len() as u32;
        self.char_grid_params = text_data.char_grid_params;
        self.char_grid_bounds = text_data.char_grid_bounds;
        self.layout = layout;
    }

    fn write_uniforms(
        &self,
        queue: &wgpu::Queue,
        camera: &OrbitalCamera,
        time: f32,
        light_dir_intensity: [f32; 4],
        classic_decal_prim_start: f32,
    ) {
        let mut uniforms = WebUiPhysicalUniforms::default();
        let aspect = self.width as f32 / self.height as f32;
        uniforms.inv_view_proj = camera.inv_view_projection_matrix(aspect).to_cols_array_2d();
        let pos = camera.position();
        uniforms.camera_pos_time = [pos.x, pos.y, pos.z, time];
        uniforms.light_dir_intensity = light_dir_intensity;
        uniforms.render_params = [self.width as f32, self.height as f32, 0.08, 1.0];
        uniforms.text_params = [
            self.char_count as f32,
            self.ui_prim_count as f32,
            1.0,
            classic_decal_prim_start,
        ];
        uniforms.char_grid_params = self.char_grid_params;
        uniforms.char_grid_bounds = self.char_grid_bounds;
        uniforms.layout_params0 = [
            self.layout.center_px[0],
            self.layout.center_px[1],
            self.layout.pixel_to_world,
            self.layout.geometry_mode,
        ];
        uniforms.layout_params1 = [
            self.layout.corner_radius_px,
            self.layout.elevation_px,
            self.layout.depth_px,
            0.0,
        ];
        uniforms.layout_params2 = self.layout.fill_color;
        uniforms.layout_params3 = self.layout.outline_color;
        uniforms.layout_params4 = self.layout.shadow_color;
        uniforms.layout_params5 = [
            self.layout.shadow_offset_px[0],
            self.layout.shadow_offset_px[1],
            0.0,
            0.0,
        ];
        uniforms.layout_params6 = [
            self.layout.shadow_extra_size_px[0],
            self.layout.shadow_extra_size_px[1],
            0.0,
            0.0,
        ];
        uniforms.layout_params7 = [
            self.layout.outline_width_px,
            self.layout.content_inset_px,
            0.0,
            0.0,
        ];
        uniforms.layout_params8 = self.layout.accent_color;
        uniforms.layout_params9 = self.layout.detail_color;
        uniforms.layout_bounds = self.layout.bounds_px;
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    fn rebuild_bind_group(&mut self, device: &wgpu::Device, label: &str) {
        self.bind_group = create_bind_group_with_storage(
            device,
            &format!("{label} Bind Group"),
            &self.bind_group_layout,
            &[(0, &self.uniform_buffer), (1, &self.theme_buffer)],
            &self.storage_buffers,
            &WEB_UI_PHYSICAL_STORAGE_BINDINGS,
        );
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
}

impl WebUiPhysicalRuntimeHost {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        label: &str,
        atlas: &VectorFontAtlas,
        text_data: &FixedTextSceneData,
        ui_primitives: &[GpuUiPrimitive],
        theme_uniforms: &ThemeUniforms,
        layout: WebUiPhysicalLayout,
    ) -> Self {
        let pass = WebUiPhysicalPass::new(
            device,
            queue,
            format,
            width,
            height,
            label,
            atlas,
            text_data,
            ui_primitives,
            theme_uniforms,
            layout,
        );
        Self {
            pass,
            device: device.clone(),
            queue: queue.clone(),
            surface_format: format,
            label: label.to_string(),
            width,
            height,
        }
    }

    fn update_uniforms(
        &self,
        camera: &OrbitalCamera,
        time: f32,
        light_dir_intensity: [f32; 4],
        classic_decal_prim_start: f32,
    ) {
        self.pass.write_uniforms(
            &self.queue,
            camera,
            time,
            light_dir_intensity,
            classic_decal_prim_start,
        );
    }

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.pass.render(render_pass);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.pass.resize(width, height);
    }

    fn sync_or_rebuild(
        &mut self,
        atlas: &VectorFontAtlas,
        text_data: &FixedTextSceneData,
        ui_primitives: &[GpuUiPrimitive],
        theme_uniforms: &ThemeUniforms,
        layout: WebUiPhysicalLayout,
    ) {
        if self.pass.can_fit_scene_data(text_data, ui_primitives) {
            self.pass.sync_scene_data(
                &self.queue,
                text_data,
                ui_primitives,
                theme_uniforms,
                layout,
            );
            return;
        }

        self.pass = WebUiPhysicalPass::new(
            &self.device,
            &self.queue,
            self.surface_format,
            self.width,
            self.height,
            &self.label,
            atlas,
            text_data,
            ui_primitives,
            theme_uniforms,
            layout,
        );
        self.pass.rebuild_bind_group(&self.device, &self.label);
    }
}

impl WebRetainedUiPass {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        label: &str,
        atlas: &VectorFontAtlas,
        text_data: &FixedTextSceneData,
        ui_primitives: &[GpuUiPrimitive],
        text_capacity: usize,
        grid_index_capacity: usize,
        primitive_capacity: usize,
    ) -> Self {
        let uniforms = WebUi2dUniforms {
            screen_params: [
                width as f32,
                height as f32,
                virtual_size_from_bounds(text_data.char_grid_bounds)[0],
                virtual_size_from_bounds(text_data.char_grid_bounds)[1],
            ],
            offset: [0.0, 0.0],
            _pad0: [0.0; 2],
            text_params: [
                text_data.char_count as f32,
                1.0,
                0.0,
                ui_primitives.len() as f32,
            ],
            char_grid_params: text_data.char_grid_params,
            char_grid_bounds: text_data.char_grid_bounds,
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label} Uniform Buffer")),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let gpu_font_data = build_font_gpu_data(atlas);
        let storage_buffers = create_storage_buffers(
            device,
            queue,
            &gpu_font_data,
            bytemuck::cast_slice(&text_data.char_instances),
            text_capacity.max(1) * std::mem::size_of::<GpuCharInstanceEx>(),
            &text_data.char_grid_cells,
            &text_data.char_grid_indices,
            grid_index_capacity.max(1),
            bytemuck::cast_slice(ui_primitives),
            primitive_capacity.max(1) * std::mem::size_of::<GpuUiPrimitive>(),
            &format!("{label} UI Primitives Buffer"),
        );
        let bind_group_layout = create_bind_group_layout_with_storage(
            device,
            &format!("{label} Bind Group Layout"),
            &[(0, wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT)],
            &WEB_UI2D_STORAGE_BINDINGS,
            wgpu::ShaderStages::FRAGMENT,
        );
        let bind_group = create_bind_group_with_storage(
            device,
            &format!("{label} Bind Group"),
            &bind_group_layout,
            &[(0, &uniform_buffer)],
            &storage_buffers,
            &WEB_UI2D_STORAGE_BINDINGS,
        );
        let shader_module = retained_ui2d_shader::create_shader_module_embed_source(device);
        let pipeline = create_fullscreen_pipeline(
            device,
            format,
            &format!("{label} Pipeline"),
            &[&bind_group_layout],
            &shader_module,
        );

        Self {
            pipeline,
            uniform_buffer,
            storage_buffers,
            bind_group,
            width,
            height,
            char_count: text_data.char_count,
            ui_prim_count: ui_primitives.len() as u32,
            char_grid_params: text_data.char_grid_params,
            char_grid_bounds: text_data.char_grid_bounds,
            text_capacity: text_capacity.max(1),
            grid_index_capacity: grid_index_capacity.max(1),
            primitive_capacity: primitive_capacity.max(1),
            view_state: WebUi2dViewState::default(),
        }
    }

    fn can_fit_scene_data(
        &self,
        text_data: &FixedTextSceneData,
        ui_primitives: &[GpuUiPrimitive],
    ) -> bool {
        text_data.char_instances.len() <= self.text_capacity
            && text_data.char_grid_indices.len() <= self.grid_index_capacity
            && ui_primitives.len() <= self.primitive_capacity
    }

    fn set_view_state(&mut self, offset: [f32; 2], scale: f32, rotation: f32) {
        self.view_state = WebUi2dViewState {
            offset,
            scale,
            rotation,
        };
    }

    fn view_state(&self) -> WebUi2dViewState {
        self.view_state
    }

    fn sync_scene_data(
        &mut self,
        queue: &wgpu::Queue,
        text_data: &FixedTextSceneData,
        ui_primitives: &[GpuUiPrimitive],
    ) {
        queue.write_buffer(
            &self.storage_buffers.char_instances_buffer,
            0,
            bytemuck::cast_slice(&text_data.char_instances),
        );
        queue.write_buffer(
            &self.storage_buffers.char_grid_cells_buffer,
            0,
            bytemuck::cast_slice(&text_data.char_grid_cells),
        );
        queue.write_buffer(
            &self.storage_buffers.char_grid_indices_buffer,
            0,
            bytemuck::cast_slice(&text_data.char_grid_indices),
        );
        queue.write_buffer(
            &self.storage_buffers.ui_primitives_buffer,
            0,
            bytemuck::cast_slice(ui_primitives),
        );
        self.char_count = text_data.char_count;
        self.ui_prim_count = ui_primitives.len() as u32;
        self.char_grid_params = text_data.char_grid_params;
        self.char_grid_bounds = text_data.char_grid_bounds;
    }

    fn write_uniforms(&self, queue: &wgpu::Queue) {
        let uniforms = WebUi2dUniforms {
            screen_params: [
                self.width as f32,
                self.height as f32,
                virtual_size_from_bounds(self.char_grid_bounds)[0],
                virtual_size_from_bounds(self.char_grid_bounds)[1],
            ],
            offset: self.view_state.offset,
            _pad0: [0.0; 2],
            text_params: [
                self.char_count as f32,
                self.view_state.scale,
                self.view_state.rotation,
                self.ui_prim_count as f32,
            ],
            char_grid_params: self.char_grid_params,
            char_grid_bounds: self.char_grid_bounds,
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue) {
        self.write_uniforms(queue);
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
}

impl WebRetainedUiRuntimeHost {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        label: &str,
        atlas: &VectorFontAtlas,
        text_data: &FixedTextSceneData,
        ui_primitives: &[GpuUiPrimitive],
    ) -> Self {
        let pass = WebRetainedUiPass::new(
            device,
            queue,
            format,
            width,
            height,
            label,
            atlas,
            text_data,
            ui_primitives,
            text_data.char_instances.len(),
            text_data.char_grid_indices.len(),
            ui_primitives.len(),
        );
        Self {
            pass,
            device: device.clone(),
            queue: queue.clone(),
            surface_format: format,
            label: label.to_string(),
            width,
            height,
        }
    }

    fn set_view_state(&mut self, offset: [f32; 2], scale: f32, rotation: f32) {
        self.pass.set_view_state(offset, scale, rotation);
    }

    fn view_state(&self) -> WebUi2dViewState {
        self.pass.view_state()
    }

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.pass.render(render_pass, &self.queue);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.pass.resize(width, height);
    }

    fn sync_or_rebuild(
        &mut self,
        atlas: &VectorFontAtlas,
        text_data: &FixedTextSceneData,
        ui_primitives: &[GpuUiPrimitive],
    ) {
        let view_state = self.pass.view_state();
        if self.pass.can_fit_scene_data(text_data, ui_primitives) {
            self.pass
                .sync_scene_data(&self.queue, text_data, ui_primitives);
            self.pass
                .set_view_state(view_state.offset, view_state.scale, view_state.rotation);
            return;
        }

        self.pass = WebRetainedUiPass::new(
            &self.device,
            &self.queue,
            self.surface_format,
            self.width,
            self.height,
            &self.label,
            atlas,
            text_data,
            ui_primitives,
            text_data.char_instances.len(),
            text_data.char_grid_indices.len(),
            ui_primitives.len(),
        );
        self.pass
            .set_view_state(view_state.offset, view_state.scale, view_state.rotation);
    }
}

impl WebRetainedUiDemo {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let atlas = load_web_retained_font_atlas();
        let text_colors = web_retained_text_colors();
        let models = ShowcaseSceneModel::default_deck_models(SceneMode::Ui2D);
        let built = models[0].build_fixed_ui2d_scene(
            [width, height],
            atlas.as_ref(),
            &text_colors,
            web_retained_text_space(height as f32),
            web_retained_ui_space(height as f32),
        );
        let (scene_state, text_data, ui_primitives) = web_retained_init_from_built(built);
        let runtime_host = WebRetainedUiRuntimeHost::new(
            device,
            queue,
            format,
            width,
            height,
            "Retained UI Web",
            atlas.as_ref(),
            &text_data,
            &ui_primitives,
        );

        Self {
            models,
            active_scene: 0,
            scene_state,
            runtime_host,
            atlas,
            text_colors,
            width,
            height,
        }
    }

    fn rebuild_active_scene(&mut self) {
        let view_state = self.runtime_host.view_state();
        let built = self.models[self.active_scene].build_fixed_ui2d_scene(
            [self.width, self.height],
            self.atlas.as_ref(),
            &self.text_colors,
            web_retained_text_space(self.height as f32),
            web_retained_ui_space(self.height as f32),
        );
        let (scene_state, text_data, ui_primitives) = web_retained_init_from_built(built);
        self.runtime_host
            .sync_or_rebuild(self.atlas.as_ref(), &text_data, &ui_primitives);
        self.runtime_host
            .set_view_state(view_state.offset, view_state.scale, view_state.rotation);
        self.scene_state = scene_state;
    }

    fn capture_active_model_from_scene(&mut self) {
        self.models[self.active_scene].capture_from_scene(&self.scene_state.scene);
    }

    fn mutate_active_scene(&mut self, action: ShowcaseSceneAction) -> bool {
        let model = self.models[self.active_scene].clone();
        if !model.apply_action(&mut self.scene_state.scene, action) {
            return false;
        }
        self.capture_active_model_from_scene();
        self.rebuild_active_scene();
        true
    }

    fn set_active_scene(&mut self, index: usize) -> bool {
        if index >= self.models.len() || index == self.active_scene {
            return false;
        }
        self.active_scene = index;
        self.rebuild_active_scene();
        true
    }
}

impl WebTodoMvcDemo {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let atlas = load_web_retained_font_atlas();
        let text_colors = todomvc_classic_text_colors();
        let retained_scene = TodoMvcRetainedScene::new(SceneMode::Ui2D, width, height);
        let (text_data, ui_primitives) =
            web_todomvc_scene_buffers(&retained_scene, atlas.as_ref(), &text_colors);
        let runtime_host = WebRetainedUiRuntimeHost::new(
            device,
            queue,
            format,
            width,
            height,
            "TodoMVC Web",
            atlas.as_ref(),
            &text_data,
            &ui_primitives,
        );

        Self {
            retained_scene,
            runtime_host,
            atlas,
            text_colors,
            width,
            height,
        }
    }

    fn rebuild_scene(&mut self) {
        let (text_data, ui_primitives) =
            web_todomvc_scene_buffers(&self.retained_scene, self.atlas.as_ref(), &self.text_colors);
        self.runtime_host
            .sync_or_rebuild(self.atlas.as_ref(), &text_data, &ui_primitives);
    }

    fn mutate_scene(&mut self, mutate: impl FnOnce(&mut TodoMvcRetainedScene) -> bool) -> bool {
        if !mutate(&mut self.retained_scene) {
            return false;
        }
        self.rebuild_scene();
        true
    }

    fn cycle_filter(&mut self) -> bool {
        let next = match self.retained_scene.filter() {
            ListFilter::All => ListFilter::Active,
            ListFilter::Active => ListFilter::Completed,
            ListFilter::Completed => ListFilter::All,
        };
        self.mutate_scene(|scene| scene.set_filter(next))
    }

    fn current_scroll_offset(&self) -> f32 {
        scroll_offset_for_node(self.retained_scene.scene(), "list_scroll").unwrap_or(0.0)
    }
}

impl WebRetainedUiPhysicalDemo {
    fn build_scene(
        model: &ShowcaseSceneModel,
        width: u32,
        height: u32,
        atlas: &VectorFontAtlas,
        theme_state: &UiPhysicalThemeState,
    ) -> (
        FixedUi2dSceneState,
        FixedTextSceneData,
        Vec<GpuUiPrimitive>,
        WebUiPhysicalLayout,
    ) {
        let text_colors = tune_generic_ui_physical_text_colors(theme_state.text_colors());
        let built = FixedUi2dSceneModelBuilder::build_fixed_ui2d_scene(
            model,
            [width, height],
            atlas,
            &text_colors,
            web_retained_text_space(height as f32),
            web_retained_ui_space(height as f32),
        );
        let (scene_state, text_data, ui_primitives) = web_retained_init_from_built(built);
        let layout = web_generic_physical_layout(&scene_state.scene, &text_data, &ui_primitives);
        (scene_state, text_data, ui_primitives, layout)
    }

    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let atlas = load_web_retained_font_atlas();
        let theme_state = UiPhysicalThemeState::new(ThemeId::Professional, true);
        let models = ShowcaseSceneModel::default_deck_models(SceneMode::UiPhysical);
        let (scene_state, text_data, ui_primitives, layout) =
            Self::build_scene(&models[0], width, height, atlas.as_ref(), &theme_state);
        let runtime_host = WebUiPhysicalRuntimeHost::new(
            device,
            queue,
            format,
            width,
            height,
            "Retained UI Physical Web",
            atlas.as_ref(),
            &text_data,
            &ui_primitives,
            &theme_state.theme_uniforms(),
            layout,
        );

        Self {
            models,
            active_scene: 0,
            scene_state,
            runtime_host,
            atlas,
            theme_state,
            width,
            height,
        }
    }

    fn rebuild_active_scene(&mut self) {
        let (scene_state, text_data, ui_primitives, layout) = Self::build_scene(
            &self.models[self.active_scene],
            self.width,
            self.height,
            self.atlas.as_ref(),
            &self.theme_state,
        );
        self.runtime_host.sync_or_rebuild(
            self.atlas.as_ref(),
            &text_data,
            &ui_primitives,
            &self.theme_state.theme_uniforms(),
            layout,
        );
        self.scene_state = scene_state;
    }

    fn capture_active_model_from_scene(&mut self) {
        self.models[self.active_scene].capture_from_scene(&self.scene_state.scene);
    }

    fn mutate_active_scene(&mut self, action: ShowcaseSceneAction) -> bool {
        let model = self.models[self.active_scene].clone();
        if !model.apply_action(&mut self.scene_state.scene, action) {
            return false;
        }
        self.capture_active_model_from_scene();
        self.rebuild_active_scene();
        true
    }

    fn set_active_scene(&mut self, index: usize) -> bool {
        if index >= self.models.len() || index == self.active_scene {
            return false;
        }
        self.active_scene = index;
        self.rebuild_active_scene();
        true
    }
}

impl WebTextPhysicalDemo {
    fn build_scene(
        model: &WrappedTextSceneModel,
        width: u32,
        height: u32,
        atlas: &VectorFontAtlas,
        theme_state: &UiPhysicalThemeState,
    ) -> (
        FixedUi2dSceneState,
        FixedTextSceneData,
        Vec<GpuUiPrimitive>,
        WebUiPhysicalLayout,
    ) {
        let text_colors = tune_generic_ui_physical_text_colors(theme_state.text_colors());
        let built = FixedUi2dSceneModelBuilder::build_fixed_ui2d_scene(
            model,
            [width, height],
            atlas,
            &text_colors,
            web_retained_text_space(height as f32),
            web_retained_ui_space(height as f32),
        );
        let (scene_state, text_data, ui_primitives) = web_retained_init_from_built(built);
        let layout = web_generic_physical_layout(&scene_state.scene, &text_data, &ui_primitives);
        (scene_state, text_data, ui_primitives, layout)
    }

    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let atlas = load_web_retained_font_atlas();
        let theme_state = UiPhysicalThemeState::new(ThemeId::Professional, true);
        let model = text_physical_scene_model();
        let (scene_state, text_data, ui_primitives, layout) =
            Self::build_scene(&model, width, height, atlas.as_ref(), &theme_state);
        let runtime_host = WebUiPhysicalRuntimeHost::new(
            device,
            queue,
            format,
            width,
            height,
            "Text Physical Web",
            atlas.as_ref(),
            &text_data,
            &ui_primitives,
            &theme_state.theme_uniforms(),
            layout,
        );

        Self {
            model,
            scene_state,
            runtime_host,
            atlas,
            theme_state,
            width,
            height,
        }
    }

    fn rebuild_scene(&mut self) {
        let (scene_state, text_data, ui_primitives, layout) = Self::build_scene(
            &self.model,
            self.width,
            self.height,
            self.atlas.as_ref(),
            &self.theme_state,
        );
        self.runtime_host.sync_or_rebuild(
            self.atlas.as_ref(),
            &text_data,
            &ui_primitives,
            &self.theme_state.theme_uniforms(),
            layout,
        );
        self.scene_state = scene_state;
    }

    fn mutate_scene(
        &mut self,
        mutate: impl FnOnce(&WrappedTextSceneModel, &mut RetainedScene) -> bool,
    ) -> bool {
        if !mutate(&self.model, &mut self.scene_state.scene) {
            return false;
        }
        self.model.capture_from_scene(&self.scene_state.scene);
        self.rebuild_scene();
        true
    }
}

impl WebTodoMvc3DDemo {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let atlas = load_web_retained_font_atlas();
        let theme_state = UiPhysicalThemeState::new(ThemeId::Classic2D, false);
        let retained_scene = TodoMvcRetainedScene::new(SceneMode::UiPhysical, width, height);
        let text_colors = theme_state.text_colors();
        let (text_data, ui_primitives) =
            web_todomvc_scene_buffers(&retained_scene, atlas.as_ref(), &text_colors);
        let layout = web_todomvc_physical_layout(retained_scene.scene());
        let runtime_host = WebUiPhysicalRuntimeHost::new(
            device,
            queue,
            format,
            width,
            height,
            "TodoMVC 3D Web",
            atlas.as_ref(),
            &text_data,
            &ui_primitives,
            &theme_state.theme_uniforms(),
            layout,
        );

        Self {
            retained_scene,
            runtime_host,
            atlas,
            theme_state,
            width,
            height,
        }
    }

    fn rebuild_scene(&mut self) {
        let text_colors = self.theme_state.text_colors();
        let (text_data, ui_primitives) =
            web_todomvc_scene_buffers(&self.retained_scene, self.atlas.as_ref(), &text_colors);
        let layout = web_todomvc_physical_layout(self.retained_scene.scene());
        self.runtime_host.sync_or_rebuild(
            self.atlas.as_ref(),
            &text_data,
            &ui_primitives,
            &self.theme_state.theme_uniforms(),
            layout,
        );
    }

    fn mutate_scene(&mut self, mutate: impl FnOnce(&mut TodoMvcRetainedScene) -> bool) -> bool {
        if !mutate(&mut self.retained_scene) {
            return false;
        }
        self.rebuild_scene();
        true
    }
}

impl WebDemo for WebRetainedUiDemo {
    fn name(&self) -> &'static str {
        "Retained UI"
    }

    fn id(&self) -> DemoId {
        DemoId::RetainedUi
    }

    fn demo_type(&self) -> DemoType {
        DemoType::Ui2D
    }

    fn keybindings(&self) -> &'static [(&'static str, &'static str)] {
        RETAINED_UI_KEYBINDINGS_WEB
    }

    fn camera_config(&self) -> (glam::Vec3, glam::Vec3) {
        (glam::Vec3::ZERO, glam::Vec3::ZERO)
    }

    fn update(&mut self, _dt: f32) {}

    fn update_uniforms(&self, _queue: &wgpu::Queue, _camera: &OrbitalCamera, _time: f32) {}

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.runtime_host.render(render_pass);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.runtime_host.resize(width, height);
        self.rebuild_active_scene();
    }

    fn set_ui2d_view_state(&mut self, offset: [f32; 2], scale: f32, rotation: f32) {
        self.runtime_host.set_view_state(offset, scale, rotation);
    }

    fn handle_key_pressed(&mut self, code: &str) -> bool {
        match code {
            "KeyY" => self.set_active_scene((self.active_scene + 1) % self.models.len()),
            "KeyO" => self.mutate_active_scene(ShowcaseSceneAction::TogglePrimaryState),
            _ => false,
        }
    }

    fn handle_key_held(&mut self, code: &str) -> bool {
        match code {
            "KeyU" => self.mutate_active_scene(ShowcaseSceneAction::AdjustPrimaryScroll(
                -WEB_RETAINED_SCROLL_STEP,
            )),
            "KeyJ" => self.mutate_active_scene(ShowcaseSceneAction::AdjustPrimaryScroll(
                WEB_RETAINED_SCROLL_STEP,
            )),
            _ => false,
        }
    }

    fn set_named_scroll(&mut self, name: &str, offset_y: f32) -> bool {
        let model = self.models[self.active_scene].clone();
        if !model.set_named_scroll_offset(&mut self.scene_state.scene, name, offset_y) {
            return false;
        }
        self.capture_active_model_from_scene();
        self.rebuild_active_scene();
        true
    }
}

impl WebDemo for WebTodoMvcDemo {
    fn name(&self) -> &'static str {
        "TodoMVC"
    }

    fn id(&self) -> DemoId {
        DemoId::TodoMvc
    }

    fn demo_type(&self) -> DemoType {
        DemoType::Ui2D
    }

    fn keybindings(&self) -> &'static [(&'static str, &'static str)] {
        TODOMVC_KEYBINDINGS_WEB
    }

    fn camera_config(&self) -> (glam::Vec3, glam::Vec3) {
        (glam::Vec3::ZERO, glam::Vec3::ZERO)
    }

    fn update(&mut self, _dt: f32) {}

    fn update_uniforms(&self, _queue: &wgpu::Queue, _camera: &OrbitalCamera, _time: f32) {}

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.runtime_host.render(render_pass);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.retained_scene.set_viewport_size(width, height);
        self.runtime_host.resize(width, height);
        self.rebuild_scene();
    }

    fn set_ui2d_view_state(&mut self, offset: [f32; 2], scale: f32, rotation: f32) {
        self.runtime_host.set_view_state(offset, scale, rotation);
    }

    fn handle_key_pressed(&mut self, code: &str) -> bool {
        match code {
            "KeyY" => self.cycle_filter(),
            "KeyO" => self.mutate_scene(|scene| scene.toggle_item(0)),
            _ => false,
        }
    }

    fn handle_key_held(&mut self, code: &str) -> bool {
        let offset = match code {
            "KeyU" => self.current_scroll_offset() - WEB_RETAINED_SCROLL_STEP,
            "KeyJ" => self.current_scroll_offset() + WEB_RETAINED_SCROLL_STEP,
            _ => return false,
        };
        self.set_list_scroll_offset(offset)
    }

    fn toggle_list_item(&mut self, index: u32) -> bool {
        self.mutate_scene(|scene| scene.toggle_item(index as usize))
    }

    fn set_list_item_completed(&mut self, index: u32, completed: bool) -> bool {
        self.mutate_scene(|scene| scene.set_item_completed(index as usize, completed))
    }

    fn set_list_item_label(&mut self, index: u32, label: &str) -> bool {
        self.mutate_scene(|scene| scene.set_item_label(index as usize, label))
    }

    fn set_list_filter(&mut self, filter: &str) -> Option<String> {
        let filter = ListFilter::from_str(filter)?;
        let _ = self.mutate_scene(|scene| scene.set_filter(filter));
        Some(filter.name().to_string())
    }

    fn set_list_scroll_offset(&mut self, offset_y: f32) -> bool {
        let changed =
            set_named_scroll_offset(self.retained_scene.scene_mut(), "list_scroll", offset_y);
        if changed {
            self.rebuild_scene();
        }
        changed
    }

    fn set_named_scroll(&mut self, name: &str, offset_y: f32) -> bool {
        let changed = set_named_scroll_offset(self.retained_scene.scene_mut(), name, offset_y);
        if changed {
            self.rebuild_scene();
        }
        changed
    }
}

impl WebDemo for WebRetainedUiPhysicalDemo {
    fn name(&self) -> &'static str {
        "Retained UI Physical"
    }

    fn id(&self) -> DemoId {
        DemoId::RetainedUiPhysical
    }

    fn demo_type(&self) -> DemoType {
        DemoType::UiPhysical
    }

    fn keybindings(&self) -> &'static [(&'static str, &'static str)] {
        RETAINED_UI_PHYSICAL_KEYBINDINGS_WEB
    }

    fn camera_config(&self) -> (glam::Vec3, glam::Vec3) {
        let preset = ui_physical_card_camera_preset(SHOWCASE_PHYSICAL_CARD_SIZE);
        (preset.fallback_offset, glam::Vec3::ZERO)
    }

    fn ui_physical_camera_preset(&self) -> Option<UiPhysicalCameraPreset> {
        Some(ui_physical_card_camera_preset(SHOWCASE_PHYSICAL_CARD_SIZE))
    }

    fn update(&mut self, _dt: f32) {}

    fn update_uniforms(&self, _queue: &wgpu::Queue, camera: &OrbitalCamera, time: f32) {
        self.runtime_host.update_uniforms(
            camera,
            time,
            self.theme_state.light_dir_intensity(),
            0.0,
        );
    }

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.runtime_host.render(render_pass);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.runtime_host.resize(width, height);
        self.rebuild_active_scene();
    }

    fn handle_key_pressed(&mut self, code: &str) -> bool {
        match code {
            "KeyY" => self.set_active_scene((self.active_scene + 1) % self.models.len()),
            "KeyO" => self.mutate_active_scene(ShowcaseSceneAction::TogglePrimaryState),
            "KeyN" => {
                self.theme_state.cycle_theme();
                self.rebuild_active_scene();
                true
            }
            "KeyM" => {
                self.theme_state.toggle_dark_mode();
                self.rebuild_active_scene();
                true
            }
            _ => false,
        }
    }

    fn handle_key_held(&mut self, code: &str) -> bool {
        match code {
            "KeyU" => self.mutate_active_scene(ShowcaseSceneAction::AdjustPrimaryScroll(
                -WEB_RETAINED_SCROLL_STEP,
            )),
            "KeyJ" => self.mutate_active_scene(ShowcaseSceneAction::AdjustPrimaryScroll(
                WEB_RETAINED_SCROLL_STEP,
            )),
            _ => false,
        }
    }

    fn set_named_scroll(&mut self, name: &str, offset_y: f32) -> bool {
        let model = self.models[self.active_scene].clone();
        if !model.set_named_scroll_offset(&mut self.scene_state.scene, name, offset_y) {
            return false;
        }
        self.capture_active_model_from_scene();
        self.rebuild_active_scene();
        true
    }

    fn set_named_theme(
        &mut self,
        theme: &str,
        dark_mode: Option<bool>,
    ) -> Option<(&'static str, bool)> {
        let result = self.theme_state.set_named_theme(theme, dark_mode)?;
        self.rebuild_active_scene();
        Some(result)
    }

    fn named_theme_options(&self) -> &'static [&'static str] {
        PHYSICAL_THEME_OPTIONS
    }
}

impl WebDemo for WebTextPhysicalDemo {
    fn name(&self) -> &'static str {
        "Text Physical"
    }

    fn id(&self) -> DemoId {
        DemoId::TextPhysical
    }

    fn demo_type(&self) -> DemoType {
        DemoType::UiPhysical
    }

    fn keybindings(&self) -> &'static [(&'static str, &'static str)] {
        TEXT_PHYSICAL_KEYBINDINGS_WEB
    }

    fn camera_config(&self) -> (glam::Vec3, glam::Vec3) {
        let preset = self
            .ui_physical_camera_preset()
            .unwrap_or_else(|| ui_physical_card_camera_preset(TEXT_PHYSICAL_FRAME_SIZE));
        (preset.fallback_offset, glam::Vec3::ZERO)
    }

    fn ui_physical_camera_preset(&self) -> Option<UiPhysicalCameraPreset> {
        let mut preset = ui_physical_card_camera_preset(TEXT_PHYSICAL_FRAME_SIZE);
        preset.fallback_offset = glam::Vec3::new(0.0, 6.4, 7.6);
        preset.min_distance = 4.6;
        preset.max_distance = 9.4;
        preset.clamp_x = 5.4;
        preset.max_height = 8.4;
        preset.clamp_z = 7.8;
        Some(preset)
    }

    fn update(&mut self, _dt: f32) {}

    fn update_uniforms(&self, _queue: &wgpu::Queue, camera: &OrbitalCamera, time: f32) {
        self.runtime_host.update_uniforms(
            camera,
            time,
            self.theme_state.light_dir_intensity(),
            0.0,
        );
    }

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.runtime_host.render(render_pass);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.runtime_host.resize(width, height);
        self.rebuild_scene();
    }

    fn handle_key_pressed(&mut self, code: &str) -> bool {
        match code {
            "KeyY" => self.mutate_scene(|model, scene| model.toggle_heading_emphasis(scene)),
            "KeyN" => {
                self.theme_state.cycle_theme();
                self.rebuild_scene();
                true
            }
            "KeyM" => {
                self.theme_state.toggle_dark_mode();
                self.rebuild_scene();
                true
            }
            _ => false,
        }
    }

    fn handle_key_held(&mut self, code: &str) -> bool {
        match code {
            "KeyU" => self.mutate_scene(|model, scene| {
                model.adjust_scroll(scene, -TEXT_PHYSICAL_SCROLL_STEP)
            }),
            "KeyJ" => self
                .mutate_scene(|model, scene| model.adjust_scroll(scene, TEXT_PHYSICAL_SCROLL_STEP)),
            _ => false,
        }
    }

    fn set_named_scroll(&mut self, name: &str, offset_y: f32) -> bool {
        let changed = set_named_scroll_offset(&mut self.scene_state.scene, name, offset_y);
        if changed {
            self.model.capture_from_scene(&self.scene_state.scene);
            self.rebuild_scene();
        }
        changed
    }

    fn set_named_theme(
        &mut self,
        theme: &str,
        dark_mode: Option<bool>,
    ) -> Option<(&'static str, bool)> {
        let result = self.theme_state.set_named_theme(theme, dark_mode)?;
        self.rebuild_scene();
        Some(result)
    }

    fn named_theme_options(&self) -> &'static [&'static str] {
        PHYSICAL_THEME_OPTIONS
    }
}

impl WebDemo for WebTodoMvc3DDemo {
    fn name(&self) -> &'static str {
        "TodoMVC 3D"
    }

    fn id(&self) -> DemoId {
        DemoId::TodoMvc3D
    }

    fn demo_type(&self) -> DemoType {
        DemoType::UiPhysical
    }

    fn keybindings(&self) -> &'static [(&'static str, &'static str)] {
        TODOMVC_3D_KEYBINDINGS_WEB
    }

    fn camera_config(&self) -> (glam::Vec3, glam::Vec3) {
        (glam::Vec3::new(0.0, 6.5, 7.0), glam::Vec3::ZERO)
    }

    fn ui_physical_camera_preset(&self) -> Option<UiPhysicalCameraPreset> {
        Some(UiPhysicalCameraPreset {
            fallback_offset: glam::Vec3::new(0.0, 6.5, 7.0),
            min_distance: 4.0,
            max_distance: 14.0,
            min_elevation: -0.9,
            max_elevation: 0.35,
            clamp_x: 7.0,
            min_height: 1.5,
            max_height: 11.0,
            clamp_z: 10.0,
        })
    }

    fn update(&mut self, _dt: f32) {}

    fn update_uniforms(&self, _queue: &wgpu::Queue, camera: &OrbitalCamera, time: f32) {
        self.runtime_host.update_uniforms(
            camera,
            time,
            self.theme_state.light_dir_intensity(),
            TODO_CLASSIC_DECAL_PRIM_START,
        );
    }

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.runtime_host.render(render_pass);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.retained_scene.set_viewport_size(width, height);
        self.runtime_host.resize(width, height);
        self.rebuild_scene();
    }

    fn handle_key_pressed(&mut self, code: &str) -> bool {
        match code {
            "KeyN" => {
                self.theme_state.cycle_theme();
                self.rebuild_scene();
                true
            }
            "KeyM" => {
                self.theme_state.toggle_dark_mode();
                self.rebuild_scene();
                true
            }
            _ => false,
        }
    }

    fn toggle_list_item(&mut self, index: u32) -> bool {
        self.mutate_scene(|scene| scene.toggle_item(index as usize))
    }

    fn set_list_item_completed(&mut self, index: u32, completed: bool) -> bool {
        self.mutate_scene(|scene| scene.set_item_completed(index as usize, completed))
    }

    fn set_list_item_label(&mut self, index: u32, label: &str) -> bool {
        self.mutate_scene(|scene| scene.set_item_label(index as usize, label))
    }

    fn set_list_filter(&mut self, filter: &str) -> Option<String> {
        let filter = ListFilter::from_str(filter)?;
        let _ = self.mutate_scene(|scene| scene.set_filter(filter));
        Some(filter.name().to_string())
    }

    fn set_list_scroll_offset(&mut self, offset_y: f32) -> bool {
        let changed =
            set_named_scroll_offset(self.retained_scene.scene_mut(), "list_scroll", offset_y);
        if changed {
            self.rebuild_scene();
        }
        changed
    }

    fn set_named_scroll(&mut self, name: &str, offset_y: f32) -> bool {
        let changed = set_named_scroll_offset(self.retained_scene.scene_mut(), name, offset_y);
        if changed {
            self.rebuild_scene();
        }
        changed
    }

    fn set_named_theme(
        &mut self,
        theme: &str,
        dark_mode: Option<bool>,
    ) -> Option<(&'static str, bool)> {
        let result = self.theme_state.set_named_theme(theme, dark_mode)?;
        self.rebuild_scene();
        Some(result)
    }

    fn named_theme_options(&self) -> &'static [&'static str] {
        PHYSICAL_THEME_OPTIONS
    }
}

// ============== DEMO FACTORY ==============

fn create_web_demo(
    id: DemoId,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> Box<dyn WebDemo> {
    match id {
        DemoId::Empty => Box::new(PlaceholderDemo::new(
            device,
            format,
            "Empty",
            DemoId::Empty,
            DemoType::World3D,
            KEYBINDINGS_3D_WEB,
            glam::Vec3::new(0.0, 0.0, 5.0),
            glam::Vec3::ZERO,
        )),
        DemoId::Objects => Box::new(SimpleWorld3dDemo::new(
            device,
            format,
            "Objects Web Demo",
            sdf_raymarch::create_shader_module_embed_source,
            "Objects",
            DemoId::Objects,
            glam::Vec3::new(0.0, 1.5, 5.0),
            glam::Vec3::new(0.0, 0.5, 0.0),
        )),
        DemoId::Spheres => Box::new(SimpleWorld3dDemo::new(
            device,
            format,
            "Spheres Web Demo",
            sdf_spheres::create_shader_module_embed_source,
            "Spheres",
            DemoId::Spheres,
            glam::Vec3::new(0.0, 2.0, 6.0),
            glam::Vec3::new(0.0, 0.0, 0.0),
        )),
        DemoId::Towers => Box::new(SimpleWorld3dDemo::new(
            device,
            format,
            "Towers Web Demo",
            sdf_towers::create_shader_module_embed_source,
            "Towers",
            DemoId::Towers,
            glam::Vec3::new(0.0, 3.0, 8.0),
            glam::Vec3::new(0.0, 1.0, 0.0),
        )),
        DemoId::Text2D => Box::new(Text2DDemo::new(device, format)),
        DemoId::Clay => Box::new(SimpleWorld3dDemo::new(
            device,
            format,
            "Clay Web Demo",
            sdf_clay_vector::create_shader_module_embed_source,
            "Clay Tablet",
            DemoId::Clay,
            glam::Vec3::new(0.0, 0.0, 2.0),
            glam::Vec3::ZERO,
        )),
        DemoId::TextShadow => Box::new(SimpleWorld3dDemo::new(
            device,
            format,
            "Text Shadow Web Demo",
            sdf_text_shadow_vector::create_shader_module_embed_source,
            "Text Shadow",
            DemoId::TextShadow,
            glam::Vec3::new(0.0, 1.5, 4.0),
            glam::Vec3::new(0.0, 0.5, 0.0),
        )),
        DemoId::TodoMvc => Box::new(WebTodoMvcDemo::new(device, queue, format, width, height)),
        DemoId::TodoMvc3D => Box::new(WebTodoMvc3DDemo::new(device, queue, format, width, height)),
        DemoId::RetainedUi => {
            Box::new(WebRetainedUiDemo::new(device, queue, format, width, height))
        }
        DemoId::RetainedUiPhysical => Box::new(WebRetainedUiPhysicalDemo::new(
            device, queue, format, width, height,
        )),
        DemoId::TextPhysical => Box::new(WebTextPhysicalDemo::new(
            device, queue, format, width, height,
        )),
    }
}

// ============== WEB RENDERER ==============

struct WebRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    surface_format: wgpu::TextureFormat,
    canvas: web_sys::HtmlCanvasElement,

    // Current demo
    current_demo: Box<dyn WebDemo>,
    current_demo_id: DemoId,

    // Camera
    camera: OrbitalCamera,

    // Input
    input: WebInputHandler,

    // 2D demo state
    text2d_offset: [f32; 2],
    text2d_scale: f32,
    text2d_rotation: f32,

    // Timing
    start_time: f64,
    last_frame_time: f64,

    // Control server integration
    control_state: Option<SharedWebControlState>,
    control_client: Option<WebWsClient>,
}

impl WebRenderer {
    fn ui_physical_camera_preset(&self) -> UiPhysicalCameraPreset {
        self.current_demo
            .ui_physical_camera_preset()
            .unwrap_or_default()
    }

    fn enforce_ui_physical_camera_policy(&mut self) {
        if self.current_demo.demo_type() != DemoType::UiPhysical {
            return;
        }

        let preset = self.ui_physical_camera_preset();
        self.camera.target = ui_physical_camera_target(self.current_demo.as_ref());
        self.camera.distance = self
            .camera
            .distance
            .clamp(preset.min_distance.max(0.1), preset.max_distance);
        self.camera.elevation = self
            .camera
            .elevation
            .clamp(preset.min_elevation, preset.max_elevation);
    }

    async fn new(
        canvas: web_sys::HtmlCanvasElement,
        initial_demo: DemoId,
    ) -> Result<Self, JsValue> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|e| JsValue::from_str(&format!("Failed to create surface: {}", e)))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("No suitable GPU adapter found: {:?}", e)))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("RayBox Web Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("Failed to create device: {}", e)))?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: WIDTH,
            height: HEIGHT,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Create initial demo
        let current_demo = create_web_demo(
            initial_demo,
            &device,
            &queue,
            surface_format,
            config.width,
            config.height,
        );

        // Setup camera for this demo
        let (pos, target) = current_demo.camera_config();
        let mut camera = OrbitalCamera::default();
        camera.target = target;
        camera.distance = (pos - target).length();
        if camera.distance > 0.0 {
            let dir = (pos - target) / camera.distance;
            camera.azimuth = dir.x.atan2(dir.z);
            camera.elevation = dir.y.asin();
        }

        let start_time = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);

        let mut renderer = Self {
            surface,
            device,
            queue,
            config,
            surface_format,
            canvas,
            current_demo,
            current_demo_id: initial_demo,
            camera,
            input: WebInputHandler::new(),
            text2d_offset: [0.0, 0.0],
            text2d_scale: 1.0,
            text2d_rotation: 0.0,
            start_time,
            last_frame_time: start_time,
            control_state: None,
            control_client: None,
        };
        renderer.current_demo.set_ui2d_view_state(
            renderer.text2d_offset,
            renderer.text2d_scale,
            renderer.text2d_rotation,
        );
        renderer.enforce_ui_physical_camera_policy();
        Ok(renderer)
    }

    /// Connect to control server
    fn connect_control(&mut self, url: &str) {
        let state = web_control::new_shared_state();
        match WebWsClient::connect(url, state.clone()) {
            Ok(client) => {
                self.control_state = Some(state);
                self.control_client = Some(client);
                log::info!("Connected to control server at {}", url);
            }
            Err(e) => {
                log::warn!("Failed to connect to control server: {:?}", e);
            }
        }
    }

    fn capture_screenshot_response(
        &self,
        id: u64,
        center_crop: Option<[u32; 2]>,
    ) -> web_control::WebResponse {
        let source_width = self.canvas.width().max(1);
        let source_height = self.canvas.height().max(1);

        let (capture_canvas, out_width, out_height) =
            if let Some([crop_width, crop_height]) = center_crop {
                let crop_width = crop_width.min(source_width).max(1);
                let crop_height = crop_height.min(source_height).max(1);
                let crop_x = (source_width.saturating_sub(crop_width)) / 2;
                let crop_y = (source_height.saturating_sub(crop_height)) / 2;

                let Some(document) = web_sys::window().and_then(|window| window.document()) else {
                    return error_response(
                        id,
                        "ScreenshotFailed",
                        "No document available for screenshot",
                    );
                };
                let Ok(element) = document.create_element("canvas") else {
                    return error_response(
                        id,
                        "ScreenshotFailed",
                        "Failed to create screenshot canvas",
                    );
                };
                let Ok(canvas) = element.dyn_into::<web_sys::HtmlCanvasElement>() else {
                    return error_response(
                        id,
                        "ScreenshotFailed",
                        "Failed to create screenshot canvas",
                    );
                };
                canvas.set_width(crop_width);
                canvas.set_height(crop_height);

                let Ok(Some(context)) = canvas.get_context("2d") else {
                    return error_response(
                        id,
                        "ScreenshotFailed",
                        "Failed to acquire screenshot context",
                    );
                };
                let Ok(context) = context.dyn_into::<web_sys::CanvasRenderingContext2d>() else {
                    return error_response(
                        id,
                        "ScreenshotFailed",
                        "Failed to acquire screenshot context",
                    );
                };
                if context
                    .draw_image_with_html_canvas_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                        &self.canvas,
                        crop_x as f64,
                        crop_y as f64,
                        crop_width as f64,
                        crop_height as f64,
                        0.0,
                        0.0,
                        crop_width as f64,
                        crop_height as f64,
                    )
                    .is_err()
                {
                    return error_response(id, "ScreenshotFailed", "Failed to crop screenshot");
                }

                (canvas, crop_width, crop_height)
            } else {
                (self.canvas.clone(), source_width, source_height)
            };

        let Ok(data_url) = capture_canvas.to_data_url_with_type("image/png") else {
            return error_response(id, "ScreenshotFailed", "Failed to encode screenshot");
        };
        let Some(base64_data) = data_url.split_once(',').map(|(_, payload)| payload) else {
            return error_response(id, "ScreenshotFailed", "Unexpected screenshot payload");
        };

        screenshot_response(id, base64_data, out_width, out_height)
    }

    /// Process pending control commands
    fn process_control_commands(&mut self) {
        let state = match &self.control_state {
            Some(s) => s.clone(),
            None => return,
        };

        // Process all pending commands
        while let Some((id, cmd)) = state.borrow_mut().pop_command() {
            let response = self.handle_control_command(id, cmd);
            state.borrow_mut().push_response(response);
        }

        // Flush responses to server
        if let Some(ref client) = self.control_client {
            client.flush_responses();
        }
    }

    /// Handle a single control command
    fn handle_control_command(&mut self, id: u64, command: WebCommand) -> web_control::WebResponse {
        match command {
            WebCommand::SwitchDemo(demo_id) => {
                if let Some(new_id) = DemoId::from_u8(demo_id) {
                    self.switch_demo(new_id);
                    success_response(
                        id,
                        Some(&format!(
                            r#"{{"demo":{},"name":"{}"}}"#,
                            demo_id,
                            new_id.name()
                        )),
                    )
                } else {
                    error_response(
                        id,
                        "InvalidDemoId",
                        &format!("Invalid demo ID: {}", demo_id),
                    )
                }
            }
            WebCommand::SetCamera {
                position,
                yaw,
                pitch,
            } => {
                if let Some(pos) = position {
                    let target = glam::Vec3::from_array(pos);
                    self.camera.distance = (target - self.camera.target).length().max(0.1);
                }
                if let Some(y) = yaw {
                    self.camera.azimuth = y;
                }
                if let Some(p) = pitch {
                    self.camera.elevation = p;
                }
                self.enforce_ui_physical_camera_policy();
                success_response(id, None)
            }
            WebCommand::SetTheme { theme, dark_mode } => {
                let options = self.current_demo.named_theme_options();
                if options.is_empty() {
                    error_response(
                        id,
                        "InvalidCommand",
                        "Current demo does not support named themes",
                    )
                } else if let Some((theme_name, dark_mode)) =
                    self.current_demo.set_named_theme(&theme, dark_mode)
                {
                    success_response(
                        id,
                        Some(&format!(
                            r#"{{"theme":"{}","dark_mode":{}}}"#,
                            theme_name, dark_mode
                        )),
                    )
                } else {
                    error_response(
                        id,
                        "InvalidTheme",
                        &format!("Invalid theme: {}. Valid: {}", theme, options.join(", ")),
                    )
                }
            }
            WebCommand::SetListItem {
                index,
                completed,
                label,
                toggle,
            } => {
                let changed = if toggle {
                    self.current_demo.toggle_list_item(index)
                } else {
                    let mut changed = false;
                    if let Some(completed) = completed {
                        changed |= self.current_demo.set_list_item_completed(index, completed);
                    }
                    if let Some(label) = label {
                        changed |= self.current_demo.set_list_item_label(index, &label);
                    }
                    changed
                };
                success_response(
                    id,
                    Some(&format!(r#"{{"index":{},"changed":{}}}"#, index, changed)),
                )
            }
            WebCommand::SetListFilter { filter } => {
                if let Some(filter_name) = self.current_demo.set_list_filter(&filter) {
                    success_response(
                        id,
                        Some(&format!(r#"{{"filter":"{}","changed":true}}"#, filter_name)),
                    )
                } else {
                    error_response(
                        id,
                        "InvalidCommand",
                        &format!("Invalid list filter: {}", filter),
                    )
                }
            }
            WebCommand::SetListScroll { offset_y } => {
                let changed = self.current_demo.set_list_scroll_offset(offset_y);
                success_response(
                    id,
                    Some(&format!(
                        r#"{{"offset_y":{},"changed":{}}}"#,
                        offset_y, changed
                    )),
                )
            }
            WebCommand::SetNamedScroll { name, offset_y } => {
                let changed = self.current_demo.set_named_scroll(&name, offset_y);
                success_response(
                    id,
                    Some(&format!(
                        r#"{{"changed":{},"name":"{}","offset_y":{}}}"#,
                        changed, name, offset_y
                    )),
                )
            }
            WebCommand::Screenshot { center_crop } => {
                self.capture_screenshot_response(id, center_crop)
            }
            WebCommand::GetStatus => {
                let pos = self.camera.position();
                let overlay_mode = if self.input.overlay_visible() {
                    "app"
                } else {
                    "off"
                };
                status_response(
                    id,
                    self.current_demo_id as u8,
                    self.current_demo.name(),
                    demo_family_name(self.current_demo.demo_type()),
                    [pos.x, pos.y, pos.z],
                    self.input.fps(),
                    overlay_mode,
                    self.input.show_keybindings,
                )
            }
            WebCommand::ToggleOverlay(mode) => {
                match mode.as_str() {
                    "off" => self.input.overlay_mode = crate::demo_core::OverlayMode::Off,
                    "app" => self.input.overlay_mode = crate::demo_core::OverlayMode::App,
                    "full" => self.input.overlay_mode = crate::demo_core::OverlayMode::App, // Web doesn't have full system stats
                    _ => {}
                }
                success_response(id, None)
            }
            WebCommand::PressKey(key) => {
                match key.as_str() {
                    "k" | "K" => self.input.toggle_keybindings(),
                    "f" | "F" => self.input.toggle_overlay_app(),
                    "g" | "G" => self.input.toggle_overlay_full(),
                    _ => {}
                }
                success_response(id, None)
            }
            WebCommand::Ping => pong_response(id),
            WebCommand::Reload => {
                // Signal that reload was requested - JavaScript will handle the actual reload
                if let Some(ref state) = self.control_state {
                    state.borrow_mut().request_reload();
                }
                success_response(id, Some(r#"{"message":"reload_requested"}"#))
            }
        }
    }

    fn switch_demo(&mut self, new_id: DemoId) {
        if new_id == self.current_demo_id {
            return;
        }

        // Create new demo
        let mut new_demo = create_web_demo(
            new_id,
            &self.device,
            &self.queue,
            self.surface_format,
            self.config.width,
            self.config.height,
        );

        // Setup camera for this demo
        let (pos, target) = new_demo.camera_config();
        self.camera = OrbitalCamera::default();
        self.camera.target = target;
        self.camera.distance = (pos - target).length().max(0.1);
        if self.camera.distance > 0.0 {
            let dir = (pos - target) / self.camera.distance;
            self.camera.azimuth = dir.x.atan2(dir.z);
            self.camera.elevation = dir.y.asin();
        }
        self.enforce_ui_physical_camera_policy();

        // Reset 2D controls
        self.text2d_offset = [0.0, 0.0];
        self.text2d_scale = 1.0;
        self.text2d_rotation = 0.0;
        new_demo.set_ui2d_view_state(self.text2d_offset, self.text2d_scale, self.text2d_rotation);

        self.current_demo = new_demo;
        self.current_demo_id = new_id;

        log::info!(
            "Switched to demo {}: {}",
            new_id as u8,
            self.current_demo.name()
        );
    }

    fn update(&mut self) {
        let now = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);
        let dt = ((now - self.last_frame_time) / 1000.0) as f32;
        self.last_frame_time = now;

        self.input.update_frame_time(dt);

        // Process control commands
        self.process_control_commands();

        // Handle demo switching for demos reachable from digit keys (0-9)
        for i in 0..10 {
            let key = format!("Digit{}", i);
            if self.input.is_key_pressed(&key) {
                if let Some(id) = DemoId::from_u8(i) {
                    self.switch_demo(id);
                }
            }
        }

        // Handle toggle keys
        if self.input.is_key_pressed("KeyK") {
            self.input.pressed_keys.remove("KeyK");
            self.input.toggle_keybindings();
        }
        if self.input.is_key_pressed("KeyF") {
            self.input.pressed_keys.remove("KeyF");
            self.input.toggle_overlay_app();
        }
        if self.input.is_key_pressed("KeyG") {
            self.input.pressed_keys.remove("KeyG");
            self.input.toggle_overlay_full();
        }
        for code in ["KeyY", "KeyO", "KeyN", "KeyM"] {
            if self.input.is_key_pressed(code) && self.current_demo.handle_key_pressed(code) {
                self.input.pressed_keys.remove(code);
            }
        }
        for code in ["KeyU", "KeyJ"] {
            let _ = self
                .input
                .is_key_pressed(code)
                .then(|| self.current_demo.handle_key_held(code));
        }

        match self.current_demo.demo_type() {
            DemoType::UiPhysical | DemoType::World3D => {
                // 3D camera controls
                const ROTATION_SPEED: f32 = 0.03;
                const ZOOM_SPEED: f32 = 0.1;

                if self.input.is_key_pressed("KeyA") {
                    self.camera.rotate_horizontal(-ROTATION_SPEED);
                }
                if self.input.is_key_pressed("KeyD") {
                    self.camera.rotate_horizontal(ROTATION_SPEED);
                }
                if self.input.is_key_pressed("KeyW") {
                    self.camera.zoom(ZOOM_SPEED);
                }
                if self.input.is_key_pressed("KeyS") {
                    self.camera.zoom(-ZOOM_SPEED);
                }
                if self.input.is_key_pressed("KeyQ") {
                    self.camera.rotate_vertical(ROTATION_SPEED);
                }
                if self.input.is_key_pressed("KeyE") {
                    self.camera.rotate_vertical(-ROTATION_SPEED);
                }
                self.enforce_ui_physical_camera_policy();
            }
            DemoType::Ui2D => {
                // 2D controls
                const PAN_SPEED: f32 = 5.0;
                const ZOOM_SPEED: f32 = 0.02;
                const ROT_SPEED: f32 = 0.05;

                if self.input.is_key_pressed("KeyA") {
                    self.text2d_offset[0] -= PAN_SPEED / self.text2d_scale;
                }
                if self.input.is_key_pressed("KeyD") {
                    self.text2d_offset[0] += PAN_SPEED / self.text2d_scale;
                }
                if self.input.is_key_pressed("KeyW") {
                    self.text2d_offset[1] += PAN_SPEED / self.text2d_scale;
                }
                if self.input.is_key_pressed("KeyS") {
                    self.text2d_offset[1] -= PAN_SPEED / self.text2d_scale;
                }
                if self.input.is_key_pressed("ArrowUp") {
                    self.text2d_scale *= 1.0 + ZOOM_SPEED;
                }
                if self.input.is_key_pressed("ArrowDown") {
                    self.text2d_scale *= 1.0 - ZOOM_SPEED;
                }
                if self.input.is_key_pressed("KeyQ") {
                    self.text2d_rotation += ROT_SPEED;
                }
                if self.input.is_key_pressed("KeyE") {
                    self.text2d_rotation -= ROT_SPEED;
                }
                self.text2d_scale = self.text2d_scale.clamp(0.1, 10.0);
                self.current_demo.set_ui2d_view_state(
                    self.text2d_offset,
                    self.text2d_scale,
                    self.text2d_rotation,
                );
            }
        }

        self.current_demo.update(dt);
    }

    fn render(&self) -> Result<(), wgpu::SurfaceError> {
        let time = {
            let now = web_sys::window()
                .and_then(|w| w.performance())
                .map(|p| p.now())
                .unwrap_or(0.0);
            ((now - self.start_time) / 1000.0) as f32
        };

        // Update uniforms
        self.current_demo
            .update_uniforms(&self.queue, &self.camera, time);

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Web Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Web Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.current_demo.render(&mut render_pass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    fn key_down(&mut self, code: String) {
        self.input.key_down(&code);
    }

    fn key_up(&mut self, code: String) {
        self.input.key_up(&code);
    }

    /// Extract current state for hot-reload preservation
    fn extract_state(&self) -> WebReloadableState {
        let now = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);
        let elapsed = ((now - self.start_time) / 1000.0) as f32;

        let overlay_mode = match self.input.overlay_mode {
            OverlayMode::Off => "off",
            OverlayMode::App => "app",
            OverlayMode::Full => "full",
        };

        WebReloadableState {
            current_demo: self.current_demo_id as u8,
            camera_distance: self.camera.distance,
            camera_azimuth: self.camera.azimuth,
            camera_elevation: self.camera.elevation,
            camera_target: [
                self.camera.target.x,
                self.camera.target.y,
                self.camera.target.z,
            ],
            overlay_mode: overlay_mode.to_string(),
            show_keybindings: self.input.show_keybindings,
            text2d_offset: self.text2d_offset,
            text2d_scale: self.text2d_scale,
            text2d_rotation: self.text2d_rotation,
            time_offset: elapsed,
        }
    }

    /// Apply saved state after hot-reload
    fn apply_state(&mut self, state: WebReloadableState) {
        // Switch demo if different
        if let Some(demo_id) = DemoId::from_u8(state.current_demo) {
            if demo_id != self.current_demo_id {
                self.switch_demo(demo_id);
            }
        }

        // Restore camera state
        self.camera.distance = state.camera_distance;
        self.camera.azimuth = state.camera_azimuth;
        self.camera.elevation = state.camera_elevation;
        self.camera.target = glam::Vec3::from_array(state.camera_target);

        // Restore overlay state
        self.input.overlay_mode = match state.overlay_mode.as_str() {
            "app" => OverlayMode::App,
            "full" => OverlayMode::Full,
            _ => OverlayMode::Off,
        };
        self.input.show_keybindings = state.show_keybindings;

        // Restore 2D state
        self.text2d_offset = state.text2d_offset;
        self.text2d_scale = state.text2d_scale;
        self.text2d_rotation = state.text2d_rotation;
        self.current_demo.set_ui2d_view_state(
            self.text2d_offset,
            self.text2d_scale,
            self.text2d_rotation,
        );

        // Adjust start time to maintain animation continuity
        if let Some(window) = web_sys::window() {
            if let Some(perf) = window.performance() {
                let now = perf.now();
                self.start_time = now - (state.time_offset as f64 * 1000.0);
            }
        }

        log::info!(
            "Applied saved state: demo={}, camera distance={:.2}",
            state.current_demo,
            state.camera_distance
        );
    }

    /// Check if reload was requested and handle it
    fn check_reload_requested(&self) -> bool {
        if let Some(ref state) = self.control_state {
            state.borrow_mut().take_reload_request()
        } else {
            false
        }
    }

    fn get_overlay_text(&self) -> String {
        let mut lines = Vec::new();

        // Demo info
        lines.push(format!(
            "Demo {}: {}",
            self.current_demo_id as u8,
            self.current_demo.name()
        ));

        // Stats (if enabled)
        if self.input.overlay_visible() {
            lines.push(self.input.format_stats());
        }

        // Keybindings (if enabled)
        if self.input.show_keybindings {
            lines.push(String::new()); // blank line
            lines.push("=== Keybindings ===".to_string());
            for (key, desc) in self.current_demo.keybindings() {
                lines.push(format!("{}: {}", key, desc));
            }
            lines.push("---".to_string());
            for (key, desc) in KEYBINDINGS_COMMON {
                lines.push(format!("{}: {}", key, desc));
            }
        }

        lines.join("\n")
    }
}

fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    web_sys::window()
        .unwrap()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .unwrap();
}

/// Check if control mode is enabled via URL parameter (?control=1)
fn is_control_mode_enabled() -> bool {
    if let Some(window) = web_sys::window() {
        if let Ok(search) = window.location().search() {
            if let Some(params) = search.strip_prefix('?') {
                for param in params.split('&') {
                    if param == "control=1" || param == "control=true" {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Get control server URL from URL parameter or use default
fn get_control_url() -> String {
    if let Some(window) = web_sys::window() {
        if let Ok(search) = window.location().search() {
            if let Some(params) = search.strip_prefix('?') {
                for param in params.split('&') {
                    if let Some(url) = param.strip_prefix("control_url=") {
                        return url.to_string();
                    }
                }
            }
        }
    }
    "ws://127.0.0.1:9300".to_string()
}

/// Parse initial demo from URL parameter (?demo=X)
fn get_initial_demo_from_url() -> DemoId {
    if let Some(window) = web_sys::window() {
        if let Ok(search) = window.location().search() {
            if let Some(params) = search.strip_prefix('?') {
                for param in params.split('&') {
                    if let Some(value) = param.strip_prefix("demo=") {
                        if let Ok(id) = value.parse::<u8>() {
                            if let Some(demo_id) = DemoId::from_u8(id) {
                                return demo_id;
                            }
                        }
                    }
                }
            }
        }
    }
    DemoId::Objects // Default to Objects demo
}

#[wasm_bindgen(start)]
pub async fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).unwrap();

    let initial_demo = get_initial_demo_from_url();
    log::info!(
        "Initializing raybox web renderer with demo {}: {}",
        initial_demo as u8,
        initial_demo.name()
    );

    let window = web_sys::window().ok_or("No window found")?;
    let document = window.document().ok_or("No document found")?;
    let canvas = document
        .get_element_by_id("canvas")
        .ok_or("No canvas element found")?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    let mut renderer = WebRenderer::new(canvas, initial_demo).await?;
    log::info!("Web renderer initialized successfully");
    log::info!(
        "Controls: 0-6 switch demos, A/D rotate, W/S zoom, Q/E tilt, K keybindings, F stats"
    );

    // Apply saved state if available (for hot-reload)
    SAVED_STATE.with(|s| {
        if let Some(state) = s.borrow_mut().take() {
            log::info!("Restoring saved state after hot-reload");
            renderer.apply_state(state);
        }
    });

    // Connect to control server if enabled
    if is_control_mode_enabled() {
        let control_url = get_control_url();
        log::info!("Control mode enabled, connecting to {}", control_url);
        renderer.connect_control(&control_url);
    }

    let renderer = Rc::new(RefCell::new(renderer));

    // Set up keyboard event listeners
    let renderer_keydown = renderer.clone();
    let keydown_closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
        renderer_keydown.borrow_mut().key_down(event.code());
    }) as Box<dyn FnMut(_)>);

    let renderer_keyup = renderer.clone();
    let keyup_closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
        renderer_keyup.borrow_mut().key_up(event.code());
    }) as Box<dyn FnMut(_)>);

    window.add_event_listener_with_callback("keydown", keydown_closure.as_ref().unchecked_ref())?;
    window.add_event_listener_with_callback("keyup", keyup_closure.as_ref().unchecked_ref())?;

    // Keep closures alive
    keydown_closure.forget();
    keyup_closure.forget();

    // Update overlay text element
    let overlay_element = document.get_element_by_id("overlay");

    // Store renderer reference for hot-reload state extraction
    RENDERER_REF.with(|r| {
        *r.borrow_mut() = Some(renderer.clone());
    });

    // Animation loop
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();

    let renderer_clone = renderer.clone();
    *g.borrow_mut() = Some(Closure::new(move || {
        renderer_clone.borrow_mut().update();
        if let Err(e) = renderer_clone.borrow().render() {
            log::error!("Render error: {:?}", e);
        }

        // Update overlay text
        if let Some(ref elem) = overlay_element {
            let text = renderer_clone.borrow().get_overlay_text();
            elem.set_text_content(Some(&text));
        }

        request_animation_frame(f.borrow().as_ref().unwrap());
    }));

    request_animation_frame(g.borrow().as_ref().unwrap());

    Ok(())
}

/// Start with a specific demo (callable from JavaScript)
#[wasm_bindgen]
pub async fn start_with_demo(demo_id: u8) -> Result<(), JsValue> {
    let initial_demo = DemoId::from_u8(demo_id).unwrap_or(DemoId::Objects);

    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).ok(); // ok() because it might already be initialized

    log::info!(
        "Starting raybox with demo {}: {}",
        initial_demo as u8,
        initial_demo.name()
    );

    let window = web_sys::window().ok_or("No window found")?;
    let document = window.document().ok_or("No document found")?;
    let canvas = document
        .get_element_by_id("canvas")
        .ok_or("No canvas element found")?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    let mut renderer = WebRenderer::new(canvas, initial_demo).await?;

    // Apply saved state if available (for hot-reload)
    SAVED_STATE.with(|s| {
        if let Some(state) = s.borrow_mut().take() {
            log::info!("Restoring saved state after hot-reload");
            renderer.apply_state(state);
        }
    });

    // Connect to control server if enabled
    if is_control_mode_enabled() {
        let control_url = get_control_url();
        log::info!("Control mode enabled, connecting to {}", control_url);
        renderer.connect_control(&control_url);
    }

    let renderer = Rc::new(RefCell::new(renderer));

    // Store renderer reference for hot-reload state extraction
    RENDERER_REF.with(|r| {
        *r.borrow_mut() = Some(renderer.clone());
    });

    // Set up keyboard event listeners
    let renderer_keydown = renderer.clone();
    let keydown_closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
        renderer_keydown.borrow_mut().key_down(event.code());
    }) as Box<dyn FnMut(_)>);

    let renderer_keyup = renderer.clone();
    let keyup_closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
        renderer_keyup.borrow_mut().key_up(event.code());
    }) as Box<dyn FnMut(_)>);

    window.add_event_listener_with_callback("keydown", keydown_closure.as_ref().unchecked_ref())?;
    window.add_event_listener_with_callback("keyup", keyup_closure.as_ref().unchecked_ref())?;

    keydown_closure.forget();
    keyup_closure.forget();

    // Update overlay text element
    let overlay_element = document.get_element_by_id("overlay");

    // Animation loop
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();

    let renderer_clone = renderer.clone();
    *g.borrow_mut() = Some(Closure::new(move || {
        renderer_clone.borrow_mut().update();
        if let Err(e) = renderer_clone.borrow().render() {
            log::error!("Render error: {:?}", e);
        }

        if let Some(ref elem) = overlay_element {
            let text = renderer_clone.borrow().get_overlay_text();
            elem.set_text_content(Some(&text));
        }

        request_animation_frame(f.borrow().as_ref().unwrap());
    }));

    request_animation_frame(g.borrow().as_ref().unwrap());

    Ok(())
}

// ============== HOT-RELOAD WASM_BINDGEN EXPORTS ==============

/// Serialize current renderer state for hot-reload preservation
/// Returns JSON string that can be passed to restore_state after WASM reload
#[wasm_bindgen]
pub fn serialize_state() -> String {
    // The state is stored globally when save_state_for_reload is called
    SAVED_STATE.with(|s| {
        if let Some(ref state) = *s.borrow() {
            serde_json::to_string(state).unwrap_or_else(|_| "{}".to_string())
        } else {
            "{}".to_string()
        }
    })
}

/// Restore renderer state after hot-reload
/// Called from JavaScript with the JSON from serialize_state
#[wasm_bindgen]
pub fn restore_state(json: &str) {
    if json.is_empty() || json == "{}" {
        return;
    }

    match serde_json::from_str::<WebReloadableState>(json) {
        Ok(state) => {
            SAVED_STATE.with(|s| {
                *s.borrow_mut() = Some(state);
            });
            log::info!("Saved state will be applied on next start");
        }
        Err(e) => {
            log::warn!("Failed to parse state JSON: {}", e);
        }
    }
}

/// Check if there's saved state to restore
#[wasm_bindgen]
pub fn has_saved_state() -> bool {
    SAVED_STATE.with(|s| s.borrow().is_some())
}

/// Clear saved state after it's been applied
#[wasm_bindgen]
pub fn clear_saved_state() {
    SAVED_STATE.with(|s| {
        *s.borrow_mut() = None;
    });
}

/// Save current state for hot-reload (called from JavaScript before reload)
/// Returns JSON string of the saved state
#[wasm_bindgen]
pub fn save_state_for_reload() -> String {
    RENDERER_REF.with(|r| {
        if let Some(ref renderer_rc) = *r.borrow() {
            let renderer = renderer_rc.borrow();
            let state = renderer.extract_state();
            SAVED_STATE.with(|s| {
                *s.borrow_mut() = Some(state.clone());
            });
            serde_json::to_string(&state).unwrap_or_else(|_| "{}".to_string())
        } else {
            log::warn!("No renderer available for state extraction");
            "{}".to_string()
        }
    })
}

/// Clean up renderer before hot-reload (called from JavaScript)
#[wasm_bindgen]
pub fn cleanup_for_reload() {
    // Save state first
    save_state_for_reload();

    // Clear the renderer reference to allow GPU resources to be freed
    RENDERER_REF.with(|r| {
        *r.borrow_mut() = None;
    });

    log::info!("Cleaned up for hot-reload");
}
