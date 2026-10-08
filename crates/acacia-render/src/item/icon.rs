//! A block item's inventory icon: its top, south and east faces as an isometric cube, drawn on the
//! CPU into a 32×32 image (the 3D GUI transform, flattened), sides darker as the GUI light leaves them.

use image::RgbaImage;

use super::block::{texels, tile_of};
use crate::assets::flipbook::Atlas;
use crate::blocks::{RenderBlock, Shape};

const SIZE: u32 = 32;
const TILE: usize = 16;
/// Face, parallelogram origin and its two edges in icon pixels, and the brightness it is drawn at.
const FACES: [(usize, [f32; 2], [f32; 2], [f32; 2], f32); 3] = [
    (2, [2.0, 9.0], [14.0, -7.0], [14.0, 7.0], 1.0),
    (4, [2.0, 9.0], [14.0, 7.0], [0.0, 15.0], 0.8),
    (0, [16.0, 16.0], [14.0, -7.0], [0.0, 15.0], 0.6),
];

/// Crossed plants show their texture flat; `None` for blocks drawn as nothing (or only as a model).
pub fn block_icon(block: &RenderBlock, atlas: &Atlas) -> Option<RgbaImage> {
    if block.shape == Shape::Cross {
        let rgba = texels(atlas.layers.get(block.textures[0] as usize), tile_of(block, 0));
        let flat = RgbaImage::from_raw(TILE as u32, TILE as u32, rgba)?;
        return Some(image::imageops::resize(&flat, SIZE, SIZE, image::imageops::FilterType::Nearest));
    }
    if !matches!(block.shape, Shape::Cube | Shape::Boxes(_) | Shape::Model(_)) {
        return None;
    }
    let mut icon = RgbaImage::new(SIZE, SIZE);
    for (face, origin, a, b, light) in FACES {
        let rgba = texels(atlas.layers.get(block.textures[face] as usize), tile_of(block, face));
        let det = a[0] * b[1] - a[1] * b[0];
        for (x, y, px) in icon.enumerate_pixels_mut() {
            let (dx, dy) = (x as f32 + 0.5 - origin[0], y as f32 + 0.5 - origin[1]);
            let u = (dx * b[1] - dy * b[0]) / det;
            let v = (a[0] * dy - a[1] * dx) / det;
            if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                continue;
            }
            let i = ((v * TILE as f32) as usize * TILE + (u * TILE as f32) as usize) * 4;
            let texel = &rgba[i..i + 4];
            if texel[3] > 0 {
                let shade = |c: u8| (f32::from(c) * light) as u8;
                px.0 = [shade(texel[0]), shade(texel[1]), shade(texel[2]), texel[3]];
            }
        }
    }
    Some(icon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_tile_the_hexagon() {
        // A pixel just inside each face, and the corners outside the cube.
        let inside = |face: usize, px: [f32; 2]| {
            let (_, o, a, b, _) = FACES.iter().copied().find(|f| f.0 == face).unwrap();
            let (dx, dy) = (px[0] - o[0], px[1] - o[1]);
            let det = a[0] * b[1] - a[1] * b[0];
            let (u, v) = ((dx * b[1] - dy * b[0]) / det, (a[0] * dy - a[1] * dx) / det);
            (0.0..1.0).contains(&u) && (0.0..1.0).contains(&v)
        };
        assert!(inside(2, [16.0, 9.0]), "top centre");
        assert!(inside(4, [8.0, 20.0]), "left side");
        assert!(inside(0, [24.0, 20.0]), "right side");
        assert!(![2, 4, 0].iter().any(|&f| inside(f, [1.0, 1.0])), "top-left corner is empty");
    }
}
