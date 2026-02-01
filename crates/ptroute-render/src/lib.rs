//! CPU path tracer and image output.

pub mod bvh;
pub mod camera;
pub mod geometry;
pub mod image_out;
pub mod integrator;
pub mod math;

pub use integrator::{render_scene, render_scene_progressive, RenderSettings};
pub use image_out::write_png;
