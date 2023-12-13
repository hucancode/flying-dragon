use wgpu::{BufferAddress, Queue, RenderPass};

use crate::world::Light;

pub trait Shader {
    fn set_pipeline<'a>(&'a self, _pass: &mut RenderPass<'a>, _offset: BufferAddress) {}
    fn write_transform_data(&self, _queue: &Queue, _offset: BufferAddress, _matrix: &[f32; 16]) {}
    fn write_rotation_data(&self, _queue: &Queue, _offset: BufferAddress, _matrix: &[f32; 16]) {}
    fn write_time_data(&self, _queue: &Queue, _time: f32) {}
    fn write_camera_data(&self, _queue: &Queue, _matrix: &[f32; 16]) {}
    fn write_light_data(&self, _queue: &Queue, _lights: &[Light]) {}
}
