//! Demo 5: 3D Clay Tablet with Vector SDF Relief Text
//!
//! Text carved into a clay slab using exact Bezier SDF.

use super::{
    world3d_runtime::{vector_text_storage_bindings, VectorTextStorageBuffers, World3dStorageHost},
    Demo, DemoContext, DemoId, DemoType, KEYBINDINGS_3D,
};
use crate::camera::FlyCamera;
use crate::input::CameraConfig;
use crate::shader_bindings::sdf_clay_vector;
use crate::text::{build_char_grid, VectorFont, VectorFontAtlas};
use anyhow::{Context, Result};
use wgpu::util::DeviceExt;

const LOREM: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum. Curabitur pretium tincidunt lacus. Nulla gravida orci a odio. Nullam varius, turpis et commodo pharetra, est eros bibendum elit, nec luctus magna felis sollicitudin mauris. Integer in mauris eu nibh euismod gravida. Duis ac tellus et risus vulputate vehicula. Donec lobortis risus a elit. Etiam tempor. Ut ullamcorper, ligula eu tempor congue, eros est euismod turpis, id tincidunt sapien risus a quam. Maecenas fermentum consequat mi. Donec fermentum. Pellentesque malesuada nulla a mi. Duis sapien sem, aliquet sed, vulputate eget, feugiat non, orci. Sed neque. Sed eget lacus. Mauris non dui nec urna suscipit nonummy. Fusce fermentum fermentum arcu. Vestibulum ante ipsum primis in faucibus orci luctus et ultrices posuere cubilia curae.";

type GpuBezierCurve = sdf_clay_vector::BezierCurve_std430_0;
type GpuGlyphData = sdf_clay_vector::GlyphData_std430_0;
type GpuCharInstance = sdf_clay_vector::CharInstance_std430_0;
type ClayUniforms = sdf_clay_vector::Uniforms_std140_0;

fn clay_uniforms(
    camera: &FlyCamera,
    width: u32,
    height: u32,
    time: f32,
    char_count: u32,
    char_grid_params: [f32; 4],
    char_grid_bounds: [f32; 4],
) -> ClayUniforms {
    let aspect = width as f32 / height as f32;
    let position = camera.position();
    ClayUniforms::new(
        sdf_clay_vector::_MatrixStorage_float4x4_ColMajorstd140_0::new(
            camera.inv_view_projection_matrix(aspect).to_cols_array_2d(),
        ),
        [position.x, position.y, position.z, time],
        [0.5, 0.8, 0.3, 1.5],
        [width as f32, height as f32, 0.2, 1.0],
        [char_count as f32, 0.0, 0.4, 0.0],
        char_grid_params,
        char_grid_bounds,
    )
}

fn build_clay_text_layout(
    atlas: &VectorFontAtlas,
    plaque_half_width: f32,
    plaque_half_height: f32,
) -> Vec<GpuCharInstance> {
    let mut instances = Vec::new();

    let full_text = format!(
        "RAYBOX SDF TEXT ENGINE\n\n{}",
        format!(
            "{} {} {} {} {} {}",
            LOREM, LOREM, LOREM, LOREM, LOREM, LOREM
        )
    );

    let scale = 0.15;
    let line_height = 0.22;
    let margin = 0.15;

    let start_x = -plaque_half_width + margin;
    let start_y = plaque_half_height - margin;
    let max_x = plaque_half_width - margin;

    let mut x = start_x;
    let mut y = start_y;
    let mut line_num = 0;
    let max_lines = 24;

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
            x += 0.08 * scale;
            if x > max_x {
                x = start_x;
                y -= line_height;
                line_num += 1;
            }
        }
    }

    instances
}

pub struct ClayDemo {
    host: World3dStorageHost<ClayUniforms>,
    char_count: u32,
    char_grid_params: [f32; 4],
    char_grid_bounds: [f32; 4],
}

impl ClayDemo {
    pub fn new(ctx: &DemoContext) -> Result<Self> {
        // Load vector font
        let font_data =
            std::fs::read("assets/fonts/DejaVuSans.ttf").context("Failed to load font file")?;
        let font = VectorFont::from_ttf(&font_data).map_err(|e| anyhow::anyhow!(e))?;
        let atlas = VectorFontAtlas::from_font(&font);

        // Build text layout
        let char_instances = build_clay_text_layout(&atlas, 3.3, 2.3);
        let char_count = char_instances.len() as u32;

        // Build character spatial grid
        let instance_data: Vec<[f32; 4]> = char_instances.iter().map(|c| c.posAndChar_0).collect();
        let char_grid = build_char_grid(&instance_data, &atlas, [80, 48]);

        let char_grid_params = [
            char_grid.dims[0] as f32,
            char_grid.dims[1] as f32,
            char_grid.cell_size[0],
            char_grid.cell_size[1],
        ];
        let char_grid_bounds = char_grid.bounds;

        // Prepare GPU data
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
                GpuGlyphData::new(entry.bounds, [entry.curve_offset, entry.curve_count, 0, 0])
            })
            .collect();
        let empty_curve = [GpuBezierCurve::new([0.0; 4], [0.0; 4], [0.0; 4])];
        let empty_glyph = [GpuGlyphData::new([0.0; 4], [0; 4])];
        let empty_char_instance = [GpuCharInstance::new([0.0; 4])];

        let uniforms = clay_uniforms(
            &FlyCamera::default(),
            ctx.width,
            ctx.height,
            0.0,
            char_count,
            char_grid_params,
            char_grid_bounds,
        );

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

        let shader_module = sdf_clay_vector::create_shader_module_embed_source(ctx.device);
        let storage_bindings = vector_text_storage_bindings(VectorTextStorageBuffers {
            curves: &curves_buffer,
            glyph_data: &glyph_data_buffer,
            char_instances: &char_instances_buffer,
            char_grid_cells: &char_grid_cells_buffer,
            char_grid_indices: &char_grid_indices_buffer,
            char_grid_distances: None,
        });
        let host = World3dStorageHost::new(
            ctx,
            "Clay Demo",
            &shader_module,
            &uniforms,
            &storage_bindings,
        )?;

        Ok(Self {
            host,
            char_count,
            char_grid_params,
            char_grid_bounds,
        })
    }

    pub fn update_uniforms(&self, queue: &wgpu::Queue, camera: &FlyCamera, time: f32) {
        let uniforms = clay_uniforms(
            camera,
            self.host.width(),
            self.host.height(),
            time,
            self.char_count,
            self.char_grid_params,
            self.char_grid_bounds,
        );
        self.host.write_uniforms(queue, &uniforms);
    }
}

impl Demo for ClayDemo {
    fn name(&self) -> &'static str {
        "Clay Tablet"
    }

    fn id(&self) -> DemoId {
        DemoId::Clay
    }

    fn demo_type(&self) -> DemoType {
        DemoType::World3D
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        KEYBINDINGS_3D
    }

    fn camera_config(&self) -> CameraConfig {
        CameraConfig {
            initial_position: glam::Vec3::new(0.0, 0.0, 7.5),
            look_at_target: glam::Vec3::new(0.0, 0.0, 0.3),
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
