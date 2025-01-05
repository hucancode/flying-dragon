use crate::geometry::Vertex;
use crate::material::Shader;
use crate::world::{Light, Renderer, MAX_ENTITY, MAX_LIGHT};
use glam::{Mat4, Quat, Vec3};
use splines::{Interpolation, Key, Spline};
use std::borrow::Cow;
use std::mem::size_of;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;
use wgpu::util::{align_to, BufferInitDescriptor, DeviceExt};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferAddress, BufferBinding,
    BufferBindingType, BufferDescriptor, BufferSize, BufferUsages, CompareFunction, DepthBiasState,
    DepthStencilState, DynamicOffset, Face, FragmentState, FrontFace, MultisampleState,
    PipelineCompilationOptions, PipelineLayoutDescriptor, PrimitiveState, Queue, RenderPass,
    RenderPipeline, RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages,
    StencilState, TextureFormat, VertexState,
};

const CURVE_RESOLUTION: usize = 512;
const CURVE_SCALE: f32 = 15.0;
const BIND_GROUP_CAMERA: [(ShaderStages, BufferBindingType, bool); 2] = [
    (ShaderStages::VERTEX, BufferBindingType::Uniform, false),
    (
        ShaderStages::FRAGMENT,
        BufferBindingType::Storage { read_only: true },
        false,
    ),
];
const BIND_GROUP_NODE: [(ShaderStages, BufferBindingType, bool); 5] = [
    (ShaderStages::VERTEX, BufferBindingType::Uniform, true),
    (ShaderStages::VERTEX, BufferBindingType::Uniform, true),
    (
        ShaderStages::VERTEX,
        BufferBindingType::Storage { read_only: true },
        false,
    ),
    (
        ShaderStages::VERTEX,
        BufferBindingType::Storage { read_only: true },
        false,
    ),
    (ShaderStages::VERTEX, BufferBindingType::Uniform, false),
];

pub struct ShaderDragon {
    pub render_pipeline: RenderPipeline,
    pub bind_group_camera: BindGroup,
    pub bind_group_node: BindGroup,
    pub vp_buffer: Buffer,
    pub w_buffer: Buffer,
    pub r_buffer: Buffer,
    pub time_buffer: Buffer,
    pub light_buffer: Buffer,
}
impl ShaderDragon {
    pub fn new(renderer: &Renderer) -> Self {
        let device = &renderer.device;
        let new_shader_timestamp = Instant::now();
        let create_bind_group_layout = |entries: &[(ShaderStages, BufferBindingType, bool)]| {
            let entries =
                entries
                    .iter()
                    .enumerate()
                    .map(
                        |(i, (visibility, ty, has_dynamic_offset))| BindGroupLayoutEntry {
                            binding: i as u32,
                            visibility: *visibility,
                            ty: BindingType::Buffer {
                                ty: *ty,
                                has_dynamic_offset: *has_dynamic_offset,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    );
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: None,
                entries: entries.collect::<Vec<_>>().as_slice(),
            })
        };
        let bind_group_layout_camera = create_bind_group_layout(&BIND_GROUP_CAMERA);
        let bind_group_layout_node = create_bind_group_layout(&BIND_GROUP_NODE);
        let create_displacement = |points: Vec<Vec3>| {
            let n = points.len();
            let i0 = 1;
            let points = points
                .iter()
                .cycle()
                .skip(n - 1)
                .take(n + 3)
                .enumerate()
                .map(|(i, v)| ((i as f32 - i0 as f32) / n as f32, *v))
                .map(|(k, v)| Key::new(k, v, Interpolation::CatmullRom));
            let spline = Spline::from_iter(points);
            let mut translation = [Mat4::IDENTITY; CURVE_RESOLUTION];
            let mut rotation = [Mat4::IDENTITY; CURVE_RESOLUTION];
            let normalize = |i, n| (i % n) as f32 / n as f32;
            for i in 0..CURVE_RESOLUTION {
                let t1 = normalize(i, CURVE_RESOLUTION);
                let t2 = normalize(i + 1, CURVE_RESOLUTION);
                let p1 = spline.clamped_sample(t1).unwrap_or_default() * CURVE_SCALE;
                let p2 = spline.clamped_sample(t2).unwrap_or_default() * CURVE_SCALE;
                let tangent = p2 - p1;
                let t = normalize(i, CURVE_RESOLUTION - 1);
                let p = spline.clamped_sample(t).unwrap_or_default() * CURVE_SCALE;
                translation[i] = Mat4::from_translation(p);
                rotation[i] =
                    Mat4::from_quat(Quat::from_rotation_arc(Vec3::X, tangent.normalize()));
            }
            (translation, rotation)
        };
        // infinity symbol oo, span from -3 -> 3
        let _points_1: Vec<Vec3> = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 1.0, 0.0),
            Vec3::new(3.0, 0.0, 0.0),
            Vec3::new(2.0, -1.0, 0.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(-2.0, 1.0, 0.0),
            Vec3::new(-3.0, 0.0, 0.0),
            Vec3::new(-2.0, -1.0, 0.0),
        ];
        // infinity symbol oo, span from -3 -> 3
        let _points_2: Vec<Vec3> = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 1.0),
            Vec3::new(3.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, -1.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(-2.0, 0.0, 1.0),
            Vec3::new(-3.0, 0.0, 0.0),
            Vec3::new(-2.0, 0.0, -1.0),
        ];
        // infinity symbol oo, span from -3 -> 3
        let _points_3: Vec<Vec3> = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 1.0, 0.0),
            Vec3::new(3.0, 0.0, 0.0),
            Vec3::new(2.0, -1.0, 0.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(-2.0, 1.0, 0.0),
            Vec3::new(-3.0, 0.0, 0.0),
            Vec3::new(-2.0, -1.0, 0.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 1.0),
            Vec3::new(3.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, -1.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(-2.0, 0.0, 1.0),
            Vec3::new(-3.0, 0.0, 0.0),
            Vec3::new(-2.0, 0.0, -1.0),
        ];
        let (displacement, rotation_offset) = create_displacement(_points_3);
        // log::info!("{:?}", texels);
        let displacement_buffer = device.create_buffer_init(&BufferInitDescriptor {
            contents: bytemuck::cast_slice(&displacement),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            label: None,
        });
        let rotation_offset_buffer = device.create_buffer_init(&BufferInitDescriptor {
            contents: bytemuck::cast_slice(&rotation_offset),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            label: None,
        });
        let time_buffer = device.create_buffer(&BufferDescriptor {
            label: None,
            size: size_of::<f32>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout_node, &bind_group_layout_camera],
            push_constant_ranges: &[],
        });
        let vp_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(Mat4::IDENTITY.as_ref()),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        let light_uniform_size = size_of::<Light>() as BufferAddress;
        let light_buffer = device.create_buffer(&BufferDescriptor {
            label: None,
            size: MAX_LIGHT as BufferAddress * light_uniform_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
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
            ],
            label: None,
        });
        let node_uniform_size = size_of::<Mat4>() as BufferAddress;
        let node_uniform_aligned = {
            let alignment = device.limits().min_uniform_buffer_offset_alignment as BufferAddress;
            align_to(node_uniform_size, alignment)
        };
        let w_buffer = device.create_buffer(&BufferDescriptor {
            label: None,
            size: MAX_ENTITY as BufferAddress * node_uniform_aligned,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let r_buffer = device.create_buffer(&BufferDescriptor {
            label: None,
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
                    binding: 2, // displacement
                    resource: displacement_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3, // rotation offset
                    resource: rotation_offset_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 4, // time
                    resource: time_buffer.as_entire_binding(),
                },
            ],
            label: None,
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
                entry_point: None,
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: Some(FragmentState {
                module: &module,
                entry_point: None,
                compilation_options: PipelineCompilationOptions::default(),
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
            cache: None,
        });
        log::info!("created shader in {:?}", new_shader_timestamp.elapsed());
        Self {
            render_pipeline,
            bind_group_camera,
            bind_group_node,
            vp_buffer,
            w_buffer,
            r_buffer,
            time_buffer,
            light_buffer,
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
        queue.write_buffer(&self.time_buffer, 0, bytemuck::bytes_of(&(time)));
    }
    fn write_camera_data(&self, queue: &Queue, matrix: &[f32; 16]) {
        queue.write_buffer(&self.vp_buffer, 0, bytemuck::bytes_of(matrix));
    }
    fn write_light_data(&self, queue: &Queue, lights: &[Light]) {
        queue.write_buffer(&self.light_buffer, 0, bytemuck::cast_slice(lights));
    }
}
