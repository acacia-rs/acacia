//! A Java item's layers as one skin: each (image, tint) its own region in a strip, quads
//! re-pointed at their region.

use std::collections::HashMap;

use image::RgbaImage;

use super::java::{ItemMesh, JavaItems, Layer, Region, vertex};
use crate::entity::Skin;
use crate::entity::bake::Mesh;

/// Java leaves a quad untinted when its index is past the layer's tints.
fn multiplier(layer: &Layer, tint: i32, color: i32, dye: Option<[u8; 3]>) -> u32 {
    let tint = match usize::try_from(tint) {
        Ok(0) if dye.is_some() => dye.map(|[r, g, b]| 0xFF00_0000 | u32::from_be_bytes([0, r, g, b])),
        Ok(index) => layer.tints.get(index).map(|&t| t as u32),
        Err(_) => None,
    };
    multiply(tint.unwrap_or(u32::MAX), color as u32)
}

fn multiply(a: u32, b: u32) -> u32 {
    let channel = |shift: u32| ((a >> shift & 0xFF) * (b >> shift & 0xFF) / 255) << shift;
    channel(24) | channel(16) | channel(8) | channel(0)
}

/// The image's first frame (an animated strip stacks them downwards), tinted by `argb`.
fn tinted(image: &RgbaImage, argb: u32) -> RgbaImage {
    let (width, height) = (image.width(), image.height().min(image.width()));
    let mut out = image::imageops::crop_imm(image, 0, 0, width, height).to_image();
    let [a, r, g, b] = argb.to_be_bytes();
    if argb != u32::MAX {
        for pixel in out.pixels_mut() {
            for (channel, by) in pixel.0.iter_mut().zip([r, g, b, a]) {
                *channel = (u16::from(*channel) * u16::from(by) / 255) as u8;
            }
        }
    }
    out
}

pub(super) fn build(items: &JavaItems, layers: &[Layer], dye: Option<[u8; 3]>) -> Option<ItemMesh> {
    let mut images: HashMap<String, Option<RgbaImage>> = HashMap::new();
    let mut regions: Vec<(Region, RgbaImage)> = Vec::new();
    let mut quads = Vec::new();
    for layer in layers {
        for quad in items.quads(layer) {
            let argb = multiplier(layer, quad.tint, quad.color, dye);
            let index = match regions.iter().position(|(r, _)| r.image == quad.image && r.argb == argb) {
                Some(index) => index,
                None => {
                    let image = images.entry(quad.image.clone()).or_insert_with(|| image::open(items.image_path(&quad.image)).ok().map(|i| i.into_rgba8()));
                    let Some(image) = image.as_ref() else { continue };
                    let pixels = tinted(image, argb);
                    let y = regions.last().map_or(0, |(r, _)| r.y + r.height);
                    regions.push((Region { image: quad.image.clone(), argb, y, width: pixels.width(), height: pixels.height() }, pixels));
                    regions.len() - 1
                }
            };
            quads.push((quad, index));
        }
    }
    if quads.is_empty() {
        return None;
    }
    let width = regions.iter().map(|(r, _)| r.width).max()?;
    let height = regions.iter().map(|(r, _)| r.y + r.height).max()?;
    let mut strip = RgbaImage::new(width, height);
    for (region, pixels) in &regions {
        image::imageops::replace(&mut strip, pixels, 0, i64::from(region.y));
    }
    let mut vertices = Vec::with_capacity(quads.len() * 6);
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for &(quad, index) in &quads {
        let region = &regions[index].0;
        let uv = |corner: usize| {
            let [u, v] = [quad.uv[corner * 2], quad.uv[corner * 2 + 1]];
            [u * region.width as f32 / width as f32, (region.y as f32 + v * region.height as f32) / height as f32]
        };
        vertices.extend([0, 1, 2, 0, 2, 3].map(|corner| vertex(quad, corner, uv(corner))));
        for corner in 0..4 {
            lo = lo.min(quad.pos[corner * 3 + 2]);
            hi = hi.max(quad.pos[corner * 3 + 2]);
        }
    }
    let skin = Skin { width, height, rgba: strip.into_raw(), mesh: Some(Mesh::one_bone(vertices)) };
    Some(ItemMesh {
        skin,
        quad_regions: quads.iter().map(|&(_, index)| index).collect(),
        regions: regions.into_iter().map(|(r, _)| r).collect(),
        foil: layers.iter().any(|l| l.foil != "NONE"),
        solid: hi - lo > 1.0 / 16.0 + 1e-4,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tints_multiply_channelwise() {
        assert_eq!(multiply(u32::MAX, 0xFFA0_6540), 0xFFA0_6540);
        assert_eq!(multiply(0x8080_8080, 0xFF00_FF00), 0x8000_8000);
    }
}
