use image::{ImageError, RgbImage};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn write_png(path: &Path, image: &RgbImage) -> Result<(), ImageError> {
    let tmp_path = temp_path(path);
    image.save(&tmp_path)?;

    if let Ok(file) = fs::File::open(&tmp_path) {
        let _ = file.sync_all();
    }

    if let Err(err) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(ImageError::IoError(err));
    }

    if let Some(parent) = path.parent() {
        if let Ok(dir) = fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }

    Ok(())
}

fn temp_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("image");
    let ext = path
        .extension()
        .and_then(|name| name.to_str())
        .unwrap_or("png");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let filename = format!(".{}.part-{}-{}.{}", stem, pid, stamp, ext);
    parent.join(filename)
}
