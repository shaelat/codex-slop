use crate::camera::Camera;
use crate::geometry::{Hit, Sphere};
use crate::math::{Ray, Vec3};
use image::{Rgb, RgbImage};
use ptroute_model::SceneFile;
use std::collections::HashMap;
use std::time::Instant;

pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub spp: u32,
    pub bounces: u32,
    pub seed: u64,
    pub progress_every: u32,
}

pub fn render_scene(scene: &SceneFile, settings: &RenderSettings) -> RgbImage {
    let spheres = build_spheres(scene);
    let camera = build_camera(scene, settings);
    let mut image = RgbImage::new(settings.width, settings.height);

    let spp = settings.spp.max(1);
    let bounces = settings.bounces.max(1);
    let progress_every = settings.progress_every;
    let start = Instant::now();

    for y in 0..settings.height {
        for x in 0..settings.width {
            let mut color = Vec3::zero();
            for sample in 0..spp {
                let mut rng = Rng::new(hash_seed(settings.seed, x, y, sample));
                let u = (x as f32 + rng.next_f32()) / settings.width as f32;
                let v = (y as f32 + rng.next_f32()) / settings.height as f32;
                let ray = camera.ray(u, 1.0 - v);
                color = color + trace(&ray, &spheres, bounces, &mut rng);
            }
            let color = color / spp as f32;
            image.put_pixel(x, y, to_rgb(color));
        }

        if progress_every > 0 {
            let done = y + 1;
            if done == settings.height || done % progress_every == 0 {
                let elapsed = start.elapsed().as_secs_f64();
                let percent = (done as f64 / settings.height as f64) * 100.0;
                let total = if done > 0 {
                    elapsed * settings.height as f64 / done as f64
                } else {
                    0.0
                };
                let remaining = (total - elapsed).max(0.0);
                eprintln!(
                    "render: {}/{} ({:.1}%) elapsed {:.1}s eta {:.1}s",
                    done, settings.height, percent, elapsed, remaining
                );
            }
        }
    }

    image
}

fn trace(ray: &Ray, spheres: &[Sphere], bounces: u32, rng: &mut Rng) -> Vec3 {
    let mut current_ray = *ray;
    let mut throughput = Vec3::new(1.0, 1.0, 1.0);
    let mut color = Vec3::zero();

    for _ in 0..bounces {
        if let Some(hit) = closest_hit(&current_ray, spheres) {
            color = color + throughput.mul_elem(hit.emission);
            let direction = random_in_hemisphere(hit.normal, rng);
            current_ray = Ray {
                origin: hit.point + hit.normal * 0.001,
                direction,
            };
            throughput = throughput.mul_elem(hit.albedo);
        } else {
            color = color + throughput.mul_elem(background(&current_ray));
            return color;
        }
    }

    color
}

fn random_in_hemisphere(normal: Vec3, rng: &mut Rng) -> Vec3 {
    let mut dir = random_unit_vector(rng);
    if dir.dot(normal) < 0.0 {
        dir = dir * -1.0;
    }
    (normal + dir).normalized()
}

fn random_unit_vector(rng: &mut Rng) -> Vec3 {
    loop {
        let p = Vec3::new(
            rng.next_f32() * 2.0 - 1.0,
            rng.next_f32() * 2.0 - 1.0,
            rng.next_f32() * 2.0 - 1.0,
        );
        if p.dot(p) < 1.0 {
            return p.normalized();
        }
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
    let mut spheres = Vec::new();
    let mut positions: HashMap<String, Vec3> = HashMap::new();

    for node in &scene.nodes {
        let position = Vec3::new(node.position[0], node.position[1], node.position[2]);
        positions.insert(node.id.clone(), position);
        spheres.push(Sphere {
            center: position,
            radius: node_radius(node.seen),
            albedo: color_from_id(&node.id),
            emission: Vec3::zero(),
        });
    }

    for edge in &scene.edges {
        let Some(from) = positions.get(&edge.from) else { continue };
        let Some(to) = positions.get(&edge.to) else { continue };

        let delta = *to - *from;
        let distance = delta.length();
        if distance <= 0.0001 {
            continue;
        }

        let radius = link_radius(edge.seen);
        let spacing = (radius * 3.0).max(0.05);
        let steps = ((distance / spacing).ceil() as u32).max(2);

        let base_color = color_from_id(&format!("{}->{}", edge.from, edge.to));
        let intensity = link_intensity(edge.seen, edge.rtt_delta_ms_avg);
        let emission = base_color * intensity;
        let albedo = Vec3::new(0.08, 0.08, 0.08);

        for i in 1..steps {
            let t = i as f32 / steps as f32;
            let center = *from + delta * t;
            spheres.push(Sphere {
                center,
                radius,
                albedo,
                emission,
            });
        }
    }

    spheres
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

fn link_radius(seen: u32) -> f32 {
    let base = 0.04;
    let scale = (seen.max(1) as f32).ln() * 0.01;
    base + scale
}

fn link_intensity(seen: u32, rtt_delta: f64) -> f32 {
    let freq = (seen.max(1) as f32).ln() + 1.0;
    let rtt = 1.0 / (1.0 + (rtt_delta.abs() as f32 / 50.0));
    3.0 * freq * rtt
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

fn hash_seed(seed: u64, x: u32, y: u32, sample: u32) -> u64 {
    let mut v = seed ^ ((x as u64) << 32) ^ ((y as u64) << 16) ^ sample as u64;
    v = v.wrapping_add(0x9e3779b97f4a7c15);
    v = (v ^ (v >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    v = (v ^ (v >> 27)).wrapping_mul(0x94d049bb133111eb);
    v ^ (v >> 31)
}

struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        let state = if seed == 0 { 0xdeadbeefcafebabe } else { seed };
        Self { state }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        (self.state >> 32) as u32
    }

    fn next_f32(&mut self) -> f32 {
        let value = self.next_u32();
        value as f32 / u32::MAX as f32
    }
}
