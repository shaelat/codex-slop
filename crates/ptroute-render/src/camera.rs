use crate::math::{Ray, Vec3};

pub struct Camera {
    origin: Vec3,
    lower_left: Vec3,
    horizontal: Vec3,
    vertical: Vec3,
}

impl Camera {
    pub fn new(look_from: Vec3, look_at: Vec3, vup: Vec3, vfov_deg: f32, aspect: f32) -> Self {
        let theta = vfov_deg.to_radians();
        let h = (theta * 0.5).tan();
        let viewport_height = 2.0 * h;
        let viewport_width = aspect * viewport_height;

        let w = (look_from - look_at).normalized();
        let u = vup.cross(w).normalized();
        let v = w.cross(u);

        let origin = look_from;
        let horizontal = u * viewport_width;
        let vertical = v * viewport_height;
        let lower_left = origin - horizontal * 0.5 - vertical * 0.5 - w;

        Self {
            origin,
            lower_left,
            horizontal,
            vertical,
        }
    }

    pub fn ray(&self, u: f32, v: f32) -> Ray {
        Ray {
            origin: self.origin,
            direction: (self.lower_left + self.horizontal * u + self.vertical * v - self.origin)
                .normalized(),
        }
    }
}
