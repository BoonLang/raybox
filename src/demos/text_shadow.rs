//! Demo 6: Floating 3D Text with Shadows using Vector SDF
//!
//! 3D extruded text casting soft shadows on ground plane.

use super::{
    world3d_runtime::{vector_text_storage_bindings, VectorTextStorageBuffers, World3dStorageHost},
    Demo, DemoContext, DemoId, DemoType, KEYBINDINGS_3D,
};
use crate::camera::FlyCamera;
use crate::input::CameraConfig;
use crate::shader_bindings::sdf_text_shadow_vector;
use crate::text::{build_char_grid, VectorFont, VectorFontAtlas};
use anyhow::{Context, Result};
use wgpu::util::DeviceExt;

const LOREM: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum. Curabitur pretium tincidunt lacus. Nulla gravida orci a odio. Nullam varius, turpis et commodo pharetra, est eros bibendum elit, nec luctus magna felis sollicitudin mauris. Integer in mauris eu nibh euismod gravida. Duis ac tellus et risus vulputate vehicula. Donec lobortis risus a elit. Etiam tempor. Ut ullamcorper, ligula eu tempor congue, eros est euismod turpis, id tincidunt sapien risus a quam. Maecenas fermentum consequat mi. Donec fermentum. Pellentesque malesuada nulla a mi. Duis sapien sem, aliquet sed, vulputate eget, feugiat non, orci. Sed neque. Sed eget lacus. Mauris non dui nec urna suscipit nonummy. Fusce fermentum fermentum arcu. Vestibulum ante ipsum primis in faucibus orci luctus et ultrices posuere cubilia curae.";

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct AtlasGridCell {
    curve_start_and_count: u32,
}

type GpuBezierCurve = sdf_text_shadow_vector::BezierCurve_std430_0;
type GpuGlyphData = sdf_text_shadow_vector::GlyphData_std430_0;
type GpuCharInstance = sdf_text_shadow_vector::CharInstance_std430_0;
type TextShadowUniforms = sdf_text_shadow_vector::Uniforms_std140_0;

fn text_shadow_uniforms(
    camera: &FlyCamera,
    width: u32,
    height: u32,
    time: f32,
    char_count: u32,
    text_aabb: [f32; 4],
    char_grid_params: [f32; 4],
    char_grid_bounds: [f32; 4],
) -> TextShadowUniforms {
    let aspect = width as f32 / height as f32;
    let position = camera.position();
    TextShadowUniforms::new(
        sdf_text_shadow_vector::_MatrixStorage_float4x4_ColMajorstd140_0::new(
            camera.inv_view_projection_matrix(aspect).to_cols_array_2d(),
        ),
        [position.x, position.y, position.z, time],
        [0.4, 0.8, 0.5, 1.3],
        [width as f32, height as f32, 0.15, 1.0],
        [char_count as f32, 0.0, 0.4, 0.0],
        text_aabb,
        char_grid_params,
        char_grid_bounds,
    )
}

fn build_shadow_text_layout(atlas: &VectorFontAtlas) -> Vec<GpuCharInstance> {
    let mut instances = Vec::new();

    let full_text = format!(
        "VECTOR SDF TEXT\n\n{}",
        format!(
            "{} {} {} {} {} {}",
            LOREM, LOREM, LOREM, LOREM, LOREM, LOREM
        )
    );

    let scale = 0.12;
    let line_height = 0.18;
    let margin = 0.1;

    let panel_width = 4.0;
    let panel_height = 3.0;
    let start_x = -panel_width / 2.0 + margin;
    let start_y = panel_height / 2.0 - margin;
    let max_x = panel_width / 2.0 - margin;

    let mut x = start_x;
    let mut y = start_y;
    let mut line_num = 0;
    let max_lines = 30;

    for ch in full_text.chars() {
        if line_num >= max_lines {
            break;
        }

        if ch == '\n' {
            x = start_x;
            y -= line_height;
            line_num += 1;
            continue;
        }

        let codepoint = ch as u32;
        if let Some(entry) = atlas.glyphs.get(&codepoint) {
            let glyph_idx = atlas
                .glyph_list
                .iter()
                .position(|(cp, _)| *cp == codepoint)
                .unwrap_or(0) as u32;

            let advance = entry.advance * scale;

            if x + advance > max_x {
                x = start_x;
                y -= line_height;
                line_num += 1;
                if line_num >= max_lines {
                    break;
                }
            }

            instances.push(GpuCharInstance::new([x, y, scale, glyph_idx as f32]));

            x += advance;
        } else if ch == ' ' {
            x += 0.3 * scale;
            if x > max_x {
                x = start_x;
                y -= line_height;
                line_num += 1;
            }
        }
    }

    instances
}

fn compute_text_aabb(instances: &[GpuCharInstance], atlas: &VectorFontAtlas) -> [f32; 4] {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for inst in instances {
        let x = inst.posAndChar_0[0];
        let y = inst.posAndChar_0[1];
        let scale = inst.posAndChar_0[2];
        let glyph_idx = inst.posAndChar_0[3] as usize;

        if glyph_idx < atlas.glyph_list.len() {
            let (_, entry) = &atlas.glyph_list[glyph_idx];
            let bounds = entry.bounds;
            min_x = min_x.min(x + bounds[0] * scale);
            min_y = min_y.min(y + bounds[1] * scale);
            max_x = max_x.max(x + bounds[2] * scale);
            max_y = max_y.max(y + bounds[3] * scale);
        }
    }

    // Add small margin
    [min_x - 0.05, min_y - 0.05, max_x + 0.05, max_y + 0.05]
}

pub struct TextShadowDemo {
    host: World3dStorageHost<TextShadowUniforms>,
    char_count: u32,
    text_aabb: [f32; 4],
    char_grid_params: [f32; 4],
    char_grid_bounds: [f32; 4],
}

impl TextShadowDemo {
    pub fn new(ctx: &DemoContext) -> Result<Self> {
        // Load vector font
        let font_data =
            std::fs::read("assets/fonts/DejaVuSans.ttf").context("Failed to load font file")?;
        let font = VectorFont::from_ttf(&font_data).map_err(|e| anyhow::anyhow!(e))?;
        let atlas = VectorFontAtlas::from_font(&font, 32);

        // Build text layout
        let char_instances = build_shadow_text_layout(&atlas);
        let char_count = char_instances.len() as u32;
        let text_aabb = compute_text_aabb(&char_instances, &atlas);

        // Build character spatial grid
        let instance_data: Vec<[f32; 4]> = char_instances.iter().map(|c| c.posAndChar_0).collect();
        let char_grid = build_char_grid(&instance_data, &atlas, [64, 48]);

        let char_grid_params = [
            char_grid.dims[0] as f32,
            char_grid.dims[1] as f32,
            char_grid.cell_size[0],
            char_grid.cell_size[1],
        ];
        let char_grid_bounds = char_grid.bounds;

        // Prepare GPU data
        let gpu_grid_cells: Vec<AtlasGridCell> = atlas
            .grid_cells
            .iter()
            .map(|c| AtlasGridCell {
                curve_start_and_count: (c.curve_start as u32)
                    | ((c.curve_count as u32) << 16)
                    | ((c.flags as u32) << 24),
            })
            .collect();

        let gpu_curve_indices: Vec<u32> = atlas.curve_indices.iter().map(|&i| i as u32).collect();

        let gpu_curves: Vec<GpuBezierCurve> = atlas
            .curves
            .iter()
            .map(|c| {
                let p0 = c.p0();
                let p1 = c.p1();
                let p2 = c.p2();
                GpuBezierCurve::new(
                    [p0.0, p0.1, p1.0, p1.1],
                    [p2.0, p2.1, c.bbox[0], c.bbox[1]],
                    [c.bbox[2], c.bbox[3], c.flags as f32, 0.0],
                )
            })
            .collect();

        let gpu_glyph_data: Vec<GpuGlyphData> = atlas
            .glyph_list
            .iter()
            .map(|(_, entry)| {
                GpuGlyphData::new(
                    entry.bounds,
                    [entry.grid_offset, entry.grid_size[0], entry.grid_size[1], 0],
                    [entry.curve_offset, entry.curve_count, 0, 0],
                )
            })
            .collect();
        let empty_curve = [GpuBezierCurve::new([0.0; 4], [0.0; 4], [0.0; 4])];
        let empty_glyph = [GpuGlyphData::new([0.0; 4], [0; 4], [0; 4])];
        let empty_char_instance = [GpuCharInstance::new([0.0; 4])];

        let uniforms = text_shadow_uniforms(
            &FlyCamera::default(),
            ctx.width,
            ctx.height,
            0.0,
            char_count,
            text_aabb,
            char_grid_params,
            char_grid_bounds,
        );

        let grid_cells_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Grid Cells Buffer"),
                contents: bytemuck::cast_slice(if gpu_grid_cells.is_empty() {
                    &[AtlasGridCell {
                        curve_start_and_count: 0,
                    }]
                } else {
                    &gpu_grid_cells
                }),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let curve_indices_buffer =
            ctx.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Curve Indices Buffer"),
                    contents: bytemuck::cast_slice(if gpu_curve_indices.is_empty() {
                        &[0u32]
                    } else {
                        &gpu_curve_indices
                    }),
                    usage: wgpu::BufferUsages::STORAGE,
                });

        let curves_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Curves Buffer"),
                contents: bytemuck::cast_slice(if gpu_curves.is_empty() {
                    &empty_curve
                } else {
                    &gpu_curves
                }),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let glyph_data_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Glyph Data Buffer"),
                contents: bytemuck::cast_slice(if gpu_glyph_data.is_empty() {
                    &empty_glyph
                } else {
                    &gpu_glyph_data
                }),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let char_instances_buffer =
            ctx.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Char Instances Buffer"),
                    contents: bytemuck::cast_slice(if char_instances.is_empty() {
                        &empty_char_instance
                    } else {
                        &char_instances
                    }),
                    usage: wgpu::BufferUsages::STORAGE,
                });

        let char_grid_cells_buffer =
            ctx.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Char Grid Cells Buffer"),
                    contents: bytemuck::cast_slice(&char_grid.cells),
                    usage: wgpu::BufferUsages::STORAGE,
                });

        let char_grid_indices_buffer =
            ctx.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Char Grid Indices Buffer"),
                    contents: bytemuck::cast_slice(&char_grid.char_indices),
                    usage: wgpu::BufferUsages::STORAGE,
                });

        let char_grid_dist_buffer =
            ctx.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Char Grid Distance Field Buffer"),
                    contents: bytemuck::cast_slice(&char_grid.cell_distances),
                    usage: wgpu::BufferUsages::STORAGE,
                });

        let shader_module = sdf_text_shadow_vector::create_shader_module_embed_source(ctx.device);
        let storage_bindings = vector_text_storage_bindings(VectorTextStorageBuffers {
            grid_cells: &grid_cells_buffer,
            curve_indices: &curve_indices_buffer,
            curves: &curves_buffer,
            glyph_data: &glyph_data_buffer,
            char_instances: &char_instances_buffer,
            char_grid_cells: &char_grid_cells_buffer,
            char_grid_indices: &char_grid_indices_buffer,
            char_grid_distances: Some(&char_grid_dist_buffer),
        });
        let host = World3dStorageHost::new(
            ctx,
            "TextShadow Demo",
            &shader_module,
            &uniforms,
            &storage_bindings,
        )?;

        Ok(Self {
            host,
            char_count,
            text_aabb,
            char_grid_params,
            char_grid_bounds,
        })
    }

    pub fn update_uniforms(&self, queue: &wgpu::Queue, camera: &FlyCamera, time: f32) {
        let uniforms = text_shadow_uniforms(
            camera,
            self.host.width(),
            self.host.height(),
            time,
            self.char_count,
            self.text_aabb,
            self.char_grid_params,
            self.char_grid_bounds,
        );
        self.host.write_uniforms(queue, &uniforms);
    }
}

impl Demo for TextShadowDemo {
    fn name(&self) -> &'static str {
        "Text Shadow"
    }

    fn id(&self) -> DemoId {
        DemoId::TextShadow
    }

    fn demo_type(&self) -> DemoType {
        DemoType::World3D
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        KEYBINDINGS_3D
    }

    fn camera_config(&self) -> CameraConfig {
        CameraConfig {
            initial_position: glam::Vec3::new(0.0, 0.0, 4.5),
            look_at_target: glam::Vec3::new(0.0, 0.0, 0.0),
        }
    }

    fn update(&mut self, _dt: f32, _camera: &mut FlyCamera) {
        // No updates needed
    }

    fn update_camera_uniforms(&self, queue: &wgpu::Queue, camera: &FlyCamera, time: f32) {
        self.update_uniforms(queue, camera, time);
    }

    fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        _queue: &wgpu::Queue,
        _time: f32,
    ) {
        self.host.render(render_pass);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.host.resize(width, height);
    }
}
