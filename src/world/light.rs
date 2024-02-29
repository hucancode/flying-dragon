use bytemuck::{Pod, Zeroable};
use glam::Vec4;
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct Light {
    pub position_and_radius: Vec4,
    pub color: Vec4,
}
