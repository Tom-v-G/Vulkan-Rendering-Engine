use cgmath::{point3, InnerSpace, Point3};

type Vec3 = cgmath::Vector3<f32>;
type Mat4 = cgmath::Matrix4<f32>;

#[derive(Debug, Clone)]
pub struct Camera {
    eye: Point3<f32>, // Point3: point in space, Vec3: displacement vector
    direction: Vec3,  // normalized, where the camera is looking
    up: Vec3,
}

impl Camera {
    pub fn new() -> Self {
        // Looking from (6,0,2) toward the origin
        let eye = point3(6.0, 0.0, 2.0);
        let target = point3(0.0, 0.0, 0.0);
        Self {
            eye,
            direction: (target - eye).normalize(),
            up: Vec3::new(0.0, 0.0, 1.0),
        }
    }

    fn right(&self) -> Vec3 {
        self.direction.cross(self.up).normalize()
    }

    pub fn move_forward(&mut self, amount: f32) {
        self.eye += self.direction * amount;
    }

    pub fn move_backward(&mut self, amount: f32) {
        self.eye -= self.direction * amount;
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

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_to_rh(self.eye, self.direction, self.up)
    }
}
