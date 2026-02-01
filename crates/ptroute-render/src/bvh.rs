use crate::geometry::{Hit, Sphere};
use crate::math::{Ray, Vec3};

const LEAF_SIZE: usize = 4;

#[derive(Debug, Clone, Copy)]
struct Aabb {
    min: Vec3,
    max: Vec3,
}

impl Aabb {
    fn empty() -> Self {
        Self {
            min: Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY),
            max: Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
        }
    }

    fn from_sphere(sphere: &Sphere) -> Self {
        let r = Vec3::new(sphere.radius, sphere.radius, sphere.radius);
        Self {
            min: sphere.center - r,
            max: sphere.center + r,
        }
    }

    fn union(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    fn extent(&self) -> Vec3 {
        self.max - self.min
    }

    fn hit(&self, ray: &Ray, mut t_min: f32, mut t_max: f32) -> bool {
        if !hit_axis(self.min.x, self.max.x, ray.origin.x, ray.direction.x, &mut t_min, &mut t_max)
        {
            return false;
        }
        if !hit_axis(self.min.y, self.max.y, ray.origin.y, ray.direction.y, &mut t_min, &mut t_max)
        {
            return false;
        }
        if !hit_axis(self.min.z, self.max.z, ray.origin.z, ray.direction.z, &mut t_min, &mut t_max)
        {
            return false;
        }

        true
    }
}

fn hit_axis(min: f32, max: f32, origin: f32, direction: f32, t_min: &mut f32, t_max: &mut f32) -> bool {
    if direction == 0.0 {
        return origin >= min && origin <= max;
    }

    let inv_d = 1.0 / direction;
    let mut t0 = (min - origin) * inv_d;
    let mut t1 = (max - origin) * inv_d;
    if inv_d < 0.0 {
        std::mem::swap(&mut t0, &mut t1);
    }

    *t_min = t0.max(*t_min);
    *t_max = t1.min(*t_max);
    *t_max > *t_min
}

#[derive(Debug)]
struct BvhNode {
    bbox: Aabb,
    left: Option<Box<BvhNode>>,
    right: Option<Box<BvhNode>>,
    start: usize,
    end: usize,
}

impl BvhNode {
    fn build(indices: &mut [usize], spheres: &[Sphere], offset: usize) -> Self {
        let mut bbox = Aabb::empty();
        for &idx in indices.iter() {
            bbox = bbox.union(Aabb::from_sphere(&spheres[idx]));
        }

        if indices.len() <= LEAF_SIZE {
            return Self {
                bbox,
                left: None,
                right: None,
                start: offset,
                end: offset + indices.len(),
            };
        }

        let extent = bbox.extent();
        let axis = if extent.x >= extent.y && extent.x >= extent.z {
            0
        } else if extent.y >= extent.z {
            1
        } else {
            2
        };

        indices.sort_by(|&a, &b| {
            let ca = sphere_center_axis(&spheres[a], axis);
            let cb = sphere_center_axis(&spheres[b], axis);
            ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mid = indices.len() / 2;
        let (left_indices, right_indices) = indices.split_at_mut(mid);
        let left = Box::new(BvhNode::build(left_indices, spheres, offset));
        let right = Box::new(BvhNode::build(right_indices, spheres, offset + mid));

        let bbox = left.bbox.union(right.bbox);

        Self {
            bbox,
            left: Some(left),
            right: Some(right),
            start: 0,
            end: 0,
        }
    }

    fn hit(&self, ray: &Ray, t_min: f32, t_max: f32, spheres: &[Sphere], indices: &[usize]) -> Option<Hit> {
        if !self.bbox.hit(ray, t_min, t_max) {
            return None;
        }

        if self.left.is_none() && self.right.is_none() {
            let mut closest = None;
            let mut closest_t = t_max;
            for &idx in &indices[self.start..self.end] {
                if let Some(hit) = spheres[idx].hit(ray, t_min, closest_t) {
                    closest_t = hit.t;
                    closest = Some(hit);
                }
            }
            return closest;
        }

        let mut hit_left = None;
        let mut hit_right = None;
        let mut closest_t = t_max;

        if let Some(left) = &self.left {
            if let Some(hit) = left.hit(ray, t_min, closest_t, spheres, indices) {
                closest_t = hit.t;
                hit_left = Some(hit);
            }
        }

        if let Some(right) = &self.right {
            if let Some(hit) = right.hit(ray, t_min, closest_t, spheres, indices) {
                hit_right = Some(hit);
            }
        }

        hit_right.or(hit_left)
    }
}

fn sphere_center_axis(sphere: &Sphere, axis: u8) -> f32 {
    match axis {
        0 => sphere.center.x,
        1 => sphere.center.y,
        _ => sphere.center.z,
    }
}

pub struct Bvh {
    spheres: Vec<Sphere>,
    indices: Vec<usize>,
    root: BvhNode,
}

impl Bvh {
    pub fn new(spheres: Vec<Sphere>) -> Self {
        let mut indices: Vec<usize> = (0..spheres.len()).collect();
        let root = if indices.is_empty() {
            BvhNode {
                bbox: Aabb::empty(),
                left: None,
                right: None,
                start: 0,
                end: 0,
            }
        } else {
            BvhNode::build(&mut indices, &spheres, 0)
        };

        Self {
            spheres,
            indices,
            root,
        }
    }

    pub fn hit(&self, ray: &Ray, t_min: f32, t_max: f32) -> Option<Hit> {
        if self.indices.is_empty() {
            return None;
        }
        self.root.hit(ray, t_min, t_max, &self.spheres, &self.indices)
    }

    pub fn spheres(&self) -> &[Sphere] {
        &self.spheres
    }
}
