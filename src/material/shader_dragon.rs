use glam::Mat4;
use std::borrow::Cow;
use std::f32::consts::PI;
use std::mem::size_of;
use std::time::Instant;
use wgpu::util::{align_to, BufferInitDescriptor, DeviceExt};
use wgpu::{
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferAddress, BufferBinding,
    BufferBindingType, BufferDescriptor, BufferSize, BufferUsages, CompareFunction, DepthBiasState,
    DepthStencilState, DynamicOffset, Extent3d, Face, FilterMode, FragmentState, FrontFace,
    MultisampleState, PipelineLayoutDescriptor, PrimitiveState, Queue, RenderPass, RenderPipeline,
    RenderPipelineDescriptor, SamplerDescriptor, ShaderModule, ShaderStages, StencilState,
    TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor, VertexState,
};

use crate::geometry::Vertex;
use crate::material::Shader;
use crate::world::{Light, Renderer};

const MAX_ENTITY: u64 = 100000;
const MAX_LIGHT: u64 = 10;
pub struct ShaderDragon {
    pub module: ShaderModule,
    pub render_pipeline: RenderPipeline,
    pub bind_group_camera: BindGroup,
    pub bind_group_node: BindGroup,
    pub vp_buffer: Buffer,
    pub w_buffer: Buffer,
    pub r_buffer: Buffer,
    pub displacement_offset_buffer: Buffer,
    pub light_buffer: Buffer,
    pub light_count_buffer: Buffer,
}
impl ShaderDragon {
    pub fn new(renderer: &Renderer) -> Self {
        let device = &renderer.device;
        let new_shader_timestamp = Instant::now();
        let bind_group_layout_camera =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0, // view projection
                        visibility: ShaderStages::VERTEX,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: BufferSize::new(size_of::<Mat4>() as u64),
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1, // light
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: BufferSize::new(0),
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 2, // light count
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: BufferSize::new(size_of::<usize>() as u64),
                        },
                        count: None,
                    },
                ],
            });
        let bind_group_layout_node = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0, // world
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: BufferSize::new(size_of::<Mat4>() as u64),
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1, // rotation
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: BufferSize::new(size_of::<Mat4>() as u64),
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2, // displacement texture
                    visibility: ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3, // displacement sampler
                    visibility: ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 4, // displacement offset
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: BufferSize::new(size_of::<f32>() as u64),
                    },
                    count: None,
                },
            ],
        });
        let create_texels = |size| {
            let mut ret = Vec::new();
            const MAX_LEN: f32 = PI * 2.0;
            let len_to_rad = |x| (x * MAX_LEN / size as f32);
            let len_to_col = |x| (((x / MAX_LEN + 1.0) * 128.0) as u8);
            for i in 0..size {
                let i = len_to_rad(i as f32);
                ret.push(len_to_col(i.cos()));
                ret.push(0);
                ret.push(len_to_col(i.sin()));
                ret.push(0);
            }
            ret
        };
        let size = 128u32;
        let texels = create_texels(size);
        // println!("{:?}", texels);
        let texture_extent = Extent3d {
            width: size,
            height: 1,
            depth_or_array_layers: 1,
        };
        let displacement_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: texture_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let displacement_texture_view =
            displacement_texture.create_view(&TextureViewDescriptor::default());
        renderer.queue.write_texture(
            displacement_texture.as_image_copy(),
            &texels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(size * 4),
                rows_per_image: None,
            },
            texture_extent,
        );
        let displacement_sampler = device.create_sampler(&SamplerDescriptor {
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });
        let displacement_offset_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Displacement Offset"),
            size: size_of::<f32>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout_node, &bind_group_layout_camera],
            push_constant_ranges: &[],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader_dragon.wgsl"))),
        });
        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &module,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(FragmentState {
                module: &module,
                entry_point: "fs_main",
                targets: &[Some(renderer.config.format.into())],
            }),
            primitive: PrimitiveState {
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: CompareFunction::Less,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState::default(),
            multiview: None,
        });
        let vp_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Camera View Projection Buffer"),
            contents: bytemuck::cast_slice(Mat4::IDENTITY.as_ref()),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        let light_uniform_size = size_of::<Light>() as BufferAddress;
        let light_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Light Buffer"),
            size: MAX_LIGHT as BufferAddress * light_uniform_size,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let light_count_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Light Count"),
            size: size_of::<usize>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_camera = device.create_bind_group(&BindGroupDescriptor {
            layout: &bind_group_layout_camera,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: vp_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: light_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: light_count_buffer.as_entire_binding(),
                },
            ],
            label: None,
        });
        let node_uniform_size = size_of::<Mat4>() as BufferAddress;
        let node_uniform_aligned = {
            let alignment = device.limits().min_uniform_buffer_offset_alignment as BufferAddress;
            align_to(node_uniform_size, alignment)
        };
        let w_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Model world transform buffer"),
            size: MAX_ENTITY as BufferAddress * node_uniform_aligned,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let r_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Model rotation buffer"),
            size: MAX_ENTITY as BufferAddress * node_uniform_aligned,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_node = device.create_bind_group(&BindGroupDescriptor {
            layout: &bind_group_layout_node,
            entries: &[
                BindGroupEntry {
                    binding: 0, // world transform
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &w_buffer,
                        offset: 0,
                        size: BufferSize::new(node_uniform_size),
                    }),
                },
                BindGroupEntry {
                    binding: 1, // rotation
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &r_buffer,
                        offset: 0,
                        size: BufferSize::new(node_uniform_size),
                    }),
                },
                BindGroupEntry {
                    binding: 2, // displacement texture
                    resource: BindingResource::TextureView(&displacement_texture_view),
                },
                BindGroupEntry {
                    binding: 3, // sampler
                    resource: BindingResource::Sampler(&displacement_sampler),
                },
                BindGroupEntry {
                    binding: 4, // offset
                    resource: displacement_offset_buffer.as_entire_binding(),
                },
            ],
            label: None,
        });
        println!("created shader in {:?}", new_shader_timestamp.elapsed());
        Self {
            module,
            render_pipeline,
            bind_group_camera,
            bind_group_node,
            vp_buffer,
            w_buffer,
            r_buffer,
            displacement_offset_buffer,
            light_buffer,
            light_count_buffer,
        }
    }
}
impl Shader for ShaderDragon {
    fn set_pipeline<'a>(&'a self, pass: &mut RenderPass<'a>, offset: BufferAddress) {
        let offsets = [offset as DynamicOffset, offset as DynamicOffset];
        pass.set_bind_group(0, &self.bind_group_node, &offsets);
        pass.set_bind_group(1, &self.bind_group_camera, &[]);
        pass.set_pipeline(&self.render_pipeline);
    }
    fn write_transform_data(&self, queue: &Queue, offset: BufferAddress, matrix: &[f32; 16]) {
        queue.write_buffer(&self.w_buffer, offset, bytemuck::bytes_of(matrix));
    }
    fn write_rotation_data(&self, queue: &Queue, offset: BufferAddress, matrix: &[f32; 16]) {
        queue.write_buffer(&self.r_buffer, offset, bytemuck::bytes_of(matrix));
    }
    fn write_time_data(&self, queue: &Queue, time: f32) {
        queue.write_buffer(
            &self.displacement_offset_buffer,
            0,
            bytemuck::bytes_of(&(time * 2.5)),
        );
    }
    fn write_camera_data(&self, queue: &Queue, matrix: &[f32; 16]) {
        queue.write_buffer(&self.vp_buffer, 0, bytemuck::bytes_of(matrix));
    }
    fn write_light_data(&self, queue: &Queue, lights: &Vec<Light>) {
        queue.write_buffer(
            &self.light_count_buffer,
            0,
            bytemuck::bytes_of(&lights.len()),
        );
        queue.write_buffer(&self.light_buffer, 0, bytemuck::cast_slice(lights));
    }
}