use crate::demo_core::DemoContext;
use crate::demos::gpu_runtime_common::{
    create_fullscreen_pipeline, uniform_bind_group_layout_entry,
};
use anyhow::Result;
use bytemuck::Pod;
use wgpu::util::DeviceExt;

struct World3dHostCore<U: Pod> {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    _marker: std::marker::PhantomData<U>,
}

pub struct World3dStorageBinding<'a> {
    pub binding: u32,
    pub visibility: wgpu::ShaderStages,
    pub read_only: bool,
    pub buffer: &'a wgpu::Buffer,
}

pub struct VectorTextStorageBuffers<'a> {
    pub curves: &'a wgpu::Buffer,
    pub glyph_data: &'a wgpu::Buffer,
    pub char_instances: &'a wgpu::Buffer,
    pub char_grid_cells: &'a wgpu::Buffer,
    pub char_grid_indices: &'a wgpu::Buffer,
    pub char_grid_distances: Option<&'a wgpu::Buffer>,
}

pub struct World3dUniformHost<U: Pod>(World3dHostCore<U>);

pub struct World3dStorageHost<U: Pod>(World3dHostCore<U>);

pub fn vector_text_storage_bindings<'a>(
    buffers: VectorTextStorageBuffers<'a>,
) -> Vec<World3dStorageBinding<'a>> {
    let mut bindings = vec![
        World3dStorageBinding {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            read_only: true,
            buffer: buffers.curves,
        },
        World3dStorageBinding {
            binding: 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            read_only: true,
            buffer: buffers.glyph_data,
        },
        World3dStorageBinding {
            binding: 3,
            visibility: wgpu::ShaderStages::FRAGMENT,
            read_only: true,
            buffer: buffers.char_instances,
        },
        World3dStorageBinding {
            binding: 4,
            visibility: wgpu::ShaderStages::FRAGMENT,
            read_only: true,
            buffer: buffers.char_grid_cells,
        },
        World3dStorageBinding {
            binding: 5,
            visibility: wgpu::ShaderStages::FRAGMENT,
            read_only: true,
            buffer: buffers.char_grid_indices,
        },
    ];
    if let Some(distances) = buffers.char_grid_distances {
        bindings.push(World3dStorageBinding {
            binding: 6,
            visibility: wgpu::ShaderStages::FRAGMENT,
            read_only: true,
            buffer: distances,
        });
    }
    bindings
}

fn create_uniform_buffer<U: Pod>(
    device: &wgpu::Device,
    label: &str,
    initial_uniforms: &U,
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("{label} Uniform Buffer")),
        contents: bytemuck::bytes_of(initial_uniforms),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

fn create_world3d_bind_group_layout(
    device: &wgpu::Device,
    label: &str,
    uniform_min_size: std::num::NonZeroU64,
    extra_entries: &[wgpu::BindGroupLayoutEntry],
) -> wgpu::BindGroupLayout {
    let mut entries = vec![uniform_bind_group_layout_entry(
        0,
        wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        uniform_min_size,
    )];
    entries.extend_from_slice(extra_entries);
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(&format!("{label} Bind Group Layout")),
        entries: &entries,
    })
}

fn create_world3d_bind_group<'a>(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::BindGroupLayout,
    uniform_buffer: &'a wgpu::Buffer,
    extra_entries: &[wgpu::BindGroupEntry<'a>],
) -> wgpu::BindGroup {
    let mut entries = vec![wgpu::BindGroupEntry {
        binding: 0,
        resource: uniform_buffer.as_entire_binding(),
    }];
    entries.extend_from_slice(extra_entries);
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(&format!("{label} Bind Group")),
        layout,
        entries: &entries,
    })
}

impl<U: Pod> World3dHostCore<U> {
    fn new(
        ctx: &DemoContext,
        label: &str,
        shader_module: &wgpu::ShaderModule,
        initial_uniforms: &U,
        extra_layout_entries: &[wgpu::BindGroupLayoutEntry],
        extra_bind_group_entries: &[wgpu::BindGroupEntry<'_>],
    ) -> Result<Self> {
        let uniform_buffer = create_uniform_buffer(&ctx.device, label, initial_uniforms);
        let uniform_min_size = std::num::NonZeroU64::new(std::mem::size_of::<U>() as u64)
            .expect("uniform size must be non-zero");
        let bind_group_layout = create_world3d_bind_group_layout(
            &ctx.device,
            label,
            uniform_min_size,
            extra_layout_entries,
        );
        let bind_group = create_world3d_bind_group(
            &ctx.device,
            label,
            &bind_group_layout,
            &uniform_buffer,
            extra_bind_group_entries,
        );
        let pipeline = create_fullscreen_pipeline(
            &ctx.device,
            ctx.surface_format,
            &format!("{label} Pipeline"),
            &[&bind_group_layout],
            shader_module,
        );

        Ok(Self {
            pipeline,
            uniform_buffer,
            bind_group,
            width: ctx.width,
            height: ctx.height,
            _marker: std::marker::PhantomData,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    pub fn write_uniforms(&self, queue: &wgpu::Queue, uniforms: &U) {
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

impl<U: Pod> World3dUniformHost<U> {
    pub fn new(
        ctx: &DemoContext,
        label: &str,
        shader_module: &wgpu::ShaderModule,
        initial_uniforms: &U,
    ) -> Result<Self> {
        Ok(Self(World3dHostCore::new(
            ctx,
            label,
            shader_module,
            initial_uniforms,
            &[],
            &[],
        )?))
    }

    pub fn width(&self) -> u32 {
        self.0.width()
    }

    pub fn height(&self) -> u32 {
        self.0.height()
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.0.resize(width, height);
    }

    pub fn write_uniforms(&self, queue: &wgpu::Queue, uniforms: &U) {
        self.0.write_uniforms(queue, uniforms);
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.0.render(render_pass);
    }
}

impl<U: Pod> World3dStorageHost<U> {
    pub fn new(
        ctx: &DemoContext,
        label: &str,
        shader_module: &wgpu::ShaderModule,
        initial_uniforms: &U,
        storage_bindings: &[World3dStorageBinding<'_>],
    ) -> Result<Self> {
        let layout_entries = storage_bindings
            .iter()
            .map(|entry| wgpu::BindGroupLayoutEntry {
                binding: entry.binding,
                visibility: entry.visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage {
                        read_only: entry.read_only,
                    },
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(entry.buffer.size()),
                },
                count: None,
            })
            .collect::<Vec<_>>();
        let bind_group_entries = storage_bindings
            .iter()
            .map(|entry| wgpu::BindGroupEntry {
                binding: entry.binding,
                resource: entry.buffer.as_entire_binding(),
            })
            .collect::<Vec<_>>();
        Ok(Self(World3dHostCore::new(
            ctx,
            label,
            shader_module,
            initial_uniforms,
            &layout_entries,
            &bind_group_entries,
        )?))
    }

    pub fn width(&self) -> u32 {
        self.0.width()
    }

    pub fn height(&self) -> u32 {
        self.0.height()
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.0.resize(width, height);
    }

    pub fn write_uniforms(&self, queue: &wgpu::Queue, uniforms: &U) {
        self.0.write_uniforms(queue, uniforms);
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.0.render(render_pass);
    }
}
