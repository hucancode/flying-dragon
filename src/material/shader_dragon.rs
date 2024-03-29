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
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferAddress, BufferBinding,
    BufferBindingType, BufferDescriptor, BufferSize, BufferUsages, CompareFunction, DepthBiasState,
    DepthStencilState, DynamicOffset, Face, FragmentState, FrontFace, MultisampleState,
    PipelineLayoutDescriptor, PrimitiveState, Queue, RenderPass, RenderPipeline,
    RenderPipelineDescriptor, ShaderModule, ShaderModuleDescriptor, ShaderSource, ShaderStages,
    StencilState, TextureFormat, VertexState,
};

const CURVE_RESOLUTION: i32 = 512;
const CURVE_SCALE: f32 = 15.0;
const CURVE_SMOOTH: i32 = 3;

pub struct ShaderDragon {
    pub module: ShaderModule,
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
                            ty: BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: BufferSize::new(0),
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
                    binding: 2, // displacement map
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3, // rotation offset map
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 4, // time
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
        let create_displacement = |points: Vec<Vec3>| {
            let n = points.len();
            let left_pad = points.iter().skip(n - 1);
            let right_pad = points.iter().take(2);
            let i0 = 1;
            let points = left_pad
                .chain(points.iter())
                .chain(right_pad)
                .enumerate()
                .map(|(i, v)| {
                    let k = (i as f32 - i0 as f32) / n as f32;
                    Key::new(k, *v, Interpolation::CatmullRom)
                })
                .collect();
            let spline = Spline::from_vec(points);
            let mut displacement = Vec::new();
            let mut rotation_offset = Vec::new();
            let mut last_tangent = Vec3::X;
            for j in 0..CURVE_RESOLUTION {
                let mut tangent = Vec3::ZERO;
                let mut weight_sum = 0.0;
                for delta in -CURVE_SMOOTH..=CURVE_SMOOTH {
                    let i = (j + CURVE_RESOLUTION + delta) % CURVE_RESOLUTION;
                    let t1 = i as f32 / CURVE_RESOLUTION as f32;
                    let t2 = ((i + 1) % CURVE_RESOLUTION) as f32 / CURVE_RESOLUTION as f32;
                    let p1 = spline.clamped_sample(t1).unwrap_or_default() * CURVE_SCALE;
                    let p2 = spline.clamped_sample(t2).unwrap_or_default() * CURVE_SCALE;
                    let weight = (CURVE_SMOOTH + 1 - delta) as f32;
                    weight_sum += weight;
                    tangent += (p2 - p1) * weight;
                }
                tangent /= weight_sum;
                let alpha = tangent.clone().angle_between(last_tangent.clone());
                if j > 0 && alpha > 0.5 || alpha.is_nan() {
                    let t1 = j as f32 / (CURVE_RESOLUTION - 1) as f32;
                    let t2 = ((j + 1) % CURVE_RESOLUTION) as f32 / (n - 1) as f32;
                    let p1 = spline.clamped_sample(t1).unwrap_or_default();
                    let p2 = spline.clamped_sample(t2).unwrap_or_default();
                    println!("at {j}, t1 = {t1}, t2 = {t2}, abnormal alpha = {alpha}");
                    println!("p1 = {p1:?}, p2 = {p2:?}");
                }
                let t = j as f32 / (CURVE_RESOLUTION - 1) as f32;
                let p = spline.clamped_sample(t).unwrap_or_default() * CURVE_SCALE;
                let translation = Mat4::from_translation(p);
                let rotation = Mat4::from_quat(Quat::from_rotation_arc(
                    Vec3::X,
                    tangent.try_normalize().unwrap_or(last_tangent),
                ));
                last_tangent = tangent;
                displacement.push(translation);
                rotation_offset.push(rotation);
            }
            (displacement, rotation_offset)
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
        // println!("{:?}", texels);
        let displacement_buffer = device.create_buffer_init(&BufferInitDescriptor {
            contents: bytemuck::cast_slice(displacement.as_slice()),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            label: None,
        });
        let rotation_offset_buffer = device.create_buffer_init(&BufferInitDescriptor {
            contents: bytemuck::cast_slice(rotation_offset.as_slice()),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            label: None,
        });
        let time_buffer = device.create_buffer(&BufferDescriptor {
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
        let vp_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Camera View Projection Buffer"),
            contents: bytemuck::cast_slice(Mat4::IDENTITY.as_ref()),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        let light_uniform_size = size_of::<Light>() as BufferAddress;
        let light_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Light Buffer"),
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
                entry_point: "vs_main",
                // entry_point: "vs_main_circle",
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
        println!("created shader in {:?}", new_shader_timestamp.elapsed());
        Self {
            module,
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
