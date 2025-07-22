use glam::{Mat4, Vec3};
use std::f32::consts::FRAC_PI_4;

pub struct Camera {
    pub azimuth: f32,
    pub elevation: f32,
    pub distance: f32,
    pub target: Vec3,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            azimuth: -0.5,
            elevation: 0.7,
            distance: 60.0,
            target: Vec3::ZERO,
            fov: FRAC_PI_4,
            near: 1.0,
            far: 1000.0,
        }
    }
}

impl Camera {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_eye_position(&self) -> Vec3 {
        let x = self.distance * self.elevation.cos() * self.azimuth.sin();
        let y = self.distance * self.elevation.cos() * self.azimuth.cos();
        let z = self.distance * self.elevation.sin();
        self.target + Vec3::new(x, y, z)
    }

    pub fn get_view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.get_eye_position(), self.target, Vec3::Z)
    }

    pub fn get_projection_matrix(&self, aspect_ratio: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov, aspect_ratio, self.near, self.far)
    }

    pub fn make_vp_matrix(&self, aspect_ratio: f32) -> Mat4 {
        self.get_projection_matrix(aspect_ratio) * self.get_view_matrix()
    }

    pub fn rotate(&mut self, delta_azimuth: f32, delta_elevation: f32) {
        self.azimuth += delta_azimuth;
        self.elevation = (self.elevation + delta_elevation).clamp(-1.5, 1.5);
    }

    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance * (1.0 + delta * 0.2)).clamp(15.0, 400.0);
    }

    pub fn make_vp_matrix_static(aspect_ratio: f32, distance: f32) -> Mat4 {
        let projection = Mat4::perspective_rh(FRAC_PI_4, aspect_ratio, 1.0, 1000.0);
        let view = Mat4::look_at_rh(Vec3::new(1.0, -2.0, 2.0) * distance, Vec3::ZERO, Vec3::Z);
        projection * view
    }
}