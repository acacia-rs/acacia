//! Rain and snow, after 26.3's `WeatherEffectRenderer` and `ClientLevel.tickWeatherEffects`: which
//! columns around the camera it rains or snows in ([`climate`], [`columns`]), their camera-facing
//! quads, and the splashes and sound each tick ([`splash`]). [`level`] reads them off a world.

pub mod climate;
pub mod columns;
pub mod level;
pub mod splash;

use std::path::Path;

use image::RgbaImage;

use crate::assets::image_file;

/// The rain and snow textures. A quad is one texture wide and four blocks a texture tall.
pub struct Streaks {
    pub rain: RgbaImage,
    pub snow: RgbaImage,
}

/// Java's `rain.png` and `snow.png` when the look has them, else the two halves of Bedrock's
/// `weather.png` (its streaks in the top 20 rows of 32). Bedrock's opaque pure-white texels, a
/// regular grid of single dots among the streaks, are not rain: cleared.
pub fn load_texture(files: &Path) -> Option<Streaks> {
    let open = |name: &str| Some(image::open(image_file(files, &format!("textures/environment/{name}"))?).ok()?.into_rgba8());
    if let (Some(rain), Some(snow)) = (open("rain"), open("snow")) {
        return Some(Streaks { rain, snow });
    }
    let image = open("weather")?;
    let (half, rows) = (image.width() / 2, (image.height() * 20 / 32).max(1));
    let cut = |x: u32| {
        let mut streaks = image::imageops::crop_imm(&image, x, 0, half, rows).to_image();
        streaks.pixels_mut().filter(|p| p.0 == [255; 4]).for_each(|p| p.0[3] = 0);
        streaks
    };
    Some(Streaks { rain: cut(0), snow: cut(half) })
}

/// The cloud map, `textures/environment/clouds.png`.
pub fn load_clouds(files: &Path) -> Option<RgbaImage> {
    Some(image::open(image_file(files, "textures/environment/clouds")?).ok()?.into_rgba8())
}
