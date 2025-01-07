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
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;
use winit::application::ApplicationHandler;
use winit::event::ElementState;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

const LIGHT_RADIUS: f32 = 70.0;
const LIGHT_INTENSITY: f32 = 60.0;
const WINDOW_WIDTH: u32 = 1024;
const WINDOW_HEIGHT: u32 = 768;

pub struct App {
    window: Option<Arc<Window>>,
    start_time_stamp: Instant,
    renderer: Option<Renderer>,
    lights: Vec<(NodeRef, NodeRef, u128)>,
    event_loop: Option<EventLoopProxy<Renderer>>,
}

impl App {
    pub fn new(event_loop: &EventLoop<Renderer>) -> Self {
        Self {
            window: None,
            start_time_stamp: Instant::now(),
            renderer: None,
            lights: Vec::new(),
            event_loop: Some(event_loop.create_proxy()),
        }
    }
}

impl App {
    pub async fn make_renderer(window: Arc<Window>) -> Renderer {
        Renderer::new(window.clone(), WINDOW_WIDTH, WINDOW_HEIGHT).await
    }
    pub fn init(&mut self) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let app_init_timestamp = Instant::now();
        let cube_mesh = Rc::new(Mesh::new_cube(0xcba6f7ff, &renderer.device));
        let shader = Rc::new(ShaderDragon::new(&renderer));
        let dragon_mesh = Rc::new(Mesh::load_obj(
            include_bytes!("assets/dragon-low.obj"),
            &renderer.device,
        ));
        log::info!("loaded mesh in {:?}", app_init_timestamp.elapsed());
        let dragon = Node::new_entity(dragon_mesh.clone(), shader.clone());
        renderer.add(dragon);
        let lights = vec![
            (
                wgpu::Color {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.5,
                },
                LIGHT_RADIUS,
                LIGHT_INTENSITY,
                6000,
            ),
            (
                wgpu::Color {
                    r: 0.0,
                    g: 1.0,
                    b: 0.0,
                    a: 0.5,
                },
                LIGHT_RADIUS,
                LIGHT_INTENSITY,
                2200,
            ),
            (
                wgpu::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 1.0,
                    a: 0.5,
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
            // infinity symbol oo, span from -3 -> 3
            let points: Vec<Vec3> = vec![
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
            let n = points.len();
            let i0 = 1;
            let points = points
                .into_iter()
                .cycle()
                .skip(n - 1)
                .take(n + 3)
                .enumerate()
                .map(|(i, v)| ((i as f32 - i0 as f32) / n as f32, v))
                .map(|(k, v)| Key::new(k, v, Interpolation::CatmullRom));
            let spline = Spline::from_iter(points);
            const CURVE_SCALE: f32 = 20.0;
            let n = 100;
            let normalize = |i, n| (i % n) as f32 / n as f32;
            for i in 0..n {
                let t1 = normalize(i, n);
                let t2 = normalize(i + 1, n);
                let p1 = spline.clamped_sample(t1).unwrap_or_default() * CURVE_SCALE;
                let p2 = spline.clamped_sample(t2).unwrap_or_default() * CURVE_SCALE;
                let rotation = Quat::from_rotation_arc(Vec3::X, (p2 - p1).normalize());
                let r = (t1 * 256.0) as u32;
                let g = r;
                let b = r;
                let col = 0xff + (b << 8) + (g << 16) + (r << 24);
                let cube_mesh = Rc::new(Mesh::new_cube(col, &renderer.device));
                let cube = Node::new_entity(cube_mesh.clone(), shader_unlit.clone());
                cube.borrow_mut().translate(p1.x, p1.y, p1.z);
                cube.borrow_mut().rotate_quat(rotation);
                cube.borrow_mut().scale(0.2, 1.0, 1.0);
                renderer.add(cube.clone());
            }
        }
        log::info!("app initialized in {:?}", app_init_timestamp.elapsed());
    }
    pub fn update(&mut self, time: f32) {
        for (light, cube, time_offset) in self.lights.iter_mut() {
            let time = time + *time_offset as f32;
            let rx = PI * 2.0 * (0.00042 * time as f64).sin() as f32;
            let ry = PI * 2.0 * (0.00011 * time as f64).sin() as f32;
            let rz = PI * 2.0 * (0.00027 * time as f64).sin() as f32;
            cube.borrow_mut().rotate(rx, ry, rz);
            let x = (0.00058 * time as f64).sin() as f32;
            let y = (0.00076 * time as f64).sin() as f32;
            let z = (0.00042 * time as f64).sin() as f32;
            let v = Vec4::new(x, y, z, 1.0).normalize() * LIGHT_RADIUS;
            light.borrow_mut().translate(v.x, v.y, v.z);
        }
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        renderer.time = time;
    }
}

impl ApplicationHandler<Renderer> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        use winit::dpi::PhysicalSize;
        log::info!("creating window...");
        let mut attr = Window::default_attributes()
            .with_inner_size(PhysicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use web_sys::HtmlCanvasElement;
            use wgpu::web_sys;
            use winit::platform::web::WindowAttributesExtWebSys;
            // use first canvas element, or create one if none found
            let canvas = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.query_selector("canvas").ok())
                .and_then(|c| c)
                .and_then(|c| c.dyn_into::<HtmlCanvasElement>().ok());
            if let Some(canvas) = canvas {
                attr = attr.with_canvas(Some(canvas));
            } else {
                attr = attr.with_append(true);
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            attr = attr.with_title("Dragon");
        }
        let window = Arc::new(event_loop.create_window(attr).unwrap());
        let Some(event_loop) = self.event_loop.take() else {
            return;
        };
        self.window = Some(window.clone());
        log::info!(
            "window created! inner size {:?} outer size {:?}",
            window.inner_size(),
            window.outer_size(),
        );
        log::info!("creating renderer...");
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let renderer = App::make_renderer(window).await;
                log::info!("renderer created!");
                if let Err(_renderer) = event_loop.send_event(renderer) {
                    log::error!("Failed to send renderer back to application thread");
                }
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let renderer = pollster::block_on(App::make_renderer(window));
            if let Err(_renderer) = event_loop.send_event(renderer) {
                log::error!("Failed to send renderer back to application thread");
            }
        }
    }
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if cause == StartCause::Poll {
            let time = self.start_time_stamp.elapsed().as_millis() as f32;
            self.update(time);
            let Some(window) = self.window.as_ref() else {
                return;
            };
            window.request_redraw();
        }
    }
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, renderer: Renderer) {
        log::info!("got renderer!");
        self.renderer = Some(renderer);
        self.init();
    }
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if event == WindowEvent::CloseRequested {
            log::debug!("window close requested");
            event_loop.exit();
            return;
        }
        let Some(renderer) = self.renderer.as_mut() else {
            log::debug!("got event {event:?}, but no renderer to handle that");
            return;
        };
        match event {
            WindowEvent::RedrawRequested => renderer.draw(),
            WindowEvent::Resized(size) => renderer.resize(size.width, size.height),
            WindowEvent::KeyboardInput {
                device_id: _dev,
                event,
                is_synthetic: _synthetic,
            } => {
                log::info!("keyboard pressed {:?}", event);
                match (event.physical_key, event.state) {
                    // space to restart animation
                    (PhysicalKey::Code(KeyCode::Space), ElementState::Released) => {
                        self.start_time_stamp = Instant::now();
                    }
                    // escape to exit
                    (PhysicalKey::Code(KeyCode::Escape), ElementState::Released) => {
                        event_loop.exit();
                    }
                    // tab to pause/play animation
                    (PhysicalKey::Code(KeyCode::KeyP), ElementState::Released) => {
                        match event_loop.control_flow() {
                            ControlFlow::Poll => event_loop.set_control_flow(ControlFlow::Wait),
                            ControlFlow::Wait => event_loop.set_control_flow(ControlFlow::Poll),
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
