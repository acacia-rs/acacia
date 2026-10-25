//! The round shadows under entities, as Java's `EntityRenderer.extractShadow` picks the pieces and
//! `ShadowFeatureRenderer` lays them: every cell around the entity, from its feet down as far as
//! the shadow reaches, whose block below is drawn with a full collision shape and whose raw
//! brightness is over 3 gets a quad on that block's top, mapped so the blob stays centred under
//! the entity. It thins with the depth below the feet, the distance from the camera (gone at 16
//! blocks) and the cell's light. Radii per kind: [`radius`]. Checked by the parity harness `shadows`.

pub mod radius;

use acacia_world::World;
use glam::{DVec3, Vec3};

use crate::blocks::BlockTable;
pub use radius::radius;

/// Lifts the quads off the ground they lie on (Java offsets the shadow's depth instead).
const LIFT: f32 = 0.01;

/// One entity's shadow: its feet, the blob's radius and `getShadowStrength`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    pub feet: DVec3,
    pub radius: f32,
    pub strength: f32,
}

/// A vertex relative to the camera; `uv` runs 0 to 1 across the blob.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShadowVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub alpha: f32,
}

/// What a shadow reads of the world.
pub trait Ground {
    /// The block at the cell is drawn (its render shape is not `INVISIBLE`) and its collision
    /// shape is a full block.
    fn floor(&self, x: i32, y: i32, z: i32) -> bool;
    /// `getMaxLocalRawBrightness` of the cell: its block light or its sky light less the sky's
    /// darkening, the greater.
    fn brightness(&self, x: i32, y: i32, z: i32) -> u8;
}

/// `EntityRenderState.ShadowPiece`: the cell over the block it lies on, the cell's corner from the
/// feet, and its alpha.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Piece {
    pub cell: [i32; 3],
    pub relative: [f32; 3],
    pub alpha: f32,
}

/// The dimension type's `ambient_light`.
pub fn ambient_light(realm: crate::sky::Realm) -> f32 {
    match realm {
        crate::sky::Realm::Overworld => 0.0,
        crate::sky::Realm::Nether => 0.1,
        crate::sky::Realm::End => 0.25,
    }
}

/// `Lightmap.getBrightness` of a dimension with `ambient` light (0 in the Overworld, 0.1 in the
/// Nether, 0.25 in the End).
pub fn light_brightness(ambient: f32, level: u8) -> f32 {
    let v = f32::from(level) / 15.0;
    let curved = v / (4.0 - 3.0 * v);
    curved + ambient * (1.0 - curved)
}

/// `extractShadow`: the pieces of shadow `s`, `distance_sq` from the camera.
pub fn pieces(s: &Shadow, distance_sq: f64, ground: &impl Ground, ambient: f32) -> Vec<Piece> {
    let mut out = Vec::new();
    let radius = s.radius.min(32.0);
    let pow = ((1.0 - distance_sq / 256.0) * f64::from(s.strength)) as f32;
    if radius <= 0.0 || pow <= 0.0 {
        return out;
    }
    let floor = |v: f64| v.floor() as i32;
    let r = f64::from(radius);
    let depth = (pow / 0.5 - 1.0).min(radius);
    let (y0, y1) = (floor(s.feet.y - f64::from(depth)), floor(s.feet.y));
    for z in floor(s.feet.z - r)..=floor(s.feet.z + r) {
        for x in floor(s.feet.x - r)..=floor(s.feet.x + r) {
            for y in y0..=y1 {
                if !ground.floor(x, y - 1, z) {
                    continue;
                }
                let brightness = ground.brightness(x, y, z);
                if brightness <= 3 {
                    continue;
                }
                let power = pow - (s.feet.y - f64::from(y)) as f32 * 0.5;
                let alpha = (power * 0.5 * light_brightness(ambient, brightness)).clamp(0.0, 1.0);
                let relative = [(f64::from(x) - s.feet.x) as f32, (f64::from(y) - s.feet.y) as f32, (f64::from(z) - s.feet.z) as f32];
                out.push(Piece { cell: [x, y, z], relative, alpha });
            }
        }
    }
    out
}

/// `ShadowFeatureRenderer.prepare`'s four corners of a piece: position from the feet and (u, v).
pub fn corners(piece: &Piece, radius: f32) -> [[f32; 5]; 4] {
    let [x, y, z] = piece.relative;
    let radius = radius.min(32.0);
    let (x1, z1) = (x + 1.0, z + 1.0);
    let across = |d: f32| -d / 2.0 / radius + 0.5;
    [[x, y, z, across(x), across(z)], [x, y, z1, across(x), across(z1)], [x1, y, z1, across(x1), across(z1)], [x1, y, z, across(x1), across(z)]]
}

/// The shadows' triangles, relative to the camera.
pub fn quads(shadows: &[Shadow], camera: DVec3, ground: &impl Ground, ambient: f32) -> Vec<ShadowVertex> {
    let mut out = Vec::new();
    for s in shadows {
        let feet = (s.feet - camera).as_vec3() + Vec3::Y * LIFT;
        for piece in pieces(s, s.feet.distance_squared(camera), ground, ambient) {
            let v = corners(&piece, s.radius).map(|[x, y, z, u, v]| ShadowVertex { position: (feet + Vec3::new(x, y, z)).to_array(), uv: [u, v], alpha: piece.alpha });
            out.extend([v[0], v[1], v[2], v[0], v[2], v[3]]);
        }
    }
    out
}

/// The world as a shadow sees it: blocks from `world` through the look's `table`, light from
/// `light` (`[block, sky]` at a cell's centre) less `sky_darken` on the sky's.
pub struct WorldGround<'a, L: Fn(DVec3) -> [f32; 2]> {
    pub world: &'a World,
    pub table: &'a BlockTable,
    pub light: L,
    pub sky_darken: i32,
}

impl<L: Fn(DVec3) -> [f32; 2]> Ground for WorldGround<'_, L> {
    fn floor(&self, x: i32, y: i32, z: i32) -> bool {
        let Some(chunk) = self.world.get(x >> 4, z >> 4) else { return false };
        let block = self.table.get(chunk.read().block(x, y, z));
        block.light.full_collision && block.shape != crate::blocks::Shape::None
    }

    fn brightness(&self, x: i32, y: i32, z: i32) -> u8 {
        let [block, sky] = (self.light)(DVec3::new(f64::from(x), f64::from(y), f64::from(z)) + 0.5);
        (block as i32).max(sky as i32 - self.sky_darken).clamp(0, 15) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Flat;

    impl Ground for Flat {
        fn floor(&self, _: i32, y: i32, _: i32) -> bool {
            y < 64
        }

        fn brightness(&self, _: i32, _: i32, _: i32) -> u8 {
            15
        }
    }

    #[test]
    fn a_shadow_lies_on_the_ground_under_the_entity_and_fades() {
        let shadow = Shadow { feet: DVec3::new(0.5, 64.0, 0.5), radius: 0.4, strength: 1.0 };
        let near = pieces(&shadow, 9.0, &Flat, 0.0);
        // The radius stays inside the cell: a piece on the ground's top, and a fainter one a block
        // down (Java does not look at the cell between).
        assert_eq!(near.iter().map(|p| p.relative).collect::<Vec<_>>(), [[-0.5, -1.0, -0.5], [-0.5, 0.0, -0.5]]);
        assert!(near[0].alpha < near[1].alpha);
        // The blob's centre is under the feet: the cell's corners lie just outside the image.
        let c = corners(&near[1], shadow.radius);
        assert!((c[0][3] - 1.125).abs() < 1e-6 && (c[2][3] + 0.125).abs() < 1e-6, "{c:?}");
        // Standing higher thins it; 16 blocks from the camera there is none.
        let raised = pieces(&Shadow { feet: DVec3::new(0.5, 64.4, 0.5), ..shadow }, 9.0, &Flat, 0.0);
        assert!(raised[0].alpha < near[1].alpha && raised[0].alpha > 0.0);
        assert!(pieces(&shadow, 256.0, &Flat, 0.0).is_empty());
    }
}
