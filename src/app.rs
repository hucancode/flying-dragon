use crate::geometry::Mesh;
use crate::material::ShaderDragon;
use crate::material::ShaderLit;
use crate::material::ShaderUnlit;
use crate::world::{new_entity, new_light, Node, NodeRef, Renderer};
use glam::{Quat, Vec3, Vec4};
use splines::{Interpolation, Key, Spline};
use std::f32::consts::PI;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;
use winit::window::Window;

const LIGHT_RADIUS: f32 = 50.0;
const LIGHT_INTENSITY: f32 = 10.0;

pub struct App {
    renderer: Renderer,
    lights: Vec<(NodeRef, NodeRef, u128)>,
}

impl App {
    pub async fn new(window: Arc<Window>) -> Self {
        let renderer = Renderer::new(window).await;
        Self {
            renderer,
            lights: Vec::new(),
        }
    }
    pub fn init(&mut self) {
        let app_init_timestamp = Instant::now();
        let cube_mesh = Rc::new(Mesh::new_cube(0xcba6f7ff, &self.renderer.device));
        let shader = Rc::new(ShaderDragon::new(&self.renderer));
        let dragon_mesh = Rc::new(Mesh::load_obj(
            include_bytes!("assets/dragon.obj"),
            &self.renderer.device,
        ));
        let dragon = new_entity(dragon_mesh.clone(), shader.clone());
        self.renderer.root.add_child(dragon);
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
                0,
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
        let shader_lit = Rc::new(ShaderLit::new(&self.renderer));
        let shader_unlit = Rc::new(ShaderUnlit::new(&self.renderer));
        self.lights = lights
            .into_iter()
            .map(|(color, radius, intensity, time_offset)| {
                let mut light = new_light(color, radius * intensity);
                self.renderer.root.add_child(light.clone());
                let mut cube = new_entity(cube_mesh.clone(), shader_lit.clone());
                cube.translate(0.0, -2.0, 0.0);
                light.add_child(cube.clone());
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
                let cube_mesh = Rc::new(Mesh::new_cube(col, &self.renderer.device));
                let mut cube = new_entity(cube_mesh.clone(), shader_unlit.clone());
                cube.translate(p1.x, p1.y, p1.z);
                cube.rotate_quat(rotation);
                cube.scale_x(0.2);
                self.renderer.root.add_child(cube.clone());
            }
        }
        println!("app initialized in {:?}", app_init_timestamp.elapsed());
    }
    pub fn update(&mut self, _delta_time: f32, time: f32) {
        for (light, cube, time_offset) in self.lights.iter_mut() {
            let time = time + *time_offset as f32;
            let rx = PI * 2.0 * (0.00042 * time as f64).sin() as f32;
            let ry = PI * 2.0 * (0.00011 * time as f64).sin() as f32;
            let rz = PI * 2.0 * (0.00027 * time as f64).sin() as f32;
            cube.rotate(rx, ry, rz);
            let x = 4.0 * (0.00058 * time as f64).sin() as f32;
            let y = 4.0 * (0.00076 * time as f64).sin() as f32;
            let z = 4.0 * (0.00142 * time as f64).sin() as f32;
            let v = Vec4::new(x, y, z, 1.0).normalize() * LIGHT_RADIUS;
            light.translate(v.x, v.y, v.z);
        }
        self.renderer.time = time;
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
    }

    pub fn draw(&self) {
        self.renderer.draw();
    }
}
