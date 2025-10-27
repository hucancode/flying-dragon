use crate::world::{node, Camera, Light, Node, NodeRef};
use glam::{Mat4, Vec4, Vec4Swizzles};
use std::cmp::max;
use std::mem::size_of;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;
use wgpu::util::{BufferInitDescriptor, DeviceExt, align_to};
use wgpu::{
    BackendOptions, Backends, Buffer, BufferAddress, BufferDescriptor, BufferUsages, Color, CommandEncoderDescriptor, Device, DeviceDescriptor, Extent3d, IndexFormat, Instance, InstanceDescriptor, InstanceFlags, Limits, LoadOp, MemoryBudgetThresholds, Operations, Queue, RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor, RequestAdapterOptions, StoreOp, Surface, SurfaceConfiguration, SurfaceError, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor
};
use winit::window::Window;
use egui_wgpu::{Renderer as EguiRenderer, RendererOptions};
use egui_winit::State as EguiState;
use egui::{Context};

pub const MAX_ENTITY: u64 = 100000;
pub const MAX_LIGHT: u64 = 10;
const CLEAR_COLOR: Color = Color {
    r: 0.01233333333,
    g: 0.01233333333,
    b: 0.02388235294,
    a: 1.0,
};

pub struct Renderer {
    pub camera: Camera,
    pub root: NodeRef,
    pub time: f32,
    pub config: SurfaceConfiguration,
    pub surface: Surface<'static>,
    pub device: Device,
    pub queue: Queue,
    depth_texture_view: TextureView,
    pub egui_state: EguiState,
    pub egui_renderer: EguiRenderer,
    pub egui_context: Context,
    pub regenerate_path: bool,
    window: Arc<Window>,
}

impl Renderer {
    fn adapt_texture_format(format: TextureFormat) -> TextureFormat {
        #[cfg(not(target_arch = "wasm32"))]
        {
            format.add_srgb_suffix()
        }
        #[cfg(target_arch = "wasm32")]
        {
            format.remove_srgb_suffix()
        }
    }
    pub async fn new(window: Arc<Window>, width: u32, height: u32) -> Renderer {
        let new_renderer_timestamp = Instant::now();
        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::all(),
            flags: InstanceFlags::from_env_or_default(),
            backend_options: BackendOptions::from_env_or_default(),
            memory_budget_thresholds: MemoryBudgetThresholds::default(),
        });
        let surface = instance.create_surface(window.clone()).unwrap();
        log::info!(
            "created surface size {width}x{height} in {:?}",
            new_renderer_timestamp.elapsed()
        );
        let device_request_timestamp = Instant::now();
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("An appropriate adapter must exist!");
        let (device, queue) = adapter
            .request_device(&DeviceDescriptor::default())
            .await
            .or(adapter
                .request_device(&DeviceDescriptor {
                    required_limits: Limits::downlevel_webgl2_defaults(),
                    ..Default::default()
                })
                .await)
            .expect("A device must be present");
        log::info!(
            "requested device in {:?}",
            device_request_timestamp.elapsed()
        );
        let mut config = surface
            .get_default_config(&adapter, max(1, width), max(1, height))
            .expect("Surface must be supported by adapter");
        let format = surface.get_capabilities(&adapter).formats[0];
        config.format = Self::adapt_texture_format(format);
        config.view_formats.push(config.format);
        surface.configure(&device, &config);
        let depth_texture = device.create_texture(&TextureDescriptor {
            size: Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT,
            label: Some("Depth Texture"),
            view_formats: &[],
        });
        let depth_texture_view = depth_texture.create_view(&TextureViewDescriptor::default());
        log::info!(
            "in total, created new renderer in {:?}",
            new_renderer_timestamp.elapsed()
        );

        let egui_context = Context::default();
        let viewport_id = egui_context.viewport_id();
        let egui_state = EguiState::new(
            egui_context.clone(),
            viewport_id,
            &window,
            None,
            None,
            None,
        );
        let egui_renderer = EguiRenderer::new(&device, config.format, RendererOptions::default());

        Self {
            camera: Camera::new(),
            root: Node::new(),
            config,
            surface,
            device,
            queue,
            time: 0.0,
            depth_texture_view,
            egui_state,
            egui_renderer,
            egui_context,
            regenerate_path: false,
            window,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = max(1, width);
        self.config.height = max(1, height);
        self.surface.configure(&self.device, &self.config);
        let depth_texture = self.device.create_texture(&TextureDescriptor {
            size: Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT,
            label: Some("Depth Texture (Resized)"),
            view_formats: &[],
        });
        self.depth_texture_view = depth_texture.create_view(&TextureViewDescriptor::default());
    }

    pub fn handle_input(&mut self, event: &winit::event::WindowEvent) -> bool {
        self.egui_state.on_window_event(&self.window, event).consumed
    }

    pub fn draw(&mut self, mut run_ui: impl FnMut(&egui::Context, &mut bool)) {
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(SurfaceError::Timeout) => {
                log::error!("timed out getting surface texture, skip drawing this frame");
                return;
            }
            Err(e) => {
                self.surface.configure(&self.device, &self.config);
                log::error!(
                    "Something wrong when getting surface texture {e:?}, skip drawing this frame",
                );
                return;
            }
        };
        let view = frame.texture.create_view(&TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor::default());
        let mut nodes = Vec::new();
        let mut lights: Vec<(Color, f32, Mat4)> = Vec::new();
        nodes.clear();
        lights.clear();
        let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(CLEAR_COLOR),
                    store: StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &self.depth_texture_view,
                depth_ops: Some(Operations {
                    load: LoadOp::Clear(1.0),
                    store: StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        let mut q = Vec::new();
        q.push((self.root.clone(), Mat4::IDENTITY));
        let vp_matrix = self.camera.make_vp_matrix(
            self.config.width as f32 / self.config.height as f32,
        );
        while let Some((node, transform_mx)) = q.pop() {
            match &node.borrow().variant {
                node::Variant::Entity(geometry, shader) => {
                    let (_scale, rotation, _translation) =
                        transform_mx.to_scale_rotation_translation();
                    let rotation = Mat4::from_quat(rotation);
                    nodes.push((geometry.clone(), shader.clone(), transform_mx, rotation));
                }
                node::Variant::Light(color, radius) => {
                    lights.push((*color, *radius, transform_mx));
                }
                _ => {}
            }
            for child in node.borrow().children.iter() {
                let transform_mx = transform_mx * child.borrow().calculate_transform();
                q.push((child.clone(), transform_mx));
            }
        }
        let lights: Vec<Light> = lights
            .into_iter()
            .map(|(color, radius, transform)| {
                let mut position = transform * Vec4::W;
                position.w = radius;
                Light {
                    position: position.xyz(),
                    radius,
                    color: Vec4::new(
                        color.r as f32,
                        color.g as f32,
                        color.b as f32,
                        color.a as f32,
                    ),
                }
            })
            .collect();
        let node_uniform_aligned = {
            let node_uniform_size = size_of::<Mat4>() as BufferAddress;
            let alignment =
                self.device.limits().min_uniform_buffer_offset_alignment as BufferAddress;
            align_to(node_uniform_size, alignment)
        };
        for (i, (geometry, shader, transform, rotation)) in nodes.iter().enumerate() {
            let offset = (node_uniform_aligned * i as u64) as BufferAddress;
            shader.set_pipeline(&mut rpass, offset);
            shader.write_camera_data(&self.queue, vp_matrix.as_ref());
            shader.write_light_data(&self.queue, &lights);
            shader.write_time_data(&self.queue, self.time);
            shader.write_transform_data(&self.queue, offset, transform.as_ref());
            shader.write_rotation_data(&self.queue, offset, rotation.as_ref());
            rpass.set_index_buffer(geometry.index_buffer.slice(..), IndexFormat::Uint32);
            rpass.set_vertex_buffer(0, geometry.vertex_buffer.slice(..));
            let n = geometry.indices.len() as u32;
            rpass.draw_indexed(0..n, 0, 0..1);
        }
        drop(rpass);

        // Run egui
        let raw_input = self.egui_state.take_egui_input(&self.window);
        let full_output = self.egui_context.run(raw_input, |ctx| {
            run_ui(ctx, &mut self.regenerate_path);
        });

        self.egui_state.handle_platform_output(&self.window, full_output.platform_output);

        // Render egui
        let paint_jobs = self.egui_context.tessellate(full_output.shapes, full_output.pixels_per_point);
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(&self.device, &self.queue, *id, image_delta);
        }

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        self.egui_renderer.update_buffers(&self.device, &self.queue, &mut encoder, &paint_jobs, &screen_descriptor);

        {
            let rpass = encoder.begin_render_pass(&RenderPassDescriptor {
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            let mut rpass = rpass.forget_lifetime();
            self.egui_renderer.render(&mut rpass, &paint_jobs, &screen_descriptor);
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }
    pub fn add(&mut self, node: NodeRef) {
        self.root.borrow_mut().add_child(node);
    }
    pub fn create_buffer(&self, size: u64, usage: BufferUsages) -> Buffer {
        self.device.create_buffer(&BufferDescriptor {
            label: None,
            size,
            usage: usage | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
    pub fn create_buffer_init(&self, contents: &[u8], usage: BufferUsages) -> Buffer {
        self.device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents,
            usage: usage | BufferUsages::COPY_DST,
        })
    }
}
