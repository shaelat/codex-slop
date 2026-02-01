use image::{ImageError, RgbImage};
use std::path::Path;

pub fn write_png(path: &Path, image: &RgbImage) -> Result<(), ImageError> {
    image.save(path)
}
