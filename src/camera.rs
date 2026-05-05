use cgmath::{point3, vec3, InnerSpace, Point3};

use crate::constants::*;

type Vec3 = cgmath::Vector3<f32>;
type Mat4 = cgmath::Matrix4<f32>;

#[derive(Debug, Clone)]
pub struct Camera {
    eye: Point3<f32>, // Point3: point in space, Vec3: displacement vector
    yaw: f32,         // rotation around Z (left/right), in radians
    pitch: f32,       // rotation around right axis (up/down), in radians
    up: Vec3,
}

impl Camera {
    pub fn new() -> Self {
        // Looking from (6,0,2) toward the origin
        let eye = point3(6.0, 0.0, 2.0);
        let target = point3(0.0, 0.0, 0.0);
        let direction = (target - eye).normalize();
        Self {
            eye,
            yaw: direction.y.atan2(direction.x),
            pitch: direction.z.asin(),
            up: Vec3::new(0.0, 0.0, 1.0),
        }
    }

    fn direction(&self) -> Vec3 {
        vec3(
            self.pitch.cos() * self.yaw.cos(),
            self.pitch.cos() * self.yaw.sin(),
            self.pitch.sin(),
        )
    }

    fn right(&self) -> Vec3 {
        self.direction().cross(self.up).normalize()
    }

    pub fn move_forward(&mut self, amount: f32) {
        self.eye += self.direction() * amount;
    }

    pub fn move_backward(&mut self, amount: f32) {
        self.eye -= self.direction() * amount;
    }

    pub fn move_right(&mut self, amount: f32) {
        self.eye += self.right() * amount;
    }

    pub fn move_left(&mut self, amount: f32) {
        self.eye -= self.right() * amount;
    }

    pub fn move_up(&mut self, amount: f32) {
        self.eye += self.up * amount;
    }

    pub fn move_down(&mut self, amount: f32) {
        self.eye -= self.up * amount;
    }

    pub fn update_camera_look(&mut self, delta: (f64, f64)) {
        const PITCH_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 0.001;

        self.yaw -= delta.0 as f32 * MOUSE_SENSITIVITY;
        self.pitch -= delta.1 as f32 * MOUSE_SENSITIVITY;
        self.pitch = self.pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_to_rh(self.eye, self.direction(), self.up)
    }
}
