//! A block item's inventory icon: the block's faces (cube, boxes or model) seen as Java's GUI
//! shows a block (from the south-east, 30° down), flattened to an isometric 32×32 image on the CPU
//! with a depth buffer; faces darker as the GUI light leaves them.

use glam::{Vec2, Vec3};
use image::RgbaImage;

use super::block::{Face, faces, texels, tile_of};
use crate::assets::flipbook::Atlas;
use crate::blocks::{RenderBlock, Shape};

const SIZE: u32 = 32;
const TILE: usize = 16;

/// Block space (0..1, x east, y up, z south) to icon pixels: the far top corner at (16, 2), the
/// near bottom one at (16, 31).
fn project(p: Vec3) -> Vec2 {
    Vec2::new(16.0, 2.0) + p.x * Vec2::new(14.0, 7.0) + p.z * Vec2::new(-14.0, 7.0) + (1.0 - p.y) * Vec2::new(0.0, 15.0)
}

/// Larger is nearer the viewer.
fn depth(p: Vec3) -> f32 {
    0.612 * (p.x + p.z) + 0.5 * p.y
}

/// The GUI light: tops full, south faces 0.8, east 0.6.
fn light(normal: Vec3) -> f32 {
    let n = normal.abs();
    if n.y >= n.x && n.y >= n.z {
        if normal.y > 0.0 { 1.0 } else { 0.5 }
    } else if n.z >= n.x {
        0.8
    } else {
        0.6
    }
}

/// Crossed plants show their texture flat; `None` for blocks drawn as nothing.
pub fn block_icon(block: &RenderBlock, atlas: &Atlas) -> Option<RgbaImage> {
    if block.shape == Shape::Cross {
        let rgba = texels(atlas.layers.get(block.textures[0] as usize), tile_of(block, 0));
        let flat = RgbaImage::from_raw(TILE as u32, TILE as u32, rgba)?;
        return Some(image::imageops::resize(&flat, SIZE, SIZE, image::imageops::FilterType::Nearest));
    }
    let mut icon = RgbaImage::new(SIZE, SIZE);
    let mut nearest = vec![f32::NEG_INFINITY; (SIZE * SIZE) as usize];
    for face in faces(block)? {
        draw(&mut icon, &mut nearest, &face, &texels(atlas.layers.get(face.tile.layer as usize), face.tile));
    }
    Some(icon)
}

/// A banner, which has no item texture (both games draw the entity): the flag's front from the
/// composed texture (Java's UV: 20×40 at 1, 1), dyed by the item's aux (legacy dye order,
/// 15 white) and without the stack's patterns, with the crossbar above it.
pub fn banner_icon(root: &std::path::Path, aux: u32) -> Option<RgbaImage> {
    let base = crate::banner::compose(root, &crate::banner::Banner::from_bedrock(aux as i32, [], 0))?;
    let scale = base.width() / 64;
    let flag = image::imageops::crop_imm(&base, scale, scale, 20 * scale, 40 * scale).to_image();
    let flag = image::imageops::resize(&flag, 14, 28, image::imageops::FilterType::Nearest);
    let mut icon = RgbaImage::new(SIZE, SIZE);
    image::imageops::replace(&mut icon, &flag, 9, 3);
    for x in 7..25 {
        for y in 1..3 {
            icon.put_pixel(x, y, image::Rgba([0x6B, 0x51, 0x32, 255]));
        }
    }
    Some(icon)
}

fn draw(icon: &mut RgbaImage, nearest: &mut [f32], face: &Face, rgba: &[u8]) {
    let [c0, c1, _, c3] = face.corners;
    let (o, a, b) = (project(c0), project(c1) - project(c0), project(c3) - project(c0));
    let det = a.perp_dot(b);
    if det.abs() < 1e-3 {
        return;
    }
    let shade = light((c1 - c0).cross(c3 - c0));
    let [t0, t1, _, t3] = face.uv.map(Vec2::from);
    for (x, y, px) in icon.enumerate_pixels_mut() {
        let d = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) - o;
        let (u, v) = (d.perp_dot(b) / a.perp_dot(b), a.perp_dot(d) / det);
        if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
            continue;
        }
        let at = depth(c0 + u * (c1 - c0) + v * (c3 - c0));
        let slot = &mut nearest[(y * SIZE + x) as usize];
        if at <= *slot {
            continue;
        }
        let uv = t0 + u * (t1 - t0) + v * (t3 - t0);
        let texel = |c: f32| ((c * TILE as f32) as usize).min(TILE - 1);
        let i = (texel(uv.y) * TILE + texel(uv.x)) * 4;
        if rgba[i + 3] > 0 {
            *slot = at;
            let lit = |c: u8| (f32::from(c) * shade) as u8;
            px.0 = [lit(rgba[i]), lit(rgba[i + 1]), lit(rgba[i + 2]), rgba[i + 3]];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cube_fills_the_hexagon() {
        let corners = [(Vec3::new(0.0, 1.0, 0.0), [16.0, 2.0]), (Vec3::new(1.0, 1.0, 1.0), [16.0, 16.0]), (Vec3::new(1.0, 0.0, 1.0), [16.0, 31.0])];
        for (p, at) in corners {
            assert_eq!(project(p), Vec2::from(at), "{p}");
        }
        assert!(depth(Vec3::ONE) > depth(Vec3::new(0.0, 1.0, 0.0)) && depth(Vec3::new(1.0, 0.0, 1.0)) > depth(Vec3::ZERO));
        assert_eq!((light(Vec3::Y), light(Vec3::Z), light(Vec3::X)), (1.0, 0.8, 0.6));
    }
}
