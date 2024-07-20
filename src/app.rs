use crate::geometry::Mesh;
use crate::material::ShaderDragon;
use crate::material::ShaderLit;
use crate::material::ShaderUnlit;
use crate::world::{Node, NodeRef, Renderer};
use glam::{Quat, Vec3, Vec4};
use splines::{Interpolation, Key, Spline};
use std::f32::consts::PI;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

const LIGHT_RADIUS: f32 = 50.0;
const LIGHT_INTENSITY: f32 = 2.0;

pub struct App {
    window: Option<Arc<Window>>,
    start_time_stamp: Instant,
    renderer: Option<Renderer>,
    lights: Vec<(NodeRef, NodeRef, u128)>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            window: None,
            start_time_stamp: Instant::now(),
            renderer: None,
            lights: Vec::new(),
        }
    }
}

impl App {
    pub async fn init(&mut self) {
        if self.window.is_none() {
            return;
        }
        let mut renderer = Renderer::new(self.window.as_ref().unwrap().clone()).await;
        let app_init_timestamp = Instant::now();
        let cube_mesh = Rc::new(Mesh::new_cube(0xcba6f7ff, &renderer.device));
        let shader = Rc::new(ShaderDragon::new(&renderer));
        let dragon_mesh = Rc::new(Mesh::load_obj(
            include_bytes!("assets/dragon.obj"),
            &renderer.device,
        ));
        println!("loaded mesh in {:?}", app_init_timestamp.elapsed());
        let dragon = Node::new_entity(dragon_mesh.clone(), shader.clone());
        renderer.add(dragon);
        let lights = vec![
            (
                wgpu::Color {
                    r: 0.0,
                    g: 0.5,
                    b: 1.0,
                    a: 1.0,
                },
                LIGHT_RADIUS,
                LIGHT_INTENSITY,
                6000,
            ),
            (
                wgpu::Color {
                    r: 0.0,
                    g: 0.5,
                    b: 1.0,
                    a: 1.0,
                },
                LIGHT_RADIUS,
                LIGHT_INTENSITY,
                2200,
            ),
            (
                wgpu::Color {
                    r: 0.0,
                    g: 1.0,
                    b: 0.5,
                    a: 1.0,
                },
                LIGHT_RADIUS,
                LIGHT_INTENSITY,
                4400,
            ),
        ];
        let shader_lit = Rc::new(ShaderLit::new(&renderer));
        let shader_unlit = Rc::new(ShaderUnlit::new(&renderer));
        self.lights = lights
            .into_iter()
            .map(|(color, radius, intensity, time_offset)| {
                let light = Node::new_light(color, radius * intensity);
                renderer.add(light.clone());
                let cube = Node::new_entity(cube_mesh.clone(), shader_lit.clone());
                cube.borrow_mut().translate(0.0, -2.0, 0.0);
                light.borrow_mut().add_child(cube.clone());
                (light, cube, time_offset)
            })
            .collect();
        const DEBUG_SPLINE: bool = false;
        if DEBUG_SPLINE {
            // infinity symbol oo
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
                .map(|v| v * 30.0)
                .enumerate()
                .map(|(i, v)| {
                    let k = (i as f32 - 1.0) / (n - 3) as f32;
                    Key::new(k, v, Interpolation::CatmullRom)
                })
                .collect();
            let spline = Spline::from_vec(points);
            let n = 80;
            for i in 0..n {
                let t1 = i as f32 / (n - 1) as f32;
                let t2 = ((i + 1) % n) as f32 / (n - 1) as f32;
                let p1 = spline.clamped_sample(t1).unwrap_or(Vec3::ZERO);
                let p2 = spline.clamped_sample(t2).unwrap_or(Vec3::ZERO);
                let rotation = Quat::from_rotation_arc(Vec3::X, (p2 - p1).normalize());
                let r = ((t1 - 0.5).abs() * 512.0) as u32;
                let g = 0x40;
                let b = 0x00;
                let col = 0xff + (b << 8) + (g << 16) + (r << 24);
                let cube_mesh = Rc::new(Mesh::new_cube(col, &renderer.device));
                let cube = Node::new_entity(cube_mesh.clone(), shader_unlit.clone());
                cube.borrow_mut().translate(p1.x, p1.y, p1.z);
                cube.borrow_mut().rotate_quat(rotation);
                cube.borrow_mut().scale(0.2, 1.0, 1.0);
                renderer.add(cube.clone());
            }
        }
        println!("app initialized in {:?}", app_init_timestamp.elapsed());
        self.renderer = Some(renderer);
    }
    pub fn update(&mut self, time: f32) {
        for (light, cube, time_offset) in self.lights.iter_mut() {
            let time = time + *time_offset as f32;
            let rx = PI * 2.0 * (0.00042 * time as f64).sin() as f32;
            let ry = PI * 2.0 * (0.00011 * time as f64).sin() as f32;
            let rz = PI * 2.0 * (0.00027 * time as f64).sin() as f32;
            cube.borrow_mut().rotate(rx, ry, rz);
            let x = 4.0 * (0.00058 * time as f64).sin() as f32;
            let y = 4.0 * (0.00076 * time as f64).sin() as f32;
            let z = 4.0 * (0.00142 * time as f64).sin() as f32;
            let v = Vec4::new(x, y, z, 1.0).normalize() * LIGHT_RADIUS;
            light.borrow_mut().translate(v.x, v.y, v.z);
        }
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.time = time;
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(Window::default_attributes().with_title("Dragon"))
            .unwrap();
        self.window = Some(Arc::new(window));
        pollster::block_on(self.init());
    }
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if cause == StartCause::Poll {
            let time = self.start_time_stamp.elapsed().as_millis() as f32;
            self.update(time);
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
    }
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if event == WindowEvent::CloseRequested {
            event_loop.exit();
        } else if let Some(renderer) = self.renderer.as_mut() {
            match event {
                WindowEvent::RedrawRequested => renderer.draw(),
                WindowEvent::Resized(size) => renderer.resize(size.width, size.height),
                _ => {}
            }
        }
    }
}
