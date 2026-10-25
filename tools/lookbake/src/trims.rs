//! Armour trim textures, which the game makes at run time (`PalettedTextureManager`): each trim
//! pattern's grey base texture (`textures/trims/entity/<layer type>/<pattern>.png`, its `.mcmeta`
//! naming the base palette) with the base palette's colours swapped for a material's
//! (`textures/palettes/trim/<material>.png`, index for index; `PaletteMapping`). Baked here for
//! every pattern, layer type and trim palette to where `acacia_render::entity::java::trims`
//! looks for them.

use std::collections::HashMap;
use std::path::Path;

use acacia_render::entity::java::trims::baked_path;
use image::RgbaImage;

use crate::download::Error;

/// Bakes into `out` (the look's root); the number of images.
pub fn bake(assets: &Path, out: &Path) -> Result<usize, Error> {
    let palettes = assets.join("textures/palettes");
    let targets: Vec<(String, Vec<u32>)> = std::fs::read_dir(palettes.join("trim"))?
        .flatten()
        .filter_map(|e| {
            let name = e.path().file_stem()?.to_str()?.to_owned();
            Some((format!("minecraft:trim/{name}"), palette(&e.path())?))
        })
        .collect();
    let mut bases: HashMap<String, Option<Vec<u32>>> = HashMap::new();
    let mut count = 0;
    for layer_type in std::fs::read_dir(assets.join("textures/trims/entity"))?.flatten() {
        let layer_name = layer_type.file_name().to_string_lossy().into_owned();
        for entry in std::fs::read_dir(layer_type.path())?.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "png") {
                continue;
            }
            let Some(base_palette) = base_palette(&path) else { continue };
            let base = bases.entry(base_palette.clone()).or_insert_with(|| palette(&palettes.join(format!("{base_palette}.png"))));
            let Some(base) = base.as_ref() else { continue };
            let image = image::open(&path)?.into_rgba8();
            let asset = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            for (id, target) in targets.iter().filter(|(_, t)| t.len() == base.len()) {
                let to = out.join(baked_path(&layer_name, asset, id));
                std::fs::create_dir_all(to.parent().expect("a file in a directory"))?;
                mapped(&image, base, target).save(&to)?;
                count += 1;
            }
        }
    }
    Ok(count)
}

/// The `palette.base_palette` of a texture's `.mcmeta`, as a path under `textures/palettes`.
fn base_palette(texture: &Path) -> Option<String> {
    let meta: serde_json::Value = serde_json::from_slice(&std::fs::read(format!("{}.mcmeta", texture.display())).ok()?).ok()?;
    let id = meta["palette"]["base_palette"].as_str()?;
    Some(id.strip_prefix("minecraft:").unwrap_or(id).to_owned())
}

/// A palette image's pixels, row by row, as ARGB.
fn palette(path: &Path) -> Option<Vec<u32>> {
    Some(image::open(path).ok()?.into_rgba8().pixels().map(|p| argb(p.0)).collect())
}

fn argb([r, g, b, a]: [u8; 4]) -> u32 {
    u32::from_be_bytes([a, r, g, b])
}

/// `PaletteMapping.create(base, target)` applied to every pixel, as `NativeImage.mappedCopy` does.
fn mapped(image: &RgbaImage, base: &[u32], target: &[u32]) -> RgbaImage {
    let opaque = |c: u32| c | 0xFF00_0000;
    let swaps: HashMap<u32, u32> = base.iter().zip(target).filter(|(k, _)| *k >> 24 != 0).map(|(k, t)| (opaque(*k), *t)).collect();
    let mut out = image.clone();
    for p in out.pixels_mut() {
        let color = argb(p.0);
        let alpha = color >> 24;
        if alpha == 0 {
            continue;
        }
        let to = swaps.get(&opaque(color)).copied().unwrap_or(opaque(color));
        let [_, r, g, b] = to.to_be_bytes();
        p.0 = [r, g, b, (alpha * (to >> 24) / 255) as u8];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swaps_base_colours_and_keeps_alpha_and_strangers() {
        let image = RgbaImage::from_raw(3, 1, vec![10, 10, 10, 128, 20, 20, 20, 255, 10, 10, 10, 0]).unwrap();
        let base = [argb([10, 10, 10, 255]), argb([0, 0, 0, 0])];
        let target = [argb([200, 100, 50, 255]), argb([1, 1, 1, 255])];
        let out = mapped(&image, &base, &target);
        assert_eq!(out.get_pixel(0, 0).0, [200, 100, 50, 128]);
        assert_eq!(out.get_pixel(1, 0).0, [20, 20, 20, 255]);
        assert_eq!(out.get_pixel(2, 0).0, [10, 10, 10, 0]);
    }
}
