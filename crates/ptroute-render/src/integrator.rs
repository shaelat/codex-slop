use crate::camera::Camera;
use crate::geometry::{Hit, Sphere};
use crate::math::{Ray, Vec3};
use image::{Rgb, RgbImage};
use ptroute_model::SceneFile;

pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
}

pub fn render_scene(scene: &SceneFile, settings: &RenderSettings) -> RgbImage {
    let spheres = build_spheres(scene);
    let camera = build_camera(scene, settings);
    let mut image = RgbImage::new(settings.width, settings.height);

    let light_dir = Vec3::new(-0.6, 1.0, -0.4).normalized();

    for y in 0..settings.height {
        let v = (y as f32 + 0.5) / settings.height as f32;
        for x in 0..settings.width {
            let u = (x as f32 + 0.5) / settings.width as f32;
            let ray = camera.ray(u, 1.0 - v);
            let color = shade(&ray, &spheres, light_dir);
            image.put_pixel(x, y, to_rgb(color));
        }
    }

    image
}

fn shade(ray: &Ray, spheres: &[Sphere], light_dir: Vec3) -> Vec3 {
    if let Some(hit) = closest_hit(ray, spheres) {
        let diffuse = hit.normal.dot(light_dir).max(0.0);
        let ambient = 0.2;
        let lighting = ambient + diffuse * 0.8;
        hit.albedo * lighting
    } else {
        background(ray)
    }
}

fn closest_hit(ray: &Ray, spheres: &[Sphere]) -> Option<Hit> {
    let mut closest = None;
    let mut closest_t = f32::INFINITY;
    for sphere in spheres {
        if let Some(hit) = sphere.hit(ray, 0.001, closest_t) {
            closest_t = hit.t;
            closest = Some(hit);
        }
    }
    closest
}

fn background(ray: &Ray) -> Vec3 {
    let t = 0.5 * (ray.direction.y + 1.0);
    let sky = Vec3::new(0.6, 0.8, 1.0);
    let ground = Vec3::new(0.05, 0.05, 0.07);
    ground * (1.0 - t) + sky * t
}

fn build_spheres(scene: &SceneFile) -> Vec<Sphere> {
    scene
        .nodes
        .iter()
        .map(|node| {
            let position = Vec3::new(node.position[0], node.position[1], node.position[2]);
            let radius = node_radius(node.seen);
            Sphere {
                center: position,
                radius,
                albedo: color_from_id(&node.id),
            }
        })
        .collect()
}

fn build_camera(scene: &SceneFile, settings: &RenderSettings) -> Camera {
    let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);

    for node in &scene.nodes {
        let pos = Vec3::new(node.position[0], node.position[1], node.position[2]);
        min = min.min(pos);
        max = max.max(pos);
    }

    let center = (min + max) * 0.5;
    let extent = (max - min).length().max(1.0);
    let distance = extent * 1.6;

    let look_from = center + Vec3::new(distance, distance * 0.6, distance);
    let look_at = center;
    let vup = Vec3::new(0.0, 1.0, 0.0);
    let aspect = settings.width as f32 / settings.height as f32;

    Camera::new(look_from, look_at, vup, 35.0, aspect)
}

fn node_radius(seen: u32) -> f32 {
    let base = 0.15;
    let scale = (seen.max(1) as f32).ln() * 0.05;
    base + scale
}

fn color_from_id(id: &str) -> Vec3 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let r = ((hash >> 0) & 0xFF) as f32 / 255.0;
    let g = ((hash >> 8) & 0xFF) as f32 / 255.0;
    let b = ((hash >> 16) & 0xFF) as f32 / 255.0;
    Vec3::new(0.2 + 0.8 * r, 0.2 + 0.8 * g, 0.2 + 0.8 * b)
}

fn to_rgb(color: Vec3) -> Rgb<u8> {
    let c = color.clamp01();
    let gamma = Vec3::new(c.x.sqrt(), c.y.sqrt(), c.z.sqrt());
    Rgb([
        (gamma.x * 255.0) as u8,
        (gamma.y * 255.0) as u8,
        (gamma.z * 255.0) as u8,
    ])
}
