//! A Java look's slot icons: the item's `gui` layers drawn as `GuiItemAtlas` draws them (the
//! display, `scale(s, -s, s)` about the slot's middle, orthographic, depth-tested, alpha cut at
//! 0.1) and lit by `Lighting`'s ITEMS_3D or ITEMS_FLAT pair. Rasterized on the CPU, once per item.

use glam::{EulerRot, Mat3, Vec2, Vec3};
use image::RgbaImage;

use super::java::{JavaItems, context};

/// A flat item is drawn at its sprite's resolution, which nearest sampling then scales exactly; a
/// solid one at a GUI scale of 4.
const SOLID_SIZE: u32 = 64;

/// The two lights in the display's space (the GUI's y flip undone), from `Lighting`'s poses.
fn lights(block_light: bool) -> [Vec3; 2] {
    let d0 = Vec3::new(0.2, 1.0, -0.7).normalize();
    let d1 = Vec3::new(-0.2, 1.0, 0.7).normalize();
    let yxz = |y: f32, x: f32| Mat3::from_euler(EulerRot::YXZ, y, x, 0.0);
    let flat = yxz(-std::f32::consts::PI / 8.0, std::f32::consts::PI * 0.75);
    let flip = Mat3::from_diagonal(Vec3::new(1.0, -1.0, 1.0));
    // ITEMS_3D's own y flip cancels the GUI's.
    let pose = if block_light { yxz(1.082_104_1, 3.237_585_8) * flat } else { flip * flat };
    [pose * d0, pose * d1]
}

/// `minecraft_mix_light`.
fn brightness(lights: &[Vec3; 2], normal: Vec3) -> f32 {
    ((lights[0].dot(normal).max(0.0) + lights[1].dot(normal).max(0.0)) * 0.6 + 0.4).min(1.0)
}

/// The Bedrock item's icon; `dye` as [`JavaItems::mesh`] takes it.
pub fn icon(items: &JavaItems, bedrock: &str, aux: u32, dye: Option<[u8; 3]>) -> Option<RgbaImage> {
    let name = items.java_name(bedrock, aux);
    let block_light = items.block_light(name, context::GUI)?;
    let mesh = items.mesh(name, context::GUI, dye)?;
    let frame = items.display(name, context::GUI)?.mesh_frame();
    let size = if mesh.solid { SOLID_SIZE } else { mesh.regions.iter().map(|r| r.width).max()?.max(16) };
    let lights = lights(block_light);
    let skin = &mesh.skin;
    let texel = |uv: Vec2| {
        let x = ((uv.x * skin.width as f32) as u32).min(skin.width - 1);
        let y = ((uv.y * skin.height as f32) as u32).min(skin.height - 1);
        let i = ((y * skin.width + x) * 4) as usize;
        [skin.rgba[i], skin.rgba[i + 1], skin.rgba[i + 2], skin.rgba[i + 3]]
    };
    let mut out = RgbaImage::new(size, size);
    let mut depth = vec![f32::MIN; (size * size) as usize];
    let half = size as f32 / 2.0;
    for triangle in skin.mesh.as_ref()?.vertices.chunks_exact(3) {
        let points = triangle.map(|v| {
            let p = frame.transform_point3(Vec3::from(v.position));
            (Vec2::new(half + p.x * size as f32, half - p.y * size as f32), p.z, Vec2::from(v.uv))
        });
        let normal = frame.transform_vector3(Vec3::from(triangle[0].normal)).normalize_or_zero();
        let shade = brightness(&lights, normal);
        fill(&points, size, |x, y, z, uv| {
            let at = (y * size + x) as usize;
            let [r, g, b, a] = texel(uv);
            if z < depth[at] || f32::from(a) < 0.1 * 255.0 {
                return;
            }
            depth[at] = z;
            let lit = |c: u8| (f32::from(c) * shade).round() as u8;
            let over = out.get_pixel_mut(x, y);
            let alpha = f32::from(a) / 255.0;
            for (dst, src) in over.0.iter_mut().zip([lit(r), lit(g), lit(b)]) {
                *dst = (f32::from(src) * alpha + f32::from(*dst) * (1.0 - alpha)).round() as u8;
            }
            over.0[3] = (f32::from(a) + f32::from(over.0[3]) * (1.0 - alpha)).round().min(255.0) as u8;
        });
    }
    Some(out)
}

/// Calls `pixel` for each pixel centre inside the triangle, with its depth and texture coordinates.
fn fill(points: &[(Vec2, f32, Vec2); 3], size: u32, mut pixel: impl FnMut(u32, u32, f32, Vec2)) {
    let [(a, za, ua), (b, zb, ub), (c, zc, uc)] = *points;
    let area = (b - a).perp_dot(c - a);
    if area.abs() < 1e-9 {
        return;
    }
    let lo = a.min(b).min(c).max(Vec2::ZERO);
    let hi = a.max(b).max(c).min(Vec2::splat(size as f32));
    for y in lo.y.floor() as u32..hi.y.ceil() as u32 {
        for x in lo.x.floor() as u32..hi.x.ceil() as u32 {
            let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            let (wa, wb) = ((b - p).perp_dot(c - p) / area, (c - p).perp_dot(a - p) / area);
            let wc = 1.0 - wa - wb;
            if wa < -1e-6 || wb < -1e-6 || wc < -1e-6 {
                continue;
            }
            pixel(x, y, wa * za + wb * zb + wc * zc, ua * wa + ub * wb + uc * wc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flat_item_faces_its_lights() {
        // A generated item's front, after the GUI's identity display, is lit fully.
        assert!(brightness(&lights(false), Vec3::Z) > 0.999, "{}", brightness(&lights(false), Vec3::Z));
    }

    #[test]
    fn a_block_shows_its_top_brightest() {
        // block/block's gui display: 30° x, 225° y.
        let turn = Mat3::from_euler(EulerRot::XYZ, 30f32.to_radians(), 225f32.to_radians(), 0.0);
        let lights = lights(true);
        let [top, left, right] = [Vec3::Y, Vec3::Z, Vec3::X].map(|n| brightness(&lights, turn * n));
        assert!(top > left && top > right && (left - right).abs() > 0.05, "{top} {left} {right}");
    }

    #[test]
    fn fill_covers_a_square_exactly() {
        let uv = Vec2::ZERO;
        let mut count = 0;
        for tri in [[Vec2::ZERO, Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0)], [Vec2::ZERO, Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)]] {
            fill(&tri.map(|p| (p, 0.0, uv)), 8, |_, _, _, _| count += 1);
        }
        // Pixel centres on the shared diagonal fall in both triangles.
        assert_eq!(count, 16 + 4);
    }
}
