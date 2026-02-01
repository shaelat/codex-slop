use ptroute_render::bvh::Bvh;
use ptroute_render::geometry::Sphere;
use ptroute_render::math::{Ray, Vec3};

#[test]
fn bvh_hit_matches_bruteforce() {
    let mut spheres = Vec::new();
    let mut rng = TestRng::new(1);

    for _ in 0..64 {
        let center = Vec3::new(
            rng.range(-5.0, 5.0),
            rng.range(-5.0, 5.0),
            rng.range(-5.0, 5.0),
        );
        let radius = rng.range(0.2, 1.0);
        spheres.push(Sphere {
            center,
            radius,
            albedo: Vec3::new(0.5, 0.5, 0.5),
            emission: Vec3::zero(),
        });
    }

    let bvh = Bvh::new(spheres.clone());

    for _ in 0..128 {
        let origin = Vec3::new(
            rng.range(-8.0, 8.0),
            rng.range(-8.0, 8.0),
            rng.range(-8.0, 8.0),
        );
        let direction = Vec3::new(
            rng.range(-1.0, 1.0),
            rng.range(-1.0, 1.0),
            rng.range(-1.0, 1.0),
        )
        .normalized();
        let ray = Ray { origin, direction };

        let brute = brute_hit(&ray, &spheres);
        let bvh_hit = bvh.hit(&ray, 0.001, f32::INFINITY);

        assert_eq!(brute.is_some(), bvh_hit.is_some());
        if let (Some(a), Some(b)) = (brute, bvh_hit) {
            assert!((a.t - b.t).abs() < 1e-3);
        }
    }
}

fn brute_hit(ray: &Ray, spheres: &[Sphere]) -> Option<ptroute_render::geometry::Hit> {
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

struct TestRng {
    state: u64,
}

impl TestRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        (self.state >> 32) as u32
    }

    fn next_f32(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }

    fn range(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.next_f32()
    }
}
