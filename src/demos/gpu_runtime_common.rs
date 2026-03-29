use crate::shader_bindings::{frame_composite, present};
use crate::text::{CharGridCell, VectorFont, VectorFontAtlas};
use anyhow::{Context, Result};
use wgpu::util::DeviceExt;

pub type GpuBezierCurve = crate::ui2d_shader_bindings::BezierCurve_std430_0;
pub type GpuGlyphData = crate::ui2d_shader_bindings::GlyphData_std430_0;

pub struct VectorFontGpuData {
    pub curves: Vec<GpuBezierCurve>,
    pub glyph_data: Vec<GpuGlyphData>,
}

pub struct UiStorageBuffers {
    pub curves_buffer: wgpu::Buffer,
    pub glyph_data_buffer: wgpu::Buffer,
    pub char_instances_buffer: wgpu::Buffer,
    pub char_grid_cells_buffer: wgpu::Buffer,
    pub char_grid_indices_buffer: wgpu::Buffer,
    pub ui_primitives_buffer: wgpu::Buffer,
}

pub const ITALIC_CODEPOINT_OFFSET: u32 = 0x10000;
const EMPTY_BEZIER_CURVES: [GpuBezierCurve; 1] =
    [GpuBezierCurve::new([0.0; 4], [0.0; 4], [0.0; 4])];
const EMPTY_GLYPH_DATA: [GpuGlyphData; 1] = [GpuGlyphData::new([0.0; 4], [0; 4])];

pub fn load_shared_vector_font_atlas() -> Result<VectorFontAtlas> {
    let font_data = std::fs::read("assets/fonts/LiberationSans-Regular.ttf")
        .context("Failed to load Liberation Sans Regular font")?;
    let mut font = VectorFont::from_ttf(&font_data).map_err(|e| anyhow::anyhow!(e))?;

    let italic_data = std::fs::read("assets/fonts/LiberationSans-Italic.ttf")
        .context("Failed to load Liberation Sans Italic font")?;
    font.merge_from_ttf(&italic_data, ITALIC_CODEPOINT_OFFSET)
        .map_err(|e| anyhow::anyhow!(e))?;

    Ok(VectorFontAtlas::from_font(&font))
}

pub fn build_font_gpu_data(atlas: &VectorFontAtlas) -> VectorFontGpuData {
    let curves = atlas
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

    let glyph_data = atlas
        .glyph_list
        .iter()
        .map(|(_, entry)| {
            GpuGlyphData::new(entry.bounds, [entry.curve_offset, entry.curve_count, 0, 0])
        })
        .collect();

    VectorFontGpuData { curves, glyph_data }
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
    let curves_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Curves Buffer"),
        contents: bytemuck::cast_slice(if gpu_font_data.curves.is_empty() {
            &EMPTY_BEZIER_CURVES
        } else {
            &gpu_font_data.curves
        }),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let glyph_data_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Glyph Data Buffer"),
        contents: bytemuck::cast_slice(if gpu_font_data.glyph_data.is_empty() {
            &EMPTY_GLYPH_DATA
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
    let min_sizes =
        [
            std::num::NonZeroU64::new(std::mem::size_of::<GpuBezierCurve>() as u64)
                .expect("GpuBezierCurve must be non-zero"),
            std::num::NonZeroU64::new(std::mem::size_of::<GpuGlyphData>() as u64)
                .expect("GpuGlyphData must be non-zero"),
            std::num::NonZeroU64::new(
                std::mem::size_of::<crate::retained::text::GpuCharInstanceEx>() as u64,
            )
            .expect("GpuCharInstanceEx must be non-zero"),
            std::num::NonZeroU64::new(std::mem::size_of::<CharGridCell>() as u64)
                .expect("CharGridCell must be non-zero"),
            std::num::NonZeroU64::new(std::mem::size_of::<u32>() as u64)
                .expect("u32 must be non-zero"),
            std::num::NonZeroU64::new(
                std::mem::size_of::<crate::retained::ui::GpuUiPrimitive>() as u64
            )
            .expect("GpuUiPrimitive must be non-zero"),
        ];
    assert_eq!(
        bindings.len(),
        min_sizes.len(),
        "UI storage bindings must have 6 entries"
    );
    bindings
        .iter()
        .enumerate()
        .map(|(index, binding)| wgpu::BindGroupLayoutEntry {
            binding: *binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: Some(min_sizes[index]),
            },
            count: None,
        })
        .collect()
}

pub fn storage_bind_group_entries<'a>(
    buffers: &'a UiStorageBuffers,
    bindings: &[u32],
) -> Vec<wgpu::BindGroupEntry<'a>> {
    assert_eq!(bindings.len(), 6, "UI storage bindings must have 6 entries");
    bindings
        .iter()
        .enumerate()
        .map(|(index, binding)| wgpu::BindGroupEntry {
            binding: *binding,
            resource: match index {
                0 => buffers.curves_buffer.as_entire_binding(),
                1 => buffers.glyph_data_buffer.as_entire_binding(),
                2 => buffers.char_instances_buffer.as_entire_binding(),
                3 => buffers.char_grid_cells_buffer.as_entire_binding(),
                4 => buffers.char_grid_indices_buffer.as_entire_binding(),
                5 => buffers.ui_primitives_buffer.as_entire_binding(),
                _ => unreachable!(),
            },
        })
        .collect()
}

pub fn uniform_bind_group_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    min_binding_size: std::num::NonZeroU64,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: Some(min_binding_size),
        },
        count: None,
    }
}

pub fn create_bind_group_layout_with_storage(
    device: &wgpu::Device,
    label: &str,
    uniform_entries: &[(u32, wgpu::ShaderStages, std::num::NonZeroU64)],
    storage_bindings: &[u32],
    storage_visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayout {
    let mut entries = uniform_entries
        .iter()
        .map(|(binding, visibility, min_binding_size)| {
            uniform_bind_group_layout_entry(*binding, *visibility, *min_binding_size)
        })
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

pub const PRESENT_INTERMEDIATE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub struct PresentHost {
    scene_texture: wgpu::Texture,
    scene_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
    bind_group: present::WgpuBindGroup0,
    pipeline: wgpu::RenderPipeline,
    surface_format: wgpu::TextureFormat,
    width: u32,
    height: u32,
}

pub struct TextureCompositeHost {
    sampler: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
}

impl PresentHost {
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        surface_format: wgpu::TextureFormat,
        label: &str,
    ) -> Self {
        let shader = present::create_shader_module_embed_source(device);
        let pipeline_layout = present::create_pipeline_layout(device);
        let vertex_entry = present::vs_main_entry();
        let fragment_entry = present::fs_main_entry([Some(wgpu::ColorTargetState {
            format: surface_format,
            blend: Some(wgpu::BlendState::REPLACE),
            write_mask: wgpu::ColorWrites::ALL,
        })]);
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("{label} Present Pipeline")),
            layout: Some(&pipeline_layout),
            vertex: present::vertex_state(&shader, &vertex_entry),
            fragment: Some(present::fragment_state(&shader, &fragment_entry)),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(&format!("{label} Present Sampler")),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label} Present Uniform Buffer")),
            contents: bytemuck::bytes_of(&present::Uniforms_std140_0::new([
                surface_format.is_srgb() as u32,
                0,
                0,
                0,
            ])),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let (scene_texture, scene_view) =
            Self::create_scene_target(device, width.max(1), height.max(1), label);
        let bind_group = present::WgpuBindGroup0::from_bindings(
            device,
            present::WgpuBindGroup0Entries::new(present::WgpuBindGroup0EntriesParams {
                sceneTexture_0: &scene_view,
                sceneSampler_0: &sampler,
                uniforms_0: uniform_buffer.as_entire_buffer_binding(),
            }),
        );

        Self {
            scene_texture,
            scene_view,
            sampler,
            uniform_buffer,
            bind_group,
            pipeline,
            surface_format,
            width: width.max(1),
            height: height.max(1),
        }
    }

    fn create_scene_target(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        label: &str,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let scene_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("{label} Present Scene Texture")),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: PRESENT_INTERMEDIATE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let scene_view = scene_texture.create_view(&wgpu::TextureViewDescriptor::default());
        (scene_texture, scene_view)
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32, label: &str) {
        let width = width.max(1);
        let height = height.max(1);
        if self.width == width && self.height == height {
            return;
        }
        let (scene_texture, scene_view) = Self::create_scene_target(device, width, height, label);
        self.scene_texture = scene_texture;
        self.scene_view = scene_view;
        self.bind_group = present::WgpuBindGroup0::from_bindings(
            device,
            present::WgpuBindGroup0Entries::new(present::WgpuBindGroup0EntriesParams {
                sceneTexture_0: &self.scene_view,
                sceneSampler_0: &self.sampler,
                uniforms_0: self.uniform_buffer.as_entire_buffer_binding(),
            }),
        );
        self.width = width;
        self.height = height;
    }

    pub fn update_surface_format(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        label: &str,
    ) {
        if self.surface_format != surface_format {
            let shader = present::create_shader_module_embed_source(device);
            let pipeline_layout = present::create_pipeline_layout(device);
            let vertex_entry = present::vs_main_entry();
            let fragment_entry = present::fs_main_entry([Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })]);
            self.pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&format!("{label} Present Pipeline")),
                layout: Some(&pipeline_layout),
                vertex: present::vertex_state(&shader, &vertex_entry),
                fragment: Some(present::fragment_state(&shader, &fragment_entry)),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });
            self.surface_format = surface_format;
        }
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&present::Uniforms_std140_0::new([
                surface_format.is_srgb() as u32,
                0,
                0,
                0,
            ])),
        );
    }

    pub fn scene_texture(&self) -> &wgpu::Texture {
        &self.scene_texture
    }

    pub fn scene_view(&self) -> &wgpu::TextureView {
        &self.scene_view
    }

    pub fn size(&self) -> [u32; 2] {
        [self.width, self.height]
    }

    pub fn encode_present_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Present Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render_pass.set_pipeline(&self.pipeline);
        self.bind_group.set(&mut render_pass);
        render_pass.draw(0..3, 0..1);
    }
}

impl TextureCompositeHost {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat, label: &str) -> Self {
        let shader = frame_composite::create_shader_module_embed_source(device);
        let pipeline_layout = frame_composite::create_pipeline_layout(device);
        let vertex_entry = frame_composite::vs_main_entry();
        let fragment_entry = frame_composite::fs_main_entry([Some(wgpu::ColorTargetState {
            format: surface_format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })]);
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("{label} Composite Pipeline")),
            layout: Some(&pipeline_layout),
            vertex: frame_composite::vertex_state(&shader, &vertex_entry),
            fragment: Some(frame_composite::fragment_state(&shader, &fragment_entry)),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(&format!("{label} Composite Sampler")),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label} Composite Uniform Buffer")),
            contents: bytemuck::bytes_of(&frame_composite::Uniforms_std140_0::new(
                [1.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 1.0],
                [0.0, 0.0, 1.0, 1.0],
            )),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        Self {
            sampler,
            uniform_buffer,
            pipeline,
        }
    }

    pub fn encode_pass(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        label: &str,
        source_view: &wgpu::TextureView,
        target_view: &wgpu::TextureView,
        load: wgpu::LoadOp<wgpu::Color>,
        target_size: [u32; 2],
        source_rect: [f32; 4],
        target_rect: [f32; 4],
    ) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&frame_composite::Uniforms_std140_0::new(
                [target_size[0] as f32, target_size[1] as f32, 0.0, 0.0],
                source_rect,
                target_rect,
            )),
        );
        let bind_group = frame_composite::WgpuBindGroup0::from_bindings(
            device,
            frame_composite::WgpuBindGroup0Entries::new(
                frame_composite::WgpuBindGroup0EntriesParams {
                    sourceTexture_0: source_view,
                    sourceSampler_0: &self.sampler,
                    uniforms_0: self.uniform_buffer.as_entire_buffer_binding(),
                },
            ),
        );
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render_pass.set_pipeline(&self.pipeline);
        bind_group.set(&mut render_pass);
        render_pass.draw(0..3, 0..1);
    }
}
