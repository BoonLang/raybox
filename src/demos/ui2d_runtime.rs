use super::DemoContext;
use crate::demo_core::{ListCommandTarget, ListFilter, NamedScrollTarget};
use crate::demos::gpu_runtime_common::{
    build_font_gpu_data, create_bind_group_layout_with_storage, create_bind_group_with_storage,
    create_fullscreen_pipeline, create_storage_buffers, UiStorageBuffers, VectorFontGpuData,
};
use crate::retained::fixed_scene::FixedUi2dSceneUpdate;
use crate::retained::fixed_scene::{
    BuiltFixedUi2dScene, FixedUi2dSceneInit, FixedUi2dSceneModelBuilder,
    FixedUi2dSceneModelCapture, FixedUi2dSceneState,
};
use crate::retained::samples::{SampleSceneAction, SampleSceneDeckTarget, SampleSceneModel};
use crate::retained::showcase::{ShowcaseSceneAction, ShowcaseSceneDeckTarget, ShowcaseSceneModel};
use crate::retained::text::{
    FixedTextGridCache, FixedTextRunLayout, FixedTextRuntimeUpdate, FixedTextSceneData,
    FixedTextScenePatch, FixedTextSceneState, GpuCharInstanceEx,
};
use crate::retained::text::{TextColors, TextRenderSpace};
use crate::retained::ui::UiRenderSpace;
use crate::retained::ui::{GpuUiPatch, GpuUiPrimitive, GpuUiRuntimeUpdate};
use crate::retained::{NamedScrollSceneModel, RetainedScene, SceneMode};
use crate::text::{CharGridCell, VectorFont, VectorFontAtlas};
use crate::ui2d_shader_bindings as retained_ui2d_shader;
use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use std::cell::Cell;
use std::fs;
use std::sync::Arc;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Ui2DUniforms {
    pub screen_params: [f32; 4],
    pub offset: [f32; 2],
    pub _pad0: [f32; 2],
    pub text_params: [f32; 4],
    pub char_grid_params: [f32; 4],
    pub char_grid_bounds: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ui2DViewState {
    pub offset: [f32; 2],
    pub scale: f32,
    pub rotation: f32,
}

impl Default for Ui2DViewState {
    fn default() -> Self {
        Self {
            offset: [0.0, 0.0],
            scale: 1.0,
            rotation: 0.0,
        }
    }
}

impl Ui2DViewState {
    pub fn apply_controls(
        &mut self,
        offset_delta: [f32; 2],
        scale_factor: f32,
        rotation_delta: f32,
    ) -> bool {
        let mut changed = false;

        if offset_delta != [0.0, 0.0] {
            self.offset[0] += offset_delta[0] / self.scale;
            self.offset[1] += offset_delta[1] / self.scale;
            changed = true;
        }

        if scale_factor != 1.0 {
            self.scale *= scale_factor;
            self.scale = self.scale.clamp(0.1, 10.0);
            changed = true;
        }

        if rotation_delta != 0.0 {
            self.rotation += rotation_delta;
            changed = true;
        }

        changed
    }

    pub fn reset_rotation(&mut self) -> bool {
        if self.rotation == 0.0 {
            return false;
        }
        self.rotation = 0.0;
        true
    }

    pub fn reset_all(&mut self) -> bool {
        if self.offset == [0.0, 0.0] && self.scale == 1.0 && self.rotation == 0.0 {
            return false;
        }
        *self = Self::default();
        true
    }
}

const UI2D_STORAGE_BINDINGS: [u32; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

pub struct Ui2dPassHost {
    pass: UiTextAndPrimitivesPass,
}

pub struct Ui2dSceneInitData {
    pub text_data: FixedTextSceneData,
    pub ui_primitives: Vec<GpuUiPrimitive>,
    pub text_capacity: usize,
    pub grid_index_capacity: usize,
    pub primitive_capacity: usize,
}

pub type Ui2dRuntimeTextUpdate = FixedTextRuntimeUpdate;
pub type Ui2dRuntimeUiUpdate = GpuUiRuntimeUpdate;

pub struct Ui2dRuntimeUpdate {
    pub text: Option<Ui2dRuntimeTextUpdate>,
    pub ui: Option<Ui2dRuntimeUiUpdate>,
}

fn virtual_size_from_bounds(bounds: [f32; 4]) -> [f32; 2] {
    let width = (bounds[2] - bounds[0]).max(1.0);
    let height = (bounds[3] - bounds[1]).max(1.0);
    [width, height]
}

impl Ui2dRuntimeUpdate {
    pub fn needs_full_rebuild(&self) -> bool {
        matches!(self.text, Some(Ui2dRuntimeTextUpdate::Full(_)))
            || matches!(self.ui, Some(Ui2dRuntimeUiUpdate::Full(_)))
    }
}

impl From<FixedUi2dSceneInit> for Ui2dSceneInitData {
    fn from(init: FixedUi2dSceneInit) -> Self {
        Self {
            text_capacity: init.text_data.char_instances.len(),
            grid_index_capacity: init.text_data.char_grid_indices.len(),
            primitive_capacity: init.ui_data.primitive_count,
            text_data: init.text_data,
            ui_primitives: init.ui_data.primitives,
        }
    }
}

impl From<FixedUi2dSceneUpdate> for Ui2dRuntimeUpdate {
    fn from(update: FixedUi2dSceneUpdate) -> Self {
        match update {
            FixedUi2dSceneUpdate::Full { text_data, ui_data } => Self {
                text: Some(Ui2dRuntimeTextUpdate::Full(text_data)),
                ui: Some(Ui2dRuntimeUiUpdate::Full(ui_data)),
            },
            FixedUi2dSceneUpdate::Partial {
                ui_patches,
                text_patch,
            } => Self {
                text: text_patch.map(Ui2dRuntimeTextUpdate::Partial),
                ui: (!ui_patches.is_empty()).then_some(Ui2dRuntimeUiUpdate::Partial(ui_patches)),
            },
        }
    }
}

pub trait Ui2dSceneState {
    fn atlas(&self) -> &VectorFontAtlas;
    fn text_state(&self) -> &FixedTextSceneState;
    fn build_ui2d_init_data(&self) -> Ui2dSceneInitData;
    fn take_ui2d_runtime_update(&mut self, colors: &TextColors) -> Option<Ui2dRuntimeUpdate>;
    fn mark_view_transform_dirty(&mut self);
    fn set_viewport_size(&mut self, width: u32, height: u32);
    fn scene(&self) -> &RetainedScene;
    fn scene_mut(&mut self) -> &mut RetainedScene;
}

pub struct FixedUi2dSceneHost {
    pub scene_state: FixedUi2dSceneState,
    runtime_host: Ui2dRuntimeHost,
    atlas: Arc<VectorFontAtlas>,
    text_colors: TextColors,
    text_render_space: TextRenderSpace,
    ui_render_space: UiRenderSpace,
}

pub struct ModeledFixedUi2dSceneHost<M> {
    model: M,
    host: FixedUi2dSceneHost,
    viewport_size: [u32; 2],
    text_colors: TextColors,
    needs_rebuild: bool,
}

pub struct Ui2dSceneDeck<H> {
    scenes: Vec<H>,
    active_scene: usize,
}

pub type ModeledFixedUi2dSceneDeck<M> = Ui2dSceneDeck<ModeledFixedUi2dSceneHost<M>>;
pub type ShowcaseUi2dDeck = ModeledFixedUi2dSceneDeck<ShowcaseSceneModel>;

pub struct StateBackedUi2dHost<S> {
    state: S,
    runtime_host: Ui2dRuntimeHost,
    text_colors: TextColors,
}

pub type StateBackedUi2dSceneDeck<S> = Ui2dSceneDeck<StateBackedUi2dHost<S>>;

struct Ui2dRuntimeHost {
    runtime: Ui2dPassHost,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_format: wgpu::TextureFormat,
    label: String,
    width: u32,
    height: u32,
}

pub struct UiPrimitivesOnlyPass {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    ui_prim_count: u32,
    view_state: Ui2DViewState,
    uniforms_dirty: Cell<bool>,
}

pub trait Ui2dDeckHost {
    fn view_state(&self) -> Ui2DViewState;
    fn set_view_state(&mut self, view_state: Ui2DViewState);
    fn needs_redraw(&self) -> bool;
    fn apply_view_controls(
        &mut self,
        offset_delta: [f32; 2],
        scale_factor: f32,
        rotation_delta: f32,
    );
    fn reset_rotation(&mut self);
    fn reset_all(&mut self);
    fn prepare_frame(&mut self);
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue);
    fn resize(&mut self, width: u32, height: u32);

    fn resize_lazy(&mut self, width: u32, height: u32) {
        self.resize(width, height);
    }

    fn ensure_ready(&mut self) {}
}

pub struct UiTextAndPrimitivesPass {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    char_instances_buffer: wgpu::Buffer,
    char_grid_cells_buffer: wgpu::Buffer,
    char_grid_indices_buffer: wgpu::Buffer,
    ui_primitives_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    char_count: u32,
    ui_prim_count: u32,
    char_grid_params: [f32; 4],
    char_grid_bounds: [f32; 4],
    text_capacity: usize,
    grid_cell_capacity: usize,
    grid_index_capacity: usize,
    primitive_capacity: usize,
    view_state: Ui2DViewState,
    uniforms_dirty: Cell<bool>,
}

impl UiPrimitivesOnlyPass {
    pub fn view_state(&self) -> Ui2DViewState {
        self.view_state
    }

    pub fn needs_redraw(&self) -> bool {
        self.uniforms_dirty.get()
    }

    pub fn new(
        ctx: &DemoContext,
        label_prefix: &str,
        ui_primitives: &[GpuUiPrimitive],
        primitive_capacity: usize,
    ) -> Self {
        let uniforms = Ui2DUniforms {
            screen_params: [
                ctx.width as f32,
                ctx.height as f32,
                ctx.width as f32,
                ctx.height as f32,
            ],
            offset: [0.0, 0.0],
            _pad0: [0.0; 2],
            text_params: [0.0, 1.0, 0.0, ui_primitives.len() as f32],
            char_grid_params: [0.0; 4],
            char_grid_bounds: [0.0; 4],
        };

        let uniform_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{label_prefix} Uniform Buffer")),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let storage_buffers = create_storage_buffers(
            ctx.device,
            ctx.queue,
            &VectorFontGpuData {
                grid_cells: Vec::new(),
                curve_indices: Vec::new(),
                curves: Vec::new(),
                glyph_data: Vec::new(),
            },
            bytemuck::cast_slice(&[GpuCharInstanceEx {
                pos_and_char: [0.0; 4],
                color_flags: [0.0; 4],
            }]),
            std::mem::size_of::<GpuCharInstanceEx>(),
            &[CharGridCell {
                offset: 0,
                count: 0,
            }],
            &[0u32],
            1,
            bytemuck::cast_slice(ui_primitives),
            primitive_capacity * std::mem::size_of::<GpuUiPrimitive>(),
            &format!("{label_prefix} Primitives Buffer"),
        );

        let bind_group_layout = create_bind_group_layout_with_storage(
            ctx.device,
            &format!("{label_prefix} Bind Group Layout"),
            &[(0, wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT)],
            &UI2D_STORAGE_BINDINGS,
            wgpu::ShaderStages::FRAGMENT,
        );

        let bind_group = create_bind_group_with_storage(
            ctx.device,
            &format!("{label_prefix} Bind Group"),
            &bind_group_layout,
            &[(0, &uniform_buffer)],
            &storage_buffers,
            &UI2D_STORAGE_BINDINGS,
        );

        let shader_module = retained_ui2d_shader::create_shader_module_embed_source(ctx.device);

        let pipeline = create_fullscreen_pipeline(
            ctx.device,
            ctx.surface_format,
            &format!("{label_prefix} Pipeline"),
            &[&bind_group_layout],
            &shader_module,
        );

        Self {
            pipeline,
            uniform_buffer,
            bind_group,
            width: ctx.width,
            height: ctx.height,
            ui_prim_count: ui_primitives.len() as u32,
            view_state: Ui2DViewState::default(),
            uniforms_dirty: Cell::new(true),
        }
    }

    fn update_uniforms(&self, queue: &wgpu::Queue) {
        if !self.uniforms_dirty.get() {
            return;
        }

        let uniforms = Ui2DUniforms {
            screen_params: [
                self.width as f32,
                self.height as f32,
                self.width as f32,
                self.height as f32,
            ],
            offset: self.view_state.offset,
            _pad0: [0.0; 2],
            text_params: [
                0.0,
                self.view_state.scale,
                self.view_state.rotation,
                self.ui_prim_count as f32,
            ],
            char_grid_params: [0.0; 4],
            char_grid_bounds: [0.0; 4],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
        self.uniforms_dirty.set(false);
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue) {
        self.update_uniforms(queue);
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.uniforms_dirty.set(true);
    }

    pub fn apply_view_controls(
        &mut self,
        offset_delta: [f32; 2],
        scale_factor: f32,
        rotation_delta: f32,
    ) {
        if self
            .view_state
            .apply_controls(offset_delta, scale_factor, rotation_delta)
        {
            self.uniforms_dirty.set(true);
        }
    }

    pub fn set_view_state(&mut self, view_state: Ui2DViewState) {
        if self.view_state != view_state {
            self.view_state = view_state;
            self.uniforms_dirty.set(true);
        }
    }

    pub fn reset_rotation(&mut self) {
        if self.view_state.reset_rotation() {
            self.uniforms_dirty.set(true);
        }
    }

    pub fn reset_all(&mut self) {
        if self.view_state.reset_all() {
            self.uniforms_dirty.set(true);
        }
    }
}

impl Ui2dPassHost {
    pub fn new_text_and_primitives(
        ctx: &DemoContext,
        label_prefix: &str,
        atlas: &VectorFontAtlas,
        text_data: &FixedTextSceneData,
        ui_primitives: &[GpuUiPrimitive],
        text_capacity: usize,
        grid_index_capacity: usize,
        primitive_capacity: usize,
    ) -> Self {
        Self {
            pass: UiTextAndPrimitivesPass::new(
                ctx,
                label_prefix,
                atlas,
                text_data,
                ui_primitives,
                text_capacity,
                grid_index_capacity,
                primitive_capacity,
            ),
        }
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue) {
        self.pass.render(render_pass, queue);
    }

    pub fn needs_redraw(&self) -> bool {
        self.pass.needs_redraw()
    }

    pub fn view_state(&self) -> Ui2DViewState {
        self.pass.view_state()
    }

    pub fn can_fit_scene_data(
        &self,
        text_data: &FixedTextSceneData,
        ui_primitives: &[GpuUiPrimitive],
    ) -> bool {
        self.pass.can_fit_scene_data(text_data, ui_primitives)
    }

    pub fn sync_fixed_scene_update(
        &mut self,
        queue: &wgpu::Queue,
        update: FixedUi2dSceneUpdate,
        text_state: &FixedTextSceneState,
    ) {
        self.sync_runtime_update(queue, update.into(), text_state);
    }

    pub fn sync_text_scene_data(&mut self, queue: &wgpu::Queue, text_data: &FixedTextSceneData) {
        self.pass.sync_text_scene_data(queue, text_data);
    }

    pub fn sync_scene_data(
        &mut self,
        queue: &wgpu::Queue,
        text_data: &FixedTextSceneData,
        ui_primitives: &[GpuUiPrimitive],
    ) {
        self.pass.sync_scene_data(queue, text_data, ui_primitives);
    }

    pub fn sync_ui_scene_data(&mut self, queue: &wgpu::Queue, ui_primitives: &[GpuUiPrimitive]) {
        self.pass.sync_ui_scene_data(queue, ui_primitives);
    }

    pub fn sync_text_patch(
        &mut self,
        queue: &wgpu::Queue,
        text_patch: &FixedTextScenePatch,
        text_state: &FixedTextSceneState,
    ) {
        self.pass.sync_text_patch(queue, text_patch, text_state);
    }

    pub fn sync_ui_patches(&mut self, queue: &wgpu::Queue, patches: &[GpuUiPatch]) {
        self.pass.sync_ui_patches(queue, patches);
    }

    pub fn sync_runtime_update(
        &mut self,
        queue: &wgpu::Queue,
        update: Ui2dRuntimeUpdate,
        text_state: &FixedTextSceneState,
    ) {
        if let Some(text) = update.text {
            match text {
                Ui2dRuntimeTextUpdate::Full(text_data) => {
                    self.sync_text_scene_data(queue, &text_data);
                }
                Ui2dRuntimeTextUpdate::Partial(text_patch) => {
                    self.sync_text_patch(queue, &text_patch, text_state);
                }
            }
        }

        if let Some(ui) = update.ui {
            match ui {
                Ui2dRuntimeUiUpdate::Full(ui_data) => {
                    self.sync_ui_scene_data(queue, &ui_data.primitives);
                }
                Ui2dRuntimeUiUpdate::Partial(ui_patches) => {
                    if !ui_patches.is_empty() {
                        self.sync_ui_patches(queue, &ui_patches);
                    }
                }
            }
        }
    }

    pub fn apply_view_controls(
        &mut self,
        offset_delta: [f32; 2],
        scale_factor: f32,
        rotation_delta: f32,
    ) -> bool {
        let before = self.pass.view_state();
        self.pass
            .apply_view_controls(offset_delta, scale_factor, rotation_delta);
        self.pass.view_state() != before
    }

    pub fn set_view_state(&mut self, view_state: Ui2DViewState) {
        self.pass.set_view_state(view_state);
    }

    pub fn reset_rotation(&mut self) -> bool {
        let changed = self.pass.view_state().rotation != 0.0;
        self.pass.reset_rotation();
        changed
    }

    pub fn reset_all(&mut self) -> bool {
        let changed = self.pass.view_state() != Ui2DViewState::default();
        self.pass.reset_all();
        changed
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.pass.resize(width, height);
    }
}

impl Ui2dRuntimeHost {
    fn new(
        ctx: &DemoContext,
        label: &str,
        atlas: &VectorFontAtlas,
        init: &Ui2dSceneInitData,
    ) -> Self {
        let runtime = Ui2dPassHost::new_text_and_primitives(
            ctx,
            label,
            atlas,
            &init.text_data,
            &init.ui_primitives,
            init.text_capacity.max(1),
            init.grid_index_capacity.max(1),
            init.primitive_capacity.max(1),
        );
        Self {
            runtime,
            device: ctx.device.clone(),
            queue: ctx.queue.clone(),
            surface_format: ctx.surface_format,
            label: label.to_string(),
            width: ctx.width,
            height: ctx.height,
        }
    }

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue) {
        self.runtime.render(render_pass, queue);
    }

    fn needs_redraw(&self) -> bool {
        self.runtime.needs_redraw()
    }

    fn view_state(&self) -> Ui2DViewState {
        self.runtime.view_state()
    }

    fn set_view_state(&mut self, view_state: Ui2DViewState) {
        self.runtime.set_view_state(view_state);
    }

    fn apply_view_controls(
        &mut self,
        offset_delta: [f32; 2],
        scale_factor: f32,
        rotation_delta: f32,
    ) -> bool {
        self.runtime
            .apply_view_controls(offset_delta, scale_factor, rotation_delta)
    }

    fn reset_rotation(&mut self) -> bool {
        self.runtime.reset_rotation()
    }

    fn reset_all(&mut self) -> bool {
        self.runtime.reset_all()
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.runtime.resize(width, height);
    }

    fn sync_or_rebuild(&mut self, atlas: &VectorFontAtlas, init: &Ui2dSceneInitData) {
        let view_state = self.runtime.view_state();
        if self
            .runtime
            .can_fit_scene_data(&init.text_data, &init.ui_primitives)
        {
            self.runtime
                .sync_scene_data(&self.queue, &init.text_data, &init.ui_primitives);
            self.runtime.set_view_state(view_state);
            return;
        }

        let ctx = DemoContext {
            device: &self.device,
            queue: &self.queue,
            surface_format: self.surface_format,
            width: self.width,
            height: self.height,
            scale_factor: 1.0,
        };
        self.runtime = Ui2dPassHost::new_text_and_primitives(
            &ctx,
            &self.label,
            atlas,
            &init.text_data,
            &init.ui_primitives,
            init.text_capacity.max(1),
            init.grid_index_capacity.max(1),
            init.primitive_capacity.max(1),
        );
        self.runtime.set_view_state(view_state);
    }

    fn sync_runtime_update(&mut self, update: Ui2dRuntimeUpdate, text_state: &FixedTextSceneState) {
        self.runtime
            .sync_runtime_update(&self.queue, update, text_state);
    }
}

fn sync_or_rebuild_ui2d_runtime(
    runtime_host: &mut Ui2dRuntimeHost,
    atlas: &VectorFontAtlas,
    init: &Ui2dSceneInitData,
) {
    runtime_host.sync_or_rebuild(atlas, init);
}

fn sync_ui2d_runtime_update(
    runtime_host: &mut Ui2dRuntimeHost,
    update: Ui2dRuntimeUpdate,
    text_state: &FixedTextSceneState,
) {
    runtime_host.sync_runtime_update(update, text_state);
}

impl UiTextAndPrimitivesPass {
    pub fn view_state(&self) -> Ui2DViewState {
        self.view_state
    }

    pub fn needs_redraw(&self) -> bool {
        self.uniforms_dirty.get()
    }

    pub fn new(
        ctx: &DemoContext,
        label_prefix: &str,
        atlas: &VectorFontAtlas,
        text_data: &FixedTextSceneData,
        ui_primitives: &[GpuUiPrimitive],
        text_capacity: usize,
        grid_index_capacity: usize,
        primitive_capacity: usize,
    ) -> Self {
        let uniforms = Ui2DUniforms {
            screen_params: [
                ctx.width as f32,
                ctx.height as f32,
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

        let uniform_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{label_prefix} Uniform Buffer")),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let storage_buffers = create_storage_buffers(
            ctx.device,
            ctx.queue,
            &build_font_gpu_data(atlas),
            bytemuck::cast_slice(&text_data.char_instances),
            text_capacity * std::mem::size_of::<GpuCharInstanceEx>(),
            &text_data.char_grid_cells,
            &text_data.char_grid_indices,
            grid_index_capacity,
            bytemuck::cast_slice(ui_primitives),
            primitive_capacity * std::mem::size_of::<GpuUiPrimitive>(),
            &format!("{label_prefix} Primitives Buffer"),
        );

        let bind_group_layout = create_bind_group_layout_with_storage(
            ctx.device,
            &format!("{label_prefix} Bind Group Layout"),
            &[(0, wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT)],
            &UI2D_STORAGE_BINDINGS,
            wgpu::ShaderStages::FRAGMENT,
        );

        let bind_group = create_bind_group_with_storage(
            ctx.device,
            &format!("{label_prefix} Bind Group"),
            &bind_group_layout,
            &[(0, &uniform_buffer)],
            &storage_buffers,
            &UI2D_STORAGE_BINDINGS,
        );

        let UiStorageBuffers {
            char_instances_buffer,
            char_grid_cells_buffer,
            char_grid_indices_buffer,
            ui_primitives_buffer,
            ..
        } = storage_buffers;

        let shader_module = retained_ui2d_shader::create_shader_module_embed_source(ctx.device);
        let pipeline = create_fullscreen_pipeline(
            ctx.device,
            ctx.surface_format,
            &format!("{label_prefix} Pipeline"),
            &[&bind_group_layout],
            &shader_module,
        );

        Self {
            pipeline,
            uniform_buffer,
            char_instances_buffer,
            char_grid_cells_buffer,
            char_grid_indices_buffer,
            ui_primitives_buffer,
            bind_group,
            width: ctx.width,
            height: ctx.height,
            char_count: text_data.char_count,
            ui_prim_count: ui_primitives.len() as u32,
            char_grid_params: text_data.char_grid_params,
            char_grid_bounds: text_data.char_grid_bounds,
            text_capacity,
            grid_cell_capacity: text_data.char_grid_cells.len(),
            grid_index_capacity,
            primitive_capacity,
            view_state: Ui2DViewState::default(),
            uniforms_dirty: Cell::new(true),
        }
    }

    pub fn sync_scene_data(
        &mut self,
        queue: &wgpu::Queue,
        text_data: &FixedTextSceneData,
        ui_primitives: &[GpuUiPrimitive],
    ) {
        assert!(text_data.char_instances.len() <= self.text_capacity);
        assert!(text_data.char_grid_cells.len() <= self.grid_cell_capacity);
        assert!(text_data.char_grid_indices.len() <= self.grid_index_capacity);
        assert!(ui_primitives.len() <= self.primitive_capacity);

        self.sync_text_scene_data(queue, text_data);
        self.sync_ui_scene_data(queue, ui_primitives);
    }

    pub fn can_fit_scene_data(
        &self,
        text_data: &FixedTextSceneData,
        ui_primitives: &[GpuUiPrimitive],
    ) -> bool {
        text_data.char_instances.len() <= self.text_capacity
            && text_data.char_grid_cells.len() <= self.grid_cell_capacity
            && text_data.char_grid_indices.len() <= self.grid_index_capacity
            && ui_primitives.len() <= self.primitive_capacity
    }

    pub fn sync_text_scene_data(&mut self, queue: &wgpu::Queue, text_data: &FixedTextSceneData) {
        assert!(text_data.char_instances.len() <= self.text_capacity);
        assert!(text_data.char_grid_cells.len() <= self.grid_cell_capacity);
        assert!(text_data.char_grid_indices.len() <= self.grid_index_capacity);

        queue.write_buffer(
            &self.char_instances_buffer,
            0,
            bytemuck::cast_slice(&text_data.char_instances),
        );
        queue.write_buffer(
            &self.char_grid_cells_buffer,
            0,
            bytemuck::cast_slice(&text_data.char_grid_cells),
        );
        queue.write_buffer(
            &self.char_grid_indices_buffer,
            0,
            bytemuck::cast_slice(&text_data.char_grid_indices),
        );

        self.char_count = text_data.char_count;
        self.char_grid_params = text_data.char_grid_params;
        self.char_grid_bounds = text_data.char_grid_bounds;
        self.uniforms_dirty.set(true);
    }

    pub fn sync_ui_scene_data(&mut self, queue: &wgpu::Queue, ui_primitives: &[GpuUiPrimitive]) {
        assert!(ui_primitives.len() <= self.primitive_capacity);
        queue.write_buffer(
            &self.ui_primitives_buffer,
            0,
            bytemuck::cast_slice(ui_primitives),
        );
        self.ui_prim_count = ui_primitives.len() as u32;
        self.uniforms_dirty.set(true);
    }

    pub fn sync_ui_patches(&mut self, queue: &wgpu::Queue, patches: &[GpuUiPatch]) {
        let primitive_size = std::mem::size_of::<GpuUiPrimitive>() as u64;
        for patch in patches {
            if patch.primitives.is_empty() {
                continue;
            }
            queue.write_buffer(
                &self.ui_primitives_buffer,
                patch.offset as u64 * primitive_size,
                bytemuck::cast_slice(&patch.primitives),
            );
        }
        self.uniforms_dirty.set(true);
    }

    pub fn sync_text_run_slots(
        &mut self,
        queue: &wgpu::Queue,
        run_updates: &[(usize, Vec<GpuCharInstanceEx>)],
        char_count: u32,
        grid: &FixedTextGridCache,
        changed_cells: &[usize],
        grid_cell_capacity: usize,
    ) {
        let char_instance_size = std::mem::size_of::<GpuCharInstanceEx>() as u64;
        for (offset, slots) in run_updates {
            if slots.is_empty() {
                continue;
            }
            queue.write_buffer(
                &self.char_instances_buffer,
                *offset as u64 * char_instance_size,
                bytemuck::cast_slice(slots),
            );
        }

        let grid_cell_size = std::mem::size_of::<CharGridCell>() as u64;
        let grid_index_size = std::mem::size_of::<u32>() as u64;
        for &cell_idx in changed_cells {
            queue.write_buffer(
                &self.char_grid_cells_buffer,
                cell_idx as u64 * grid_cell_size,
                bytemuck::cast_slice(&[grid.cells[cell_idx]]),
            );

            let index_offset = grid.cell_index_offset(cell_idx);
            queue.write_buffer(
                &self.char_grid_indices_buffer,
                index_offset as u64 * grid_index_size,
                bytemuck::cast_slice(
                    &grid.indices[index_offset..index_offset + grid_cell_capacity],
                ),
            );
        }

        self.char_count = char_count;
        self.uniforms_dirty.set(true);
    }

    pub fn sync_text_patch(
        &mut self,
        queue: &wgpu::Queue,
        text_patch: &FixedTextScenePatch,
        text_state: &FixedTextSceneState,
    ) {
        self.sync_text_run_slots(
            queue,
            &text_patch.run_updates,
            text_patch.char_count,
            &text_state.text_grid,
            &text_patch.changed_cells,
            text_state.grid_cell_capacity(),
        );
    }

    pub fn sync_fixed_scene_update(
        &mut self,
        queue: &wgpu::Queue,
        update: FixedUi2dSceneUpdate,
        text_state: &FixedTextSceneState,
    ) {
        match update {
            FixedUi2dSceneUpdate::Full { text_data, ui_data } => {
                self.sync_scene_data(queue, &text_data, &ui_data.primitives);
            }
            FixedUi2dSceneUpdate::Partial {
                ui_patches,
                text_patch,
            } => {
                if !ui_patches.is_empty() {
                    self.sync_ui_patches(queue, &ui_patches);
                }
                if let Some(text_patch) = text_patch {
                    self.sync_text_patch(queue, &text_patch, text_state);
                }
            }
        }
    }

    fn update_uniforms(&self, queue: &wgpu::Queue) {
        if !self.uniforms_dirty.get() {
            return;
        }

        let uniforms = Ui2DUniforms {
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
        self.uniforms_dirty.set(false);
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue) {
        self.update_uniforms(queue);
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.uniforms_dirty.set(true);
    }

    pub fn apply_view_controls(
        &mut self,
        offset_delta: [f32; 2],
        scale_factor: f32,
        rotation_delta: f32,
    ) {
        if self
            .view_state
            .apply_controls(offset_delta, scale_factor, rotation_delta)
        {
            self.uniforms_dirty.set(true);
        }
    }

    pub fn set_view_state(&mut self, view_state: Ui2DViewState) {
        if self.view_state != view_state {
            self.view_state = view_state;
            self.uniforms_dirty.set(true);
        }
    }

    pub fn reset_rotation(&mut self) {
        if self.view_state.reset_rotation() {
            self.uniforms_dirty.set(true);
        }
    }

    pub fn reset_all(&mut self) {
        if self.view_state.reset_all() {
            self.uniforms_dirty.set(true);
        }
    }
}

impl FixedUi2dSceneHost {
    pub fn scene(&self) -> &RetainedScene {
        &self.scene_state.scene
    }

    pub fn scene_mut(&mut self) -> &mut RetainedScene {
        &mut self.scene_state.scene
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue) {
        self.runtime_host.render(render_pass, queue);
    }

    pub fn needs_redraw(&self) -> bool {
        !self.scene_state.scene.dirty().is_empty() || self.runtime_host.needs_redraw()
    }

    pub fn view_state(&self) -> Ui2DViewState {
        self.runtime_host.view_state()
    }

    pub fn atlas(&self) -> &VectorFontAtlas {
        &self.atlas
    }

    pub fn from_state_and_init(
        ctx: &DemoContext,
        label: &str,
        mut scene_state: FixedUi2dSceneState,
        init: FixedUi2dSceneInit,
        atlas: Arc<VectorFontAtlas>,
        text_colors: TextColors,
        text_render_space: TextRenderSpace,
        ui_render_space: UiRenderSpace,
    ) -> Self {
        let FixedUi2dSceneInit { text_data, ui_data } = init;
        let runtime_host = Ui2dRuntimeHost::new(
            ctx,
            label,
            &atlas,
            &Ui2dSceneInitData {
                text_data,
                primitive_capacity: ui_data.primitive_count.max(1),
                ui_primitives: ui_data.primitives,
                text_capacity: scene_state.text_state.layout().total_capacity().max(1),
                grid_index_capacity: scene_state.text_state.layout().grid_index_capacity().max(1),
            },
        );
        scene_state.clear_dirty();

        Self {
            scene_state,
            runtime_host,
            atlas,
            text_colors,
            text_render_space,
            ui_render_space,
        }
    }

    pub fn from_built_scene(
        ctx: &DemoContext,
        label: &str,
        built: BuiltFixedUi2dScene,
        atlas: Arc<VectorFontAtlas>,
        text_colors: TextColors,
        text_render_space: TextRenderSpace,
        ui_render_space: UiRenderSpace,
    ) -> Self {
        Self::from_state_and_init(
            ctx,
            label,
            built.state,
            built.init,
            atlas,
            text_colors,
            text_render_space,
            ui_render_space,
        )
    }

    pub fn new(
        ctx: &DemoContext,
        label: &str,
        scene: RetainedScene,
        layout: FixedTextRunLayout<'static>,
        atlas: Arc<VectorFontAtlas>,
        text_colors: TextColors,
        text_render_space: TextRenderSpace,
        ui_render_space: UiRenderSpace,
    ) -> Self {
        let (scene_state, init) = FixedUi2dSceneState::new(
            scene,
            layout,
            &atlas,
            &text_colors,
            text_render_space,
            ui_render_space,
        );
        Self::from_state_and_init(
            ctx,
            label,
            scene_state,
            init,
            atlas,
            text_colors,
            text_render_space,
            ui_render_space,
        )
    }

    pub fn prepare_frame(&mut self) {
        let Some(update) = self.scene_state.take_update(
            &self.atlas,
            &self.text_colors,
            self.text_render_space,
            self.ui_render_space,
        ) else {
            return;
        };

        sync_ui2d_runtime_update(
            &mut self.runtime_host,
            update.into(),
            &self.scene_state.text_state,
        );
    }

    pub fn apply_view_controls(
        &mut self,
        offset_delta: [f32; 2],
        scale_factor: f32,
        rotation_delta: f32,
    ) {
        let _ = self
            .runtime_host
            .apply_view_controls(offset_delta, scale_factor, rotation_delta);
    }

    pub fn set_view_state(&mut self, view_state: Ui2DViewState) {
        self.runtime_host.set_view_state(view_state);
    }

    pub fn reset_rotation(&mut self) {
        let _ = self.runtime_host.reset_rotation();
    }

    pub fn reset_all(&mut self) {
        let _ = self.runtime_host.reset_all();
    }

    pub fn set_render_spaces(
        &mut self,
        text_render_space: TextRenderSpace,
        ui_render_space: UiRenderSpace,
    ) {
        self.text_render_space = text_render_space;
        self.ui_render_space = ui_render_space;
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.runtime_host.resize(width, height);
    }

    pub fn replace_state_and_init(
        &mut self,
        mut scene_state: FixedUi2dSceneState,
        init: FixedUi2dSceneInit,
    ) {
        let mut init = Ui2dSceneInitData::from(init);
        init.text_capacity = scene_state.text_state.layout().total_capacity().max(1);
        init.grid_index_capacity = scene_state.text_state.layout().grid_index_capacity().max(1);
        scene_state.clear_dirty();
        sync_or_rebuild_ui2d_runtime(&mut self.runtime_host, &self.atlas, &init);
        self.scene_state = scene_state;
    }

    pub fn replace_built_scene(&mut self, built: BuiltFixedUi2dScene) {
        self.replace_state_and_init(built.state, built.init);
    }

    pub fn replace_state_and_init_with_spaces(
        &mut self,
        scene_state: FixedUi2dSceneState,
        init: FixedUi2dSceneInit,
        text_render_space: TextRenderSpace,
        ui_render_space: UiRenderSpace,
    ) {
        self.text_render_space = text_render_space;
        self.ui_render_space = ui_render_space;
        self.replace_state_and_init(scene_state, init);
    }

    pub fn replace_built_scene_with_spaces(
        &mut self,
        built: BuiltFixedUi2dScene,
        text_render_space: TextRenderSpace,
        ui_render_space: UiRenderSpace,
    ) {
        self.text_render_space = text_render_space;
        self.ui_render_space = ui_render_space;
        self.replace_built_scene(built);
    }

    pub fn replace_scene(&mut self, scene: RetainedScene, layout: FixedTextRunLayout<'static>) {
        let (scene_state, init) = FixedUi2dSceneState::new(
            scene,
            layout,
            &self.atlas,
            &self.text_colors,
            self.text_render_space,
            self.ui_render_space,
        );
        self.replace_state_and_init(scene_state, init);
    }

    pub fn replace_scene_with_spaces(
        &mut self,
        scene: RetainedScene,
        layout: FixedTextRunLayout<'static>,
        text_render_space: TextRenderSpace,
        ui_render_space: UiRenderSpace,
    ) {
        self.text_render_space = text_render_space;
        self.ui_render_space = ui_render_space;
        self.replace_scene(scene, layout);
    }
}

impl<M: FixedUi2dSceneModelBuilder> ModeledFixedUi2dSceneHost<M> {
    pub fn new(
        ctx: &DemoContext,
        label: &str,
        model: M,
        atlas: Arc<VectorFontAtlas>,
        text_colors: TextColors,
        text_render_space: TextRenderSpace,
        ui_render_space: UiRenderSpace,
    ) -> Self {
        let built = model.build_fixed_ui2d_scene(
            [ctx.width, ctx.height],
            &atlas,
            &text_colors,
            text_render_space,
            ui_render_space,
        );
        let host = FixedUi2dSceneHost::from_built_scene(
            ctx,
            label,
            built,
            atlas,
            text_colors,
            text_render_space,
            ui_render_space,
        );
        Self {
            model,
            host,
            viewport_size: [ctx.width, ctx.height],
            text_colors,
            needs_rebuild: false,
        }
    }

    pub fn model(&self) -> &M {
        &self.model
    }

    pub fn model_mut(&mut self) -> &mut M {
        &mut self.model
    }

    pub fn host(&self) -> &FixedUi2dSceneHost {
        &self.host
    }

    pub fn host_mut(&mut self) -> &mut FixedUi2dSceneHost {
        &mut self.host
    }

    pub fn needs_rebuild(&self) -> bool {
        self.needs_rebuild
    }

    pub fn scene(&self) -> &RetainedScene {
        self.host.scene()
    }

    pub fn scene_mut(&mut self) -> &mut RetainedScene {
        self.host.scene_mut()
    }

    pub fn view_state(&self) -> Ui2DViewState {
        self.host.view_state()
    }

    pub fn needs_redraw(&self) -> bool {
        self.needs_rebuild || self.host.needs_redraw()
    }

    pub fn set_view_state(&mut self, view_state: Ui2DViewState) {
        self.host.set_view_state(view_state);
    }

    pub fn apply_view_controls(
        &mut self,
        offset_delta: [f32; 2],
        scale_factor: f32,
        rotation_delta: f32,
    ) {
        self.host
            .apply_view_controls(offset_delta, scale_factor, rotation_delta);
    }

    pub fn reset_rotation(&mut self) {
        self.host.reset_rotation();
    }

    pub fn reset_all(&mut self) {
        self.host.reset_all();
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue) {
        self.host.render(render_pass, queue);
    }

    pub fn resize_lazy(&mut self, width: u32, height: u32) {
        self.viewport_size = [width, height];
        self.needs_rebuild = true;
    }

    pub fn resize_and_rebuild(&mut self, width: u32, height: u32) {
        self.viewport_size = [width, height];
        self.rebuild();
    }

    pub fn rebuild(&mut self) {
        self.host
            .resize(self.viewport_size[0], self.viewport_size[1]);
        let text_render_space = self.host.text_render_space;
        let ui_render_space = self.host.ui_render_space;
        let built = self.model.build_fixed_ui2d_scene(
            self.viewport_size,
            self.host.atlas(),
            &self.text_colors,
            text_render_space,
            ui_render_space,
        );
        self.host
            .replace_built_scene_with_spaces(built, text_render_space, ui_render_space);
        self.needs_rebuild = false;
    }

    pub fn ensure_ready(&mut self) {
        if self.needs_rebuild {
            self.rebuild();
        }
    }

    pub fn prepare_frame(&mut self) {
        self.ensure_ready();
        self.host.prepare_frame();
    }
}

impl<M: FixedUi2dSceneModelBuilder> Ui2dDeckHost for ModeledFixedUi2dSceneHost<M> {
    fn view_state(&self) -> Ui2DViewState {
        ModeledFixedUi2dSceneHost::view_state(self)
    }

    fn set_view_state(&mut self, view_state: Ui2DViewState) {
        ModeledFixedUi2dSceneHost::set_view_state(self, view_state);
    }

    fn needs_redraw(&self) -> bool {
        ModeledFixedUi2dSceneHost::needs_redraw(self)
    }

    fn apply_view_controls(
        &mut self,
        offset_delta: [f32; 2],
        scale_factor: f32,
        rotation_delta: f32,
    ) {
        ModeledFixedUi2dSceneHost::apply_view_controls(
            self,
            offset_delta,
            scale_factor,
            rotation_delta,
        );
    }

    fn reset_rotation(&mut self) {
        ModeledFixedUi2dSceneHost::reset_rotation(self);
    }

    fn reset_all(&mut self) {
        ModeledFixedUi2dSceneHost::reset_all(self);
    }

    fn prepare_frame(&mut self) {
        ModeledFixedUi2dSceneHost::prepare_frame(self);
    }

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue) {
        ModeledFixedUi2dSceneHost::render(self, render_pass, queue);
    }

    fn resize(&mut self, width: u32, height: u32) {
        ModeledFixedUi2dSceneHost::resize_and_rebuild(self, width, height);
    }

    fn resize_lazy(&mut self, width: u32, height: u32) {
        ModeledFixedUi2dSceneHost::resize_lazy(self, width, height);
    }

    fn ensure_ready(&mut self) {
        ModeledFixedUi2dSceneHost::ensure_ready(self);
    }
}

impl<M: FixedUi2dSceneModelBuilder + FixedUi2dSceneModelCapture> ModeledFixedUi2dSceneHost<M> {
    pub fn capture_model_from_scene(&mut self) {
        self.model.capture_from_scene(self.host.scene());
    }

    pub fn mutate_scene_and_capture<F>(&mut self, mutate: F) -> bool
    where
        F: FnOnce(&mut RetainedScene) -> bool,
    {
        let changed = mutate(self.host.scene_mut());
        if changed {
            self.capture_model_from_scene();
        }
        changed
    }
}

impl<H: Ui2dDeckHost> Ui2dSceneDeck<H> {
    pub fn new(scenes: Vec<H>) -> Self {
        assert!(
            !scenes.is_empty(),
            "Ui2D scene deck requires at least one scene"
        );
        Self {
            scenes,
            active_scene: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.scenes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scenes.is_empty()
    }

    pub fn active_index(&self) -> usize {
        self.active_scene
    }

    pub fn active_host(&self) -> &H {
        &self.scenes[self.active_scene]
    }

    pub fn active_host_mut(&mut self) -> &mut H {
        &mut self.scenes[self.active_scene]
    }

    pub fn view_state(&self) -> Ui2DViewState {
        self.active_host().view_state()
    }

    pub fn needs_redraw(&self) -> bool {
        self.active_host().needs_redraw()
    }

    pub fn set_view_state(&mut self, view_state: Ui2DViewState) {
        self.active_host_mut().set_view_state(view_state);
    }

    pub fn apply_view_controls(
        &mut self,
        offset_delta: [f32; 2],
        scale_factor: f32,
        rotation_delta: f32,
    ) {
        self.active_host_mut()
            .apply_view_controls(offset_delta, scale_factor, rotation_delta);
    }

    pub fn reset_rotation(&mut self) {
        self.active_host_mut().reset_rotation();
    }

    pub fn reset_all(&mut self) {
        self.active_host_mut().reset_all();
    }

    pub fn cycle_next(&mut self) -> bool {
        let next_scene = (self.active_scene + 1) % self.scenes.len();
        self.set_active_scene(next_scene)
    }

    pub fn set_active_scene(&mut self, index: usize) -> bool {
        if index >= self.scenes.len() || index == self.active_scene {
            return false;
        }

        let current_view_state = self.scenes[self.active_scene].view_state();
        self.active_scene = index;
        self.ensure_active_scene_ready();
        self.scenes[self.active_scene].set_view_state(current_view_state);
        true
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        for (index, scene) in self.scenes.iter_mut().enumerate() {
            if index == self.active_scene {
                scene.resize(width, height);
            } else {
                scene.resize_lazy(width, height);
            }
        }
    }

    pub fn ensure_active_scene_ready(&mut self) {
        self.active_host_mut().ensure_ready();
    }

    pub fn prepare_frame(&mut self) {
        self.ensure_active_scene_ready();
        self.active_host_mut().prepare_frame();
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue) {
        self.active_host().render(render_pass, queue);
    }
}

impl<M: FixedUi2dSceneModelBuilder> Ui2dSceneDeck<ModeledFixedUi2dSceneHost<M>> {
    pub fn model(&self) -> &M {
        self.active_host().model()
    }

    pub fn model_mut(&mut self) -> &mut M {
        self.active_host_mut().model_mut()
    }

    pub fn scene(&self) -> &RetainedScene {
        self.active_host().scene()
    }

    pub fn scene_mut(&mut self) -> &mut RetainedScene {
        self.active_host_mut().scene_mut()
    }
}

impl<M: FixedUi2dSceneModelBuilder + FixedUi2dSceneModelCapture>
    Ui2dSceneDeck<ModeledFixedUi2dSceneHost<M>>
{
    pub fn capture_active_model_from_scene(&mut self) {
        self.active_host_mut().capture_model_from_scene();
    }

    pub fn mutate_active_scene_and_capture<F>(&mut self, mutate: F) -> bool
    where
        F: FnOnce(&mut RetainedScene) -> bool,
    {
        self.active_host_mut().mutate_scene_and_capture(mutate)
    }
}

impl SampleSceneDeckTarget for Ui2dSceneDeck<ModeledFixedUi2dSceneHost<SampleSceneModel>> {
    fn active_sample_scene_kind(&self) -> crate::retained::samples::SampleSceneKind {
        self.model().kind
    }

    fn cycle_sample_scene(&mut self) -> bool {
        self.cycle_next()
    }

    fn apply_active_sample_scene_action(&mut self, action: SampleSceneAction) -> bool {
        let model = *self.model();
        self.mutate_active_scene_and_capture(|scene| model.apply_action(scene, action))
    }
}

impl ShowcaseSceneDeckTarget for Ui2dSceneDeck<ModeledFixedUi2dSceneHost<ShowcaseSceneModel>> {
    fn cycle_showcase_scene(&mut self) -> bool {
        self.cycle_next()
    }

    fn apply_active_showcase_action(&mut self, action: ShowcaseSceneAction) -> bool {
        let model = self.model().clone();
        self.mutate_active_scene_and_capture(|scene| model.apply_action(scene, action))
    }
}

impl<S: Ui2dSceneState> StateBackedUi2dHost<S> {
    pub fn new(ctx: &DemoContext, label: &str, state: S, text_colors: TextColors) -> Self {
        let init = state.build_ui2d_init_data();
        let runtime_host = Ui2dRuntimeHost::new(ctx, label, state.atlas(), &init);
        Self {
            state,
            runtime_host,
            text_colors,
        }
    }

    pub fn state(&self) -> &S {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut S {
        &mut self.state
    }

    pub fn scene(&self) -> &RetainedScene {
        self.state.scene()
    }

    pub fn scene_mut(&mut self) -> &mut RetainedScene {
        self.state.scene_mut()
    }

    pub fn mutate_scene<F>(&mut self, mutate: F) -> bool
    where
        F: FnOnce(&mut RetainedScene) -> bool,
    {
        mutate(self.state.scene_mut())
    }

    pub fn view_state(&self) -> Ui2DViewState {
        self.runtime_host.view_state()
    }

    pub fn needs_redraw(&self) -> bool {
        !self.state.scene().dirty().is_empty() || self.runtime_host.needs_redraw()
    }

    pub fn set_view_state(&mut self, view_state: Ui2DViewState) {
        self.runtime_host.set_view_state(view_state);
    }

    pub fn apply_view_controls(
        &mut self,
        offset_delta: [f32; 2],
        scale_factor: f32,
        rotation_delta: f32,
    ) -> bool {
        let changed =
            self.runtime_host
                .apply_view_controls(offset_delta, scale_factor, rotation_delta);
        if changed {
            self.state.mark_view_transform_dirty();
        }
        changed
    }

    pub fn reset_rotation(&mut self) -> bool {
        let changed = self.runtime_host.reset_rotation();
        if changed {
            self.state.mark_view_transform_dirty();
        }
        changed
    }

    pub fn reset_all(&mut self) -> bool {
        let changed = self.runtime_host.reset_all();
        if changed {
            self.state.mark_view_transform_dirty();
        }
        changed
    }

    pub fn prepare_frame(&mut self) {
        let Some(update) = self.state.take_ui2d_runtime_update(&self.text_colors) else {
            return;
        };

        if update.needs_full_rebuild() {
            let init = self.state.build_ui2d_init_data();
            if !self
                .runtime_host
                .runtime
                .can_fit_scene_data(&init.text_data, &init.ui_primitives)
            {
                sync_or_rebuild_ui2d_runtime(&mut self.runtime_host, self.state.atlas(), &init);
                return;
            }
        }

        sync_ui2d_runtime_update(&mut self.runtime_host, update, self.state.text_state());
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue) {
        self.runtime_host.render(render_pass, queue);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.runtime_host.resize(width, height);
        self.state.set_viewport_size(width, height);
    }
}

impl<S: Ui2dSceneState> Ui2dDeckHost for StateBackedUi2dHost<S> {
    fn view_state(&self) -> Ui2DViewState {
        StateBackedUi2dHost::view_state(self)
    }

    fn set_view_state(&mut self, view_state: Ui2DViewState) {
        StateBackedUi2dHost::set_view_state(self, view_state);
    }

    fn needs_redraw(&self) -> bool {
        StateBackedUi2dHost::needs_redraw(self)
    }

    fn apply_view_controls(
        &mut self,
        offset_delta: [f32; 2],
        scale_factor: f32,
        rotation_delta: f32,
    ) {
        let _ = StateBackedUi2dHost::apply_view_controls(
            self,
            offset_delta,
            scale_factor,
            rotation_delta,
        );
    }

    fn reset_rotation(&mut self) {
        let _ = StateBackedUi2dHost::reset_rotation(self);
    }

    fn reset_all(&mut self) {
        let _ = StateBackedUi2dHost::reset_all(self);
    }

    fn prepare_frame(&mut self) {
        StateBackedUi2dHost::prepare_frame(self);
    }

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue) {
        StateBackedUi2dHost::render(self, render_pass, queue);
    }

    fn resize(&mut self, width: u32, height: u32) {
        StateBackedUi2dHost::resize(self, width, height);
    }
}

impl<S: Ui2dSceneState> Ui2dSceneDeck<StateBackedUi2dHost<S>> {
    pub fn active_state(&self) -> &S {
        self.active_host().state()
    }

    pub fn active_state_mut(&mut self) -> &mut S {
        self.active_host_mut().state_mut()
    }

    pub fn active_scene(&self) -> &RetainedScene {
        self.active_host().scene()
    }

    pub fn active_scene_mut(&mut self) -> &mut RetainedScene {
        self.active_host_mut().scene_mut()
    }

    pub fn mutate_active_scene<F>(&mut self, mutate: F) -> bool
    where
        F: FnOnce(&mut RetainedScene) -> bool,
    {
        self.active_host_mut().mutate_scene(mutate)
    }
}

impl<M> NamedScrollTarget for ModeledFixedUi2dSceneHost<M>
where
    M: FixedUi2dSceneModelBuilder + FixedUi2dSceneModelCapture + NamedScrollSceneModel,
{
    fn set_named_scroll_offset(&mut self, name: &str, offset_y: f32) -> bool {
        let changed = {
            let model = &self.model;
            let scene = self.host.scene_mut();
            model.set_named_scroll_offset(scene, name, offset_y)
        };
        if changed {
            self.capture_model_from_scene();
        }
        changed
    }
}

impl<S: Ui2dSceneState + NamedScrollTarget> NamedScrollTarget for StateBackedUi2dHost<S> {
    fn set_named_scroll_offset(&mut self, name: &str, offset_y: f32) -> bool {
        self.state_mut().set_named_scroll_offset(name, offset_y)
    }
}

impl<S: Ui2dSceneState + ListCommandTarget> ListCommandTarget for StateBackedUi2dHost<S> {
    fn toggle_item(&mut self, index: usize) -> bool {
        self.state_mut().toggle_item(index)
    }

    fn set_item_completed(&mut self, index: usize, completed: bool) -> bool {
        self.state_mut().set_item_completed(index, completed)
    }

    fn set_item_label(&mut self, index: usize, label: &str) -> bool {
        self.state_mut().set_item_label(index, label)
    }

    fn set_filter(&mut self, filter: ListFilter) -> bool {
        self.state_mut().set_filter(filter)
    }

    fn set_scroll_offset(&mut self, offset_y: f32) {
        self.state_mut().set_scroll_offset(offset_y);
    }
}

impl<H: Ui2dDeckHost + NamedScrollTarget> NamedScrollTarget for Ui2dSceneDeck<H> {
    fn set_named_scroll_offset(&mut self, name: &str, offset_y: f32) -> bool {
        self.active_host_mut()
            .set_named_scroll_offset(name, offset_y)
    }
}

impl<H: Ui2dDeckHost + ListCommandTarget> ListCommandTarget for Ui2dSceneDeck<H> {
    fn toggle_item(&mut self, index: usize) -> bool {
        self.active_host_mut().toggle_item(index)
    }

    fn set_item_completed(&mut self, index: usize, completed: bool) -> bool {
        self.active_host_mut().set_item_completed(index, completed)
    }

    fn set_item_label(&mut self, index: usize, label: &str) -> bool {
        self.active_host_mut().set_item_label(index, label)
    }

    fn set_filter(&mut self, filter: ListFilter) -> bool {
        self.active_host_mut().set_filter(filter)
    }

    fn set_scroll_offset(&mut self, offset_y: f32) {
        self.active_host_mut().set_scroll_offset(offset_y);
    }
}

pub fn load_dejavu_font_atlas() -> Result<Arc<VectorFontAtlas>> {
    let font_data = fs::read("assets/fonts/DejaVuSans.ttf").context("Failed to load font file")?;
    let font = VectorFont::from_ttf(&font_data).map_err(|e| anyhow::anyhow!(e))?;
    Ok(Arc::new(VectorFontAtlas::from_font(&font, 32)))
}

pub fn create_showcase_ui2d_deck(
    ctx: &DemoContext,
    atlas: Arc<VectorFontAtlas>,
    text_colors: TextColors,
    text_render_space: TextRenderSpace,
    ui_render_space: UiRenderSpace,
) -> ShowcaseUi2dDeck {
    let scenes = ShowcaseSceneModel::default_deck_models(SceneMode::Ui2D)
        .into_iter()
        .map(|model| {
            ModeledFixedUi2dSceneHost::new(
                ctx,
                model.label(),
                model,
                atlas.clone(),
                text_colors,
                text_render_space,
                ui_render_space,
            )
        })
        .collect();
    Ui2dSceneDeck::new(scenes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MockDeckHost {
        view_state: Ui2DViewState,
        prepare_count: usize,
        resize_count: usize,
        resize_lazy_count: usize,
        ensure_ready_count: usize,
        needs_redraw: bool,
    }

    impl Ui2dDeckHost for MockDeckHost {
        fn view_state(&self) -> Ui2DViewState {
            self.view_state
        }

        fn set_view_state(&mut self, view_state: Ui2DViewState) {
            self.view_state = view_state;
        }

        fn needs_redraw(&self) -> bool {
            self.needs_redraw
        }

        fn apply_view_controls(
            &mut self,
            offset_delta: [f32; 2],
            scale_factor: f32,
            rotation_delta: f32,
        ) {
            let _ = self
                .view_state
                .apply_controls(offset_delta, scale_factor, rotation_delta);
        }

        fn reset_rotation(&mut self) {
            let _ = self.view_state.reset_rotation();
        }

        fn reset_all(&mut self) {
            let _ = self.view_state.reset_all();
        }

        fn prepare_frame(&mut self) {
            self.prepare_count += 1;
        }

        fn render<'a>(&'a self, _render_pass: &mut wgpu::RenderPass<'a>, _queue: &wgpu::Queue) {}

        fn resize(&mut self, _width: u32, _height: u32) {
            self.resize_count += 1;
        }

        fn resize_lazy(&mut self, _width: u32, _height: u32) {
            self.resize_lazy_count += 1;
        }

        fn ensure_ready(&mut self) {
            self.ensure_ready_count += 1;
        }
    }

    #[test]
    fn generic_scene_deck_preserves_view_state_on_switch() {
        let mut deck = Ui2dSceneDeck::new(vec![MockDeckHost::default(), MockDeckHost::default()]);
        deck.set_view_state(Ui2DViewState {
            offset: [12.0, -6.0],
            scale: 1.5,
            rotation: 0.25,
        });

        assert!(deck.set_active_scene(1));
        assert_eq!(
            deck.view_state(),
            Ui2DViewState {
                offset: [12.0, -6.0],
                scale: 1.5,
                rotation: 0.25,
            }
        );
        assert_eq!(deck.active_host().ensure_ready_count, 1);
    }

    #[test]
    fn generic_scene_deck_uses_eager_and_lazy_resize_paths() {
        let mut deck = Ui2dSceneDeck::new(vec![MockDeckHost::default(), MockDeckHost::default()]);

        deck.resize(800, 600);

        assert_eq!(deck.scenes[0].resize_count, 1);
        assert_eq!(deck.scenes[0].resize_lazy_count, 0);
        assert_eq!(deck.scenes[1].resize_count, 0);
        assert_eq!(deck.scenes[1].resize_lazy_count, 1);
    }

    #[test]
    fn generic_scene_deck_reports_pending_redraw_from_active_host() {
        let mut deck = Ui2dSceneDeck::new(vec![
            MockDeckHost::default(),
            MockDeckHost {
                needs_redraw: true,
                ..Default::default()
            },
        ]);

        assert!(!deck.needs_redraw());
        assert!(deck.set_active_scene(1));
        assert!(deck.needs_redraw());
    }
}
