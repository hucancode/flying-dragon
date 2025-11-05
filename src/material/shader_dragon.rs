use crate::geometry::Vertex;
use crate::material::Shader;
use crate::world::{Light, MAX_ENTITY, MAX_LIGHT, Renderer};
use core::f32;
use glam::{Mat4, Quat, Vec3};
use splines::{Interpolation, Key, Spline};
use std::borrow::Cow;
use std::mem::size_of;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;
use wgpu::util::align_to;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferAddress, BufferBinding,
    BufferBindingType, BufferSize, BufferUsages, CompareFunction, DepthBiasState,
    DepthStencilState, DynamicOffset, Face, FragmentState, FrontFace, MultisampleState,
    PipelineCompilationOptions, PipelineLayoutDescriptor, PrimitiveState, Queue, RenderPass,
    RenderPipeline, RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages,
    StencilState, TextureFormat, VertexState,
};

const CURVE_RESOLUTION: usize = 1024;
const CURVE_SCALE: f32 = 15.0;
const BIND_GROUP_CAMERA: [(ShaderStages, BufferBindingType, bool); 3] = [
    (ShaderStages::VERTEX, BufferBindingType::Uniform, false),
    (
        ShaderStages::FRAGMENT,
        BufferBindingType::Storage { read_only: true },
        false,
    ),
    (ShaderStages::FRAGMENT, BufferBindingType::Uniform, false), // light_count
];
const BIND_GROUP_NODE: [(ShaderStages, BufferBindingType, bool); 5] = [
    (ShaderStages::VERTEX, BufferBindingType::Uniform, true),
    (ShaderStages::VERTEX, BufferBindingType::Uniform, true),
    (
        ShaderStages::VERTEX,
        BufferBindingType::Storage { read_only: true },
        false,
    ),
    (ShaderStages::VERTEX, BufferBindingType::Uniform, false), // time
    (ShaderStages::VERTEX, BufferBindingType::Uniform, false), // combined_transform_map_length
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
    pub light_count_buffer: Buffer,
    pub combined_transform_buffer: Buffer,
    // transform_length_buffer: Buffer,
}
impl ShaderDragon {
    fn generate_path_data() -> [Mat4; CURVE_RESOLUTION] {
        let seed_points_in_range = |n, max_distance| {
            let mut last_last_point = Vec3::ZERO;
            let mut last_point = Vec3::ONE;
            (0..n)
                .map(|_| {
                    let random_point_in_front = |last_last_point: Vec3, last_point: Vec3| -> Vec3 {
                        use rand::Rng;
                        let mut rng = rand::rng();
                        let length: f32 = max_distance * 0.5;
                        const MAX_RETRY: usize = 20;
                        let mut best_direction = Vec3::ONE;
                        let mut best_score = f32::MAX;
                        for _ in 0..MAX_RETRY {
                            let direction = Vec3::new(
                                (rng.random_range(0.0..1.0) - 0.5) * 2.0 * length,
                                (rng.random_range(0.0..1.0) - 0.5) * 2.0 * length,
                                (rng.random_range(0.0..1.0) - 0.5) * 2.0 * length,
                            );
                            let distance = (direction + last_point).length();
                            let angle = direction.angle_between(last_point - last_last_point).abs();
                            let score = angle + distance / max_distance * std::f32::consts::PI;
                            if score < best_score {
                                best_score = score;
                                best_direction = direction;
                            }
                            if score < std::f32::consts::PI {
                                return direction;
                            }
                        }
                        best_direction
                    };
                    let delta = random_point_in_front(last_last_point, last_point);
                    last_last_point = last_point;
                    last_point += delta;
                    last_point
                })
                .collect()
        };
        let create_combined_transforms = |points: Vec<Vec3>| {
            let n = points.len();
            let i0 = 1;
            let mut d = 0.0;
            let mut distances = vec![0.0; n];
            for i in 1..n {
                let j = i - 1;
                let p1 = points[i];
                let p2 = points[j];
                d += p2.distance(p1);
                distances[i] = d;
            }
            d += points[n - 1].distance(points[0]);
            for distance in distances.iter_mut().skip(1) {
                *distance /= d;
            }
            let distances = distances
                .into_iter()
                .cycle()
                .skip(n - i0)
                .take(n + i0 * 2 + 1);
            let distances = distances.enumerate().map(|(i, v)| {
                if i < i0 {
                    v - 1.0
                } else if i > n {
                    v + 1.0
                } else {
                    v
                }
            });
            let points = points.into_iter().cycle().skip(n - i0).take(n + i0 * 2 + 1);
            let points = distances
                .zip(points)
                .map(|(k, v)| Key::new(k, v, Interpolation::CatmullRom));
            let spline = Spline::from_iter(points);
            let mut combined_transforms = [Mat4::IDENTITY; CURVE_RESOLUTION];
            let normalize = |i, n| (i % n) as f32 / n as f32;
            for i in 0..CURVE_RESOLUTION {
                let t1 = normalize(i, CURVE_RESOLUTION);
                let t2 = normalize(i + 1, CURVE_RESOLUTION);
                let p1 = spline.clamped_sample(t1).unwrap_or_default() * CURVE_SCALE;
                let p2 = spline.clamped_sample(t2).unwrap_or_default() * CURVE_SCALE;
                let tangent = p2 - p1;
                let translation = Mat4::from_translation(p1);
                let rotation = Mat4::from_quat(Quat::from_rotation_arc(Vec3::X, tangent.normalize()));
                combined_transforms[i] = translation * rotation;
            }
            return combined_transforms;
        };
        create_combined_transforms(seed_points_in_range(60, 4.5))
    }

    pub fn regenerate_path(&self, renderer: &Renderer) {
        let combined_transforms = Self::generate_path_data();
        renderer.queue.write_buffer(&self.combined_transform_buffer, 0, bytemuck::cast_slice(&combined_transforms));
    }

    pub fn new(renderer: &Renderer) -> Self {
        let device = &renderer.device;
        let new_shader_timestamp = Instant::now();
        let align = |n| {
            let alignment = device.limits().min_uniform_buffer_offset_alignment as BufferAddress;
            align_to(n, alignment)
        };
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
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout_node, &bind_group_layout_camera],
            push_constant_ranges: &[],
        });
        let combined_transforms = Self::generate_path_data();
        let combined_transform_buffer =
            renderer.create_buffer_init(bytemuck::cast_slice(&combined_transforms), BufferUsages::STORAGE);
        let time_buffer = renderer.create_buffer(size_of::<f32>() as u64, BufferUsages::UNIFORM);
        let vp_buffer = renderer.create_buffer_init(
            bytemuck::cast_slice(Mat4::IDENTITY.as_ref()),
            BufferUsages::UNIFORM,
        );
        let light_buffer = renderer.create_buffer(
            MAX_LIGHT * size_of::<Light>() as BufferAddress,
            BufferUsages::STORAGE,
        );
        let light_count_buffer = renderer.create_buffer(size_of::<u32>() as u64, BufferUsages::UNIFORM);
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
        let w_buffer =
            renderer.create_buffer(MAX_ENTITY * align(node_uniform_size), BufferUsages::UNIFORM);
        let r_buffer =
            renderer.create_buffer(MAX_ENTITY * align(node_uniform_size), BufferUsages::UNIFORM);
        let transform_length_buffer = renderer.create_buffer(size_of::<u32>() as u64, BufferUsages::UNIFORM);
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
                    binding: 2, // combined_transform_map
                    resource: combined_transform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3, // time
                    resource: time_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 4, // combined_transform_map_length
                    resource: transform_length_buffer.as_entire_binding(),
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
                targets: &[Some(renderer.config.view_formats[0].into())],
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
        renderer.queue.write_buffer(&transform_length_buffer, 0, bytemuck::bytes_of(&(CURVE_RESOLUTION as u32)));

        Self {
            render_pipeline,
            bind_group_camera,
            bind_group_node,
            vp_buffer,
            w_buffer,
            r_buffer,
            time_buffer,
            light_buffer,
            light_count_buffer,
            combined_transform_buffer,
            // transform_length_buffer,
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
        queue.write_buffer(&self.light_count_buffer, 0, bytemuck::bytes_of(&(lights.len() as u32)));
    }
}
