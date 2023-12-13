use crate::geometry::Vertex;
use crate::material::Shader;
use crate::world::{Light, Renderer, MAX_ENTITY, MAX_LIGHT};
use glam::{Mat4, Quat, Vec3};
use splines::{Interpolation, Key, Spline};
use std::borrow::Cow;
use std::mem::size_of;
use std::time::Instant;
use wgpu::util::{align_to, BufferInitDescriptor, DeviceExt};
use wgpu::{
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferAddress, BufferBinding,
    BufferBindingType, BufferDescriptor, BufferSize, BufferUsages, CompareFunction, DepthBiasState,
    DepthStencilState, DynamicOffset, Extent3d, Face, FilterMode, FragmentState, FrontFace,
    ImageDataLayout, MultisampleState, PipelineLayoutDescriptor, PrimitiveState, Queue, RenderPass,
    RenderPipeline, RenderPipelineDescriptor, SamplerBindingType, SamplerDescriptor, ShaderModule,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, StencilState, TextureDescriptor,
    TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureViewDescriptor,
    TextureViewDimension, VertexState,
};

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
                    ty: BindingType::Texture {
                        multisampled: false,
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3, // displacement sampler
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
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
            // infinity symbol oo, span from -3 -> 3
            let points: Vec<Vec3> = vec![
                Vec3::new(-2.0, 0.0, -1.0),
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(2.0, 0.0, 1.0),
                Vec3::new(3.0, 0.0, 0.0),
                Vec3::new(2.0, 0.0, -1.0),
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(-2.0, 0.0, 1.0),
                Vec3::new(-3.0, 0.0, 0.0),
                Vec3::new(-2.0, 0.0, -1.0),
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(2.0, 0.0, 1.0),
            ];
            let n = points.len();
            let points = points
                .into_iter()
                .enumerate()
                .map(|(i, v)| {
                    let k = (i as f32 - 1.0) / (n - 3) as f32;
                    Key::new(k, v, Interpolation::CatmullRom)
                })
                .collect();
            let spline = Spline::from_vec(points);
            let mut displacements = Vec::new();
            let mut normals = Vec::new();
            let mut binormals = Vec::new();
            for i in 0..size {
                let t1 = i as f32 / (size - 1) as f32;
                let t2 = ((i + 1) % size) as f32 / (n - 1) as f32;
                let p1 = spline.clamped_sample(t1).unwrap_or_default();
                let p2 = spline.clamped_sample(t2).unwrap_or_default();
                let rotation = Quat::from_rotation_arc(Vec3::X, (p2 - p1).normalize());
                let normal = (rotation * Vec3::Y).normalize();
                let binormal = (rotation * Vec3::Z).normalize();
                displacements.push(p1.x);
                displacements.push(p1.y);
                displacements.push(p1.z);
                displacements.push(0.0);
                normals.push(normal.x);
                normals.push(normal.y);
                normals.push(normal.z);
                normals.push(0.0);
                binormals.push(binormal.x);
                binormals.push(binormal.y);
                binormals.push(binormal.z);
                binormals.push(0.0);
            }
            displacements
                .into_iter()
                .map(|v| (v / 3.0 * 128.0) as i8)
                .chain(normals.into_iter().map(|v| (v * 128.0) as i8))
                .chain(binormals.into_iter().map(|v| (v * 128.0) as i8))
                .collect()
        };
        let size = 32;
        let texels: Vec<i8> = create_texels(size);
        // println!("{:?}", texels);
        let texture_extent = Extent3d {
            width: size,
            height: 3,
            depth_or_array_layers: 1,
        };
        let displacement_texture = device.create_texture(&TextureDescriptor {
            label: None,
            size: texture_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Snorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        renderer.queue.write_texture(
            displacement_texture.as_image_copy(),
            bytemuck::cast_slice(&texels),
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(size * 4),
                rows_per_image: None,
            },
            texture_extent,
        );
        let displacement_sampler = device.create_sampler(&SamplerDescriptor {
            address_mode_u: AddressMode::Repeat,
            mag_filter: FilterMode::Linear,
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
        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader_dragon.wgsl"))),
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
                    resource: BindingResource::TextureView(
                        &displacement_texture.create_view(&TextureViewDescriptor::default()),
                    ),
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
            bytemuck::bytes_of(&(time * 50.5)),
        );
    }
    fn write_camera_data(&self, queue: &Queue, matrix: &[f32; 16]) {
        queue.write_buffer(&self.vp_buffer, 0, bytemuck::bytes_of(matrix));
    }
    fn write_light_data(&self, queue: &Queue, lights: &[Light]) {
        queue.write_buffer(
            &self.light_count_buffer,
            0,
            bytemuck::bytes_of(&lights.len()),
        );
        queue.write_buffer(&self.light_buffer, 0, bytemuck::cast_slice(lights));
    }
}
