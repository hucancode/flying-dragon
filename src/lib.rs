mod app;
mod geometry;
mod material;
mod world;

pub use app::App;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
use winit::error::EventLoopError;
use winit::event_loop::{ControlFlow, EventLoop};
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn run_wasm() {
    if run().is_ok() {
        log::info!("App started successfully");
    } else {
        log::error!("Failed to start WASM app");
    }
}

fn init_logger() {
    let base_level = log::LevelFilter::Debug;
    let wgpu_level = log::LevelFilter::Error;
    #[cfg(target_arch = "wasm32")]
    {
        // On web, we use fern, as console_log doesn't have filtering on a per-module level.
        fern::Dispatch::new()
            .level(base_level)
            .level_for("wgpu_core", wgpu_level)
            .level_for("wgpu_hal", wgpu_level)
            .level_for("naga", wgpu_level)
            .chain(fern::Output::call(console_log::log))
            .apply()
            .unwrap();
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::builder()
            .filter_level(base_level)
            .filter_module("wgpu_core", wgpu_level)
            .filter_module("wgpu_hal", wgpu_level)
            .filter_module("naga", wgpu_level)
            .parse_default_env()
            .init();
    }
}

pub fn run() -> Result<(), EventLoopError> {
    init_logger();
    let event_loop = EventLoop::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(&event_loop);
    event_loop.run_app(&mut app)?;
    Ok(())
}
