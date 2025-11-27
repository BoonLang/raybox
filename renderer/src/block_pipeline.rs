use wgpu::util::DeviceExt;

/// Per-instance data for a block (axis-aligned box scaled from unit cube).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BlockInstance {
    pub rect: [f32; 4],  // x, y, w, h in px
    pub depth: f32,      // extrusion in px
    pub elevation: f32,  // z offset in px
    pub color: [f32; 4], // RGBA 0..1
}

impl BlockInstance {
    pub fn new(
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        depth: f32,
        elevation: f32,
        color: [f32; 4],
    ) -> Self {
        Self {
            rect: [x, y, w, h],
            depth,
            elevation,
            color,
        }
    }

    pub fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<BlockInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                }, // rect
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as u64,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32,
                }, // depth
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 5]>() as u64,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32,
                }, // elevation
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as u64,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                }, // color
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Globals {
    pub screen_size: [f32; 2],
    pub light_dir_deg: [f32; 2], // azimuth, altitude
    pub ambient: f32,
    pub add_rim: f32,
    pub ao_strength: f32,
    pub _pad: f32,
}

pub struct BlockPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    pub globals_buf: wgpu::Buffer,
    pub globals_bind_group: wgpu::BindGroup,
}

impl BlockPipeline {
    pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> Self {
        // Unit cube vertices with normals
        #[rustfmt::skip]
        let vertices: &[f32] = &[
            // pos xyz, normal xyz
            // top (z=1)
            0.0,0.0,1.0, 0.0,0.0,1.0,
            1.0,0.0,1.0, 0.0,0.0,1.0,
            1.0,1.0,1.0, 0.0,0.0,1.0,
            0.0,1.0,1.0, 0.0,0.0,1.0,
            // bottom (z=0)
            0.0,0.0,0.0, 0.0,0.0,-1.0,
            1.0,0.0,0.0, 0.0,0.0,-1.0,
            1.0,1.0,0.0, 0.0,0.0,-1.0,
            0.0,1.0,0.0, 0.0,0.0,-1.0,
            // front (y=0)
            0.0,0.0,0.0, 0.0,-1.0,0.0,
            1.0,0.0,0.0, 0.0,-1.0,0.0,
            1.0,0.0,1.0, 0.0,-1.0,0.0,
            0.0,0.0,1.0, 0.0,-1.0,0.0,
            // back (y=1)
            0.0,1.0,0.0, 0.0,1.0,0.0,
            1.0,1.0,0.0, 0.0,1.0,0.0,
            1.0,1.0,1.0, 0.0,1.0,0.0,
            0.0,1.0,1.0, 0.0,1.0,0.0,
            // left (x=0)
            0.0,0.0,0.0, -1.0,0.0,0.0,
            0.0,1.0,0.0, -1.0,0.0,0.0,
            0.0,1.0,1.0, -1.0,0.0,0.0,
            0.0,0.0,1.0, -1.0,0.0,0.0,
            // right (x=1)
            1.0,0.0,0.0, 1.0,0.0,0.0,
            1.0,1.0,0.0, 1.0,0.0,0.0,
            1.0,1.0,1.0, 1.0,0.0,0.0,
            1.0,0.0,1.0, 1.0,0.0,0.0,
        ];
        let indices: &[u16] = &[
            0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7, 8, 9, 10, 8, 10, 11, 12, 13, 14, 12, 14, 15, 16,
            17, 18, 16, 18, 19, 20, 21, 22, 20, 22, 23,
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("block vertices"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("block indices"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let globals = Globals {
            screen_size: [config.width as f32, config.height as f32],
            light_dir_deg: [135.0, 60.0],
            ambient: 1.0, // flat lighting to match 2D reference
            add_rim: 0.10,
            ao_strength: 0.08,
            _pad: 0.0,
        };
        let globals_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("block globals"),
            contents: bytemuck::bytes_of(&globals),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("block globals layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("block globals bind group"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buf.as_entire_binding(),
            }],
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("block_shader.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("block pipeline layout"),
            bind_group_layouts: &[&globals_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("block pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: 6 * 4,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: wgpu::VertexFormat::Float32x3,
                            },
                            wgpu::VertexAttribute {
                                offset: 3 * 4,
                                shader_location: 1,
                                format: wgpu::VertexFormat::Float32x3,
                            },
                        ],
                    },
                    BlockInstance::layout(),
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            globals_buf,
            globals_bind_group,
        }
    }

    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        instances: &[BlockInstance],
    ) {
        if instances.is_empty() {
            return;
        }

        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("block instances"),
            contents: bytemuck::cast_slice(instances),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("block render encoder"),
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("block pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.globals_bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_vertex_buffer(1, instance_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..36, 0, 0..instances.len() as u32);
        }

        queue.submit(std::iter::once(encoder.finish()));
    }
}
