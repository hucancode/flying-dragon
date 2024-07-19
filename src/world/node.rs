use crate::geometry::Mesh;
use crate::material::Shader;
use glam::{f32::Quat, EulerRot, Mat4, Vec3};
use std::{cell::RefCell, rc::Rc};
use wgpu::Color;

pub enum Variant {
    Entity(Rc<Mesh>, Rc<dyn Shader>),
    Light(Color, f32),
    Group,
}

impl Default for Variant {
    fn default() -> Self {
        Self::Group
    }
}

pub type NodeRef = Rc<RefCell<Node>>;

pub struct Node {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub variant: Variant,
    pub children: Vec<NodeRef>,
    pub parent: Option<NodeRef>,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            variant: Variant::default(),
            children: Vec::new(),
            parent: None,
        }
    }
}

impl Node {
    pub fn new() -> NodeRef {
        Rc::new(RefCell::new(Node::default()))
    }

    pub fn new_light(color: Color, radius: f32) -> NodeRef {
        Rc::new(RefCell::new(Node {
            variant: Variant::Light(color, radius),
            ..Default::default()
        }))
    }

    pub fn new_entity(geometry: Rc<Mesh>, shader: Rc<dyn Shader>) -> NodeRef {
        Rc::new(RefCell::new(Node {
            variant: Variant::Entity(geometry, shader),
            ..Default::default()
        }))
    }
    pub fn translate(&mut self, x: f32, y: f32, z: f32) {
        self.translation.x = x;
        self.translation.y = y;
        self.translation.z = z;
    }
    pub fn scale(&mut self, x: f32, y: f32, z: f32) {
        self.scale.x = x;
        self.scale.y = y;
        self.scale.z = z;
    }
    pub fn rotate_quat(&mut self, q: Quat) {
        self.rotation = q;
    }
    pub fn rotate(&mut self, x: f32, y: f32, z: f32) {
        self.rotation = Quat::from_euler(EulerRot::XYZ, x, y, z);
    }
    pub fn calculate_transform(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
    pub fn add_child(&mut self, child: NodeRef) {
        self.children.push(child);
    }
}
