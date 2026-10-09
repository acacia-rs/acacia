//! The round shadows under entities, as Java's `EntityRenderDispatcher.renderShadow` lays them:
//! under each entity, every cell within its radius that rests on a full cube gets a quad on the
//! ground, mapped so the blob stays centred under the entity; the shadow thins with the entity's
//! height above that ground and with its distance from the camera, gone at 16 blocks.

use acacia_world::{BlockFlags, World};
use glam::{DVec3, Vec3};

/// Blocks from the camera at which shadows have faded out.
const REACH: f64 = 16.0;
/// Lifts the quads off the ground they lie on.
const LIFT: f32 = 0.01;
const FEET_SLACK: f64 = 0.01;

/// One entity's shadow: its feet and the blob's radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    pub feet: DVec3,
    pub radius: f32,
}

/// A vertex relative to the camera; `uv` runs 0 to 1 across the blob.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShadowVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub alpha: f32,
}

/// The shadows' triangles. `full_cube(x, y, z)` is whether that block is one.
pub fn quads(shadows: &[Shadow], camera: DVec3, full_cube: impl Fn(i32, i32, i32) -> bool) -> Vec<ShadowVertex> {
    let mut out = Vec::new();
    for s in shadows {
        let strength = (1.0 - s.feet.distance_squared(camera) / (REACH * REACH)) as f32;
        if strength <= 0.0 || s.radius <= 0.0 {
            continue;
        }
        let r = f64::from(s.radius);
        let depth = f64::from((strength / 0.5).min(s.radius));
        let cells = |lo: f64, hi: f64| lo.floor() as i32..=hi.floor() as i32;
        // Feet arrive as f32: an entity on the ground may be a hair under its cell.
        for y in cells(s.feet.y - depth, s.feet.y + FEET_SLACK) {
            // Thinner the higher the entity stands over this ground.
            let alpha = ((strength - (s.feet.y - f64::from(y)) as f32 * 0.5) * 0.5).min(1.0);
            if alpha <= 0.0 {
                continue;
            }
            for x in cells(s.feet.x - r, s.feet.x + r) {
                for z in cells(s.feet.z - r, s.feet.z + r) {
                    if full_cube(x, y - 1, z) && !full_cube(x, y, z) {
                        quad(&mut out, s, camera, [x, y, z], alpha);
                    }
                }
            }
        }
    }
    out
}

/// The top of the cube under cell `[x, y, z]`, textured by where it lies under the entity.
fn quad(out: &mut Vec<ShadowVertex>, s: &Shadow, camera: DVec3, [x, y, z]: [i32; 3], alpha: f32) {
    let corner = |dx: i32, dz: i32| {
        let at = DVec3::new(f64::from(x + dx), f64::from(y), f64::from(z + dz));
        let across = |d: f64| (d / 2.0 / f64::from(s.radius) + 0.5) as f32;
        ShadowVertex { position: ((at - camera).as_vec3() + Vec3::Y * LIFT).to_array(), uv: [across(at.x - s.feet.x), across(at.z - s.feet.z)], alpha }
    };
    let v = [corner(0, 0), corner(0, 1), corner(1, 1), corner(1, 0)];
    out.extend([v[0], v[1], v[2], v[0], v[2], v[3]]);
}

/// Whether the block at a position is a full cube, read from `world`; unloaded chunks have none.
pub fn full_cubes(world: &World) -> impl Fn(i32, i32, i32) -> bool + '_ {
    move |x, y, z| {
        let Some(chunk) = world.get(x >> 4, z >> 4) else { return false };
        let id = chunk.read().block(x, y, z);
        world.registry().get(id).is_some_and(|state| state.flags.contains(BlockFlags::FULL_CUBE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shadow_lies_on_the_ground_under_the_entity_and_fades() {
        let ground = |_: i32, y: i32, _: i32| y < 64;
        let shadow = Shadow { feet: DVec3::new(0.5, 64.0, 0.5), radius: 0.4 };
        let near = quads(&[shadow], DVec3::new(0.5, 65.6, 3.5), ground);
        // The radius stays inside the cell: one quad, on the ground's top.
        assert_eq!(near.len(), 6);
        assert!(near.iter().all(|v| (v.position[1] - (64.0 - 65.6 + LIFT)).abs() < 1e-4));
        // The blob's centre is under the feet: the cell's corners lie just outside the image.
        let close = |uv: [f32; 2], want: f32| uv.iter().all(|c| (c - want).abs() < 1e-5);
        assert!(close(near[0].uv, -0.125) && close(near[2].uv, 1.125), "{:?} {:?}", near[0].uv, near[2].uv);
        // Standing a block up thins it; 16 blocks from the camera there is none.
        let raised = quads(&[Shadow { feet: DVec3::new(0.5, 64.9, 0.5), ..shadow }], DVec3::new(0.5, 65.6, 3.5), ground);
        assert!(raised[0].alpha < near[0].alpha && raised[0].alpha > 0.0);
        assert!(quads(&[shadow], DVec3::new(0.5, 64.0, 20.0), ground).is_empty());
    }
}
