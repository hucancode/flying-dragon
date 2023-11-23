use flying_dragon::run;
use winit::event_loop::EventLoop;
use winit::window::Window;
fn main() {
    let event_loop = EventLoop::new();
    let window = Window::new(&event_loop).unwrap();
    window.set_title("Dragon");
    env_logger::init();
    pollster::block_on(run(event_loop, window));
}
