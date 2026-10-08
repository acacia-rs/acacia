//! Rain around the camera, after Java's `WeatherEffectRenderer`: each column within a radius gets
//! a quad turned to face the camera, from the ground (or a few blocks below the eye) to well above
//! it, with the pack's streak texture scrolling down and fading towards the edge of the radius.

use std::path::Path;

use acacia_world::World;
use glam::DVec3;
use image::RgbaImage;

use crate::assets::image_file;

/// Columns this far from the camera get rain (Java's fancy radius).
pub const RADIUS: i32 = 10;
/// Rain starts this far above the camera and stops this far below it at most.
const ABOVE: i32 = 10;
const BELOW: i32 = 10;

/// Rain streaks, rain in the left half and snow in the right, and how they lie on a column.
pub struct Streaks {
    pub image: RgbaImage,
    /// Width of a column's quad in blocks; it shows the whole rain half across it.
    pub column_width: f32,
    /// Blocks the texture's height covers before it repeats.
    pub blocks_per_repeat: f32,
}

/// Java's `rain.png` and `snow.png` side by side when the look has them (a 1-block quad, 64
/// texels a block down: Java's `WeatherEffectRenderer`), else Bedrock's `weather.png` (the
/// streaks in its top 20 rows at 16 texels a block). Bedrock's opaque pure-white texels, a regular
/// grid of single dots among the streaks, are not rain: cleared.
pub fn load_texture(files: &Path) -> Option<Streaks> {
    let open = |name: &str| Some(image::open(image_file(files, &format!("textures/environment/{name}"))?).ok()?.into_rgba8());
    if let (Some(rain), Some(snow)) = (open("rain"), open("snow")) {
        let mut image = RgbaImage::new(rain.width() * 2, rain.height());
        image::imageops::overlay(&mut image, &rain, 0, 0);
        image::imageops::overlay(&mut image, &image::imageops::resize(&snow, rain.width(), rain.height(), image::imageops::FilterType::Nearest), i64::from(rain.width()), 0);
        return Some(Streaks { image, column_width: 1.0, blocks_per_repeat: 4.0 });
    }
    let image = open("weather")?;
    let rows = (image.height() * 20 / 32).max(1);
    let mut streaks = image::imageops::crop_imm(&image, 0, 0, image.width(), rows).to_image();
    streaks.pixels_mut().filter(|p| p.0 == [255; 4]).for_each(|p| p.0[3] = 0);
    Some(Streaks { image: streaks, column_width: 0.5, blocks_per_repeat: 1.25 })
}

/// The cloud map, `textures/environment/clouds.png`: opaque texels are cloud.
pub fn load_clouds(files: &Path) -> Option<RgbaImage> {
    Some(image::open(image_file(files, "textures/environment/clouds")?).ok()?.into_rgba8())
}

/// One rainy column: where it is and the y range it rains over.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Column {
    pub x: i32,
    pub z: i32,
    pub bottom: i32,
    pub top: i32,
}

/// The columns around `camera` that rain reaches, each down to its first blocking block.
pub fn columns(world: &World, camera: DVec3) -> Vec<Column> {
    let eye = camera.floor().as_ivec3();
    let (top, lowest) = (eye.y + ABOVE, eye.y - BELOW);
    let mut out = Vec::new();
    for dx in -RADIUS..=RADIUS {
        for dz in -RADIUS..=RADIUS {
            if dx * dx + dz * dz > RADIUS * RADIUS {
                continue;
            }
            let (x, z) = (eye.x + dx, eye.z + dz);
            let Some(chunk) = world.get(x >> 4, z >> 4) else { continue };
            let chunk = chunk.read();
            // The first block from above that stops rain: anything but air.
            let ground = (lowest..=top).rev().find(|&y| world.registry().get(chunk.block(x, y, z)).is_some_and(|s| !s.is_air())).map_or(lowest, |y| y + 1);
            if ground < top {
                out.push(Column { x, z, bottom: ground.max(lowest), top });
            }
        }
    }
    out
}

/// How opaque rain is at `column` for a camera at `camera`: full near, half at the radius.
pub fn fade(column: &Column, camera: DVec3) -> f32 {
    let d = DVec3::new(f64::from(column.x) + 0.5 - camera.x, 0.0, f64::from(column.z) + 0.5 - camera.z).length() as f32;
    let r = RADIUS as f32;
    ((1.0 - (d * d) / (r * r)) * 0.5 + 0.5).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rain_fades_towards_the_edge() {
        let c = |x| Column { x, z: 0, bottom: 0, top: 10 };
        let camera = DVec3::new(0.5, 5.0, 0.5);
        assert!((fade(&c(0), camera) - 1.0).abs() < 1e-6);
        assert!((fade(&c(10), camera) - 0.5).abs() < 1e-6);
    }
}
