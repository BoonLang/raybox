use crate::text::{CharGridCell, VectorFont, VectorFontAtlas};
use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuGridCell {
    pub curve_start_and_count: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuBezierCurve {
    pub points01: [f32; 4],
    pub points2bbox: [f32; 4],
    pub bbox_flags: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuGlyphData {
    pub bounds: [f32; 4],
    pub grid_info: [u32; 4],
    pub curve_info: [u32; 4],
}

pub struct VectorFontGpuData {
    pub grid_cells: Vec<GpuGridCell>,
    pub curve_indices: Vec<u32>,
    pub curves: Vec<GpuBezierCurve>,
    pub glyph_data: Vec<GpuGlyphData>,
}

pub struct UiStorageBuffers {
    pub grid_cells_buffer: wgpu::Buffer,
    pub curve_indices_buffer: wgpu::Buffer,
    pub curves_buffer: wgpu::Buffer,
    pub glyph_data_buffer: wgpu::Buffer,
    pub char_instances_buffer: wgpu::Buffer,
    pub char_grid_cells_buffer: wgpu::Buffer,
    pub char_grid_indices_buffer: wgpu::Buffer,
    pub ui_primitives_buffer: wgpu::Buffer,
}

pub const ITALIC_CODEPOINT_OFFSET: u32 = 0x10000;

pub fn load_shared_vector_font_atlas() -> Result<VectorFontAtlas> {
    let font_data = std::fs::read("assets/fonts/LiberationSans-Regular.ttf")
        .context("Failed to load Liberation Sans Regular font")?;
    let mut font = VectorFont::from_ttf(&font_data).map_err(|e| anyhow::anyhow!(e))?;

    let italic_data = std::fs::read("assets/fonts/LiberationSans-Italic.ttf")
        .context("Failed to load Liberation Sans Italic font")?;
    font.merge_from_ttf(&italic_data, ITALIC_CODEPOINT_OFFSET)
        .map_err(|e| anyhow::anyhow!(e))?;

    Ok(VectorFontAtlas::from_font(&font, 32))
}

pub fn build_font_gpu_data(atlas: &VectorFontAtlas) -> VectorFontGpuData {
    let grid_cells = atlas
        .grid_cells
        .iter()
        .map(|c| GpuGridCell {
            curve_start_and_count: (c.curve_start as u32)
                | ((c.curve_count as u32) << 16)
                | ((c.flags as u32) << 24),
        })
        .collect();

    let curve_indices = atlas.curve_indices.iter().map(|&i| i as u32).collect();

    let curves = atlas
        .curves
        .iter()
        .map(|c| {
            let p0 = c.p0();
            let p1 = c.p1();
            let p2 = c.p2();
            GpuBezierCurve {
                points01: [p0.0, p0.1, p1.0, p1.1],
                points2bbox: [p2.0, p2.1, c.bbox[0], c.bbox[1]],
                bbox_flags: [c.bbox[2], c.bbox[3], c.flags as f32, 0.0],
            }
        })
        .collect();

    let glyph_data = atlas
        .glyph_list
        .iter()
        .map(|(_, entry)| GpuGlyphData {
            bounds: entry.bounds,
            grid_info: [entry.grid_offset, entry.grid_size[0], entry.grid_size[1], 0],
            curve_info: [entry.curve_offset, entry.curve_count, 0, 0],
        })
        .collect();

    VectorFontGpuData {
        grid_cells,
        curve_indices,
        curves,
        glyph_data,
    }
}

pub fn create_storage_buffers(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    gpu_font_data: &VectorFontGpuData,
    char_instances_bytes: &[u8],
    char_instances_capacity_bytes: usize,
    char_grid_cells: &[CharGridCell],
    char_grid_indices: &[u32],
    char_grid_index_capacity: usize,
    ui_primitives_bytes: &[u8],
    ui_primitive_capacity_bytes: usize,
    ui_primitives_label: &str,
) -> UiStorageBuffers {
    let grid_cells_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Grid Cells Buffer"),
        contents: bytemuck::cast_slice(if gpu_font_data.grid_cells.is_empty() {
            &[GpuGridCell {
                curve_start_and_count: 0,
            }]
        } else {
            &gpu_font_data.grid_cells
        }),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let curve_indices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Curve Indices Buffer"),
        contents: bytemuck::cast_slice(if gpu_font_data.curve_indices.is_empty() {
            &[0u32]
        } else {
            &gpu_font_data.curve_indices
        }),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let curves_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Curves Buffer"),
        contents: bytemuck::cast_slice(if gpu_font_data.curves.is_empty() {
            &[GpuBezierCurve {
                points01: [0.0; 4],
                points2bbox: [0.0; 4],
                bbox_flags: [0.0; 4],
            }]
        } else {
            &gpu_font_data.curves
        }),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let glyph_data_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Glyph Data Buffer"),
        contents: bytemuck::cast_slice(if gpu_font_data.glyph_data.is_empty() {
            &[GpuGlyphData {
                bounds: [0.0; 4],
                grid_info: [0; 4],
                curve_info: [0; 4],
            }]
        } else {
            &gpu_font_data.glyph_data
        }),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let char_instances_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Char Instances Buffer"),
        contents: &vec![0u8; char_instances_capacity_bytes],
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    queue.write_buffer(&char_instances_buffer, 0, char_instances_bytes);

    let char_grid_cells_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Char Grid Cells Buffer"),
        contents: bytemuck::cast_slice(char_grid_cells),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });

    let char_grid_indices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Char Grid Indices Buffer"),
        contents: &vec![0u8; char_grid_index_capacity * std::mem::size_of::<u32>()],
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    queue.write_buffer(
        &char_grid_indices_buffer,
        0,
        bytemuck::cast_slice(char_grid_indices),
    );

    let ui_primitives_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(ui_primitives_label),
        contents: &vec![0u8; ui_primitive_capacity_bytes],
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    queue.write_buffer(&ui_primitives_buffer, 0, ui_primitives_bytes);

    UiStorageBuffers {
        grid_cells_buffer,
        curve_indices_buffer,
        curves_buffer,
        glyph_data_buffer,
        char_instances_buffer,
        char_grid_cells_buffer,
        char_grid_indices_buffer,
        ui_primitives_buffer,
    }
}

pub fn storage_bind_group_layout_entries(
    bindings: &[u32],
    visibility: wgpu::ShaderStages,
) -> Vec<wgpu::BindGroupLayoutEntry> {
    bindings
        .iter()
        .map(|binding| wgpu::BindGroupLayoutEntry {
            binding: *binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        })
        .collect()
}

pub fn storage_bind_group_entries<'a>(
    buffers: &'a UiStorageBuffers,
    bindings: &[u32],
) -> Vec<wgpu::BindGroupEntry<'a>> {
    assert_eq!(bindings.len(), 8, "UI storage bindings must have 8 entries");
    bindings
        .iter()
        .enumerate()
        .map(|(index, binding)| wgpu::BindGroupEntry {
            binding: *binding,
            resource: match index {
                0 => buffers.grid_cells_buffer.as_entire_binding(),
                1 => buffers.curve_indices_buffer.as_entire_binding(),
                2 => buffers.curves_buffer.as_entire_binding(),
                3 => buffers.glyph_data_buffer.as_entire_binding(),
                4 => buffers.char_instances_buffer.as_entire_binding(),
                5 => buffers.char_grid_cells_buffer.as_entire_binding(),
                6 => buffers.char_grid_indices_buffer.as_entire_binding(),
                7 => buffers.ui_primitives_buffer.as_entire_binding(),
                _ => unreachable!(),
            },
        })
        .collect()
}

pub fn uniform_bind_group_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

pub fn create_bind_group_layout_with_storage(
    device: &wgpu::Device,
    label: &str,
    uniform_entries: &[(u32, wgpu::ShaderStages)],
    storage_bindings: &[u32],
    storage_visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayout {
    let mut entries = uniform_entries
        .iter()
        .map(|(binding, visibility)| uniform_bind_group_layout_entry(*binding, *visibility))
        .collect::<Vec<_>>();
    entries.extend(storage_bind_group_layout_entries(
        storage_bindings,
        storage_visibility,
    ));
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &entries,
    })
}

pub fn create_bind_group_with_storage<'a>(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::BindGroupLayout,
    uniform_buffers: &[(u32, &'a wgpu::Buffer)],
    storage_buffers: &'a UiStorageBuffers,
    storage_bindings: &[u32],
) -> wgpu::BindGroup {
    let mut entries = uniform_buffers
        .iter()
        .map(|(binding, buffer)| wgpu::BindGroupEntry {
            binding: *binding,
            resource: buffer.as_entire_binding(),
        })
        .collect::<Vec<_>>();
    entries.extend(storage_bind_group_entries(
        storage_buffers,
        storage_bindings,
    ));
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &entries,
    })
}

pub fn create_fullscreen_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    label: &str,
    bind_group_layouts: &[&wgpu::BindGroupLayout],
    shader_module: &wgpu::ShaderModule,
) -> wgpu::RenderPipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&format!("{label} Layout")),
        bind_group_layouts,
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
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
                format: surface_format,
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
