use bytemuck::{Pod, Zeroable};
use glam::{Vec3, Vec4};
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct Light {
    pub position: Vec3,
    pub radius: f32,
    pub color: Vec4,
}
