//! A lightning bolt's geometry, as 26.3's `LightningBoltRenderer.submit` builds it: a trunk of 8
//! segments 16 blocks tall, each jittering up to 5 blocks off the last, and 2 branches of 3
//! jittering up to 15, drawn 4 times at growing widths. Quads in emission order, relative to the
//! bolt's foot.

use glam::Vec3;

use crate::java_random::JavaRandom;

const SEGMENTS: usize = 8;
/// `setColor(0.45, 0.45, 0.5, 0.3)` as the vertex stores it (8 bits a channel, floored), RGBA.
pub const COLOR: [u8; 4] = [114, 114, 127, 76];

/// The four sides of a segment: (right x+, right z+, left x+, left z+).
const SIDES: [[bool; 4]; 4] = [[false, false, true, false], [true, false, true, true], [true, true, false, true], [false, true, false, false]];

/// `seed` is the bolt entity's (`LightningBolt.seed`, re-drawn on each flash).
pub fn bolt(seed: i64) -> Vec<[Vec3; 4]> {
    let (mut xs, mut zs) = ([0f32; SEGMENTS], [0f32; SEGMENTS]);
    let (mut x_off, mut z_off) = (0f32, 0f32);
    let mut random = JavaRandom::new(seed);
    for i in (0..SEGMENTS).rev() {
        (xs[i], zs[i]) = (x_off, z_off);
        x_off += (random.next_int(11) - 5) as f32;
        z_off += (random.next_int(11) - 5) as f32;
    }
    let mut out = Vec::with_capacity(4 * 14 * 4);
    for layer in 0..4 {
        let mut random = JavaRandom::new(seed);
        for branch in 0..3 {
            let trunk = branch == 0;
            let start = 7 - branch;
            let end = if trunk { 0 } else { start - 2 };
            let (mut sx, mut sz) = (xs[start] - x_off, zs[start] - z_off);
            for segment in (end..=start).rev() {
                let (ex, ez) = (sx, sz);
                let (bound, half) = if trunk { (11, 5) } else { (31, 15) };
                sx += (random.next_int(bound) - half) as f32;
                sz += (random.next_int(bound) - half) as f32;
                let mut top = 0.1 + layer as f32 * 0.2;
                let mut bottom = top;
                if trunk {
                    top *= segment as f32 * 0.1 + 1.0;
                    bottom *= (segment as i32 - 1) as f32 * 0.1 + 1.0;
                }
                let y0 = (segment * 16) as f32;
                let side = |positive: bool, r: f32| if positive { r } else { -r };
                for [rx, rz, lx, lz] in SIDES {
                    out.push([
                        Vec3::new(sx + side(rx, bottom), y0, sz + side(rz, bottom)),
                        Vec3::new(ex + side(rx, top), y0 + 16.0, ez + side(rz, top)),
                        Vec3::new(ex + side(lx, top), y0 + 16.0, ez + side(lz, top)),
                        Vec3::new(sx + side(lx, bottom), y0, sz + side(lz, bottom)),
                    ]);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn geometry_matches_the_games_vertex_digest() {
        // Horizon's capture of 26.3's LightningBoltRenderer (identity pose, big-endian xyz floats)
        // over these seeds: research/horizon gfx/lightning/tests.
        let mut seeds = vec![0, 1, -1, i64::MIN, i64::MAX, 0x123456789abcdef];
        let mut random = JavaRandom::new(42);
        seeds.extend((0..256).map(|_| random.next_long()));
        let mut digest = Sha256::new();
        for seed in seeds {
            for v in bolt(seed).into_iter().flatten() {
                v.to_array().iter().for_each(|c| digest.update(c.to_be_bytes()));
            }
        }
        let hex: String = digest.finalize().iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "a9ed25caa50b9aa340faa4bb417b3d533b061f0dce66514e4bb212931444dee6");
    }

    #[test]
    fn the_bolt_stands_on_its_foot_and_reaches_the_sky() {
        let quads = bolt(7);
        // 4 layers × (8 trunk + 3 + 3 branch segments) × 4 sides.
        assert_eq!(quads.len(), 4 * 14 * 4);
        let ys = || quads.iter().flatten().map(|v| v.y);
        assert_eq!((ys().fold(f32::MAX, f32::min), ys().fold(f32::MIN, f32::max)), (0.0, 128.0));
        let foot = quads.iter().flatten().filter(|v| v.y == 0.0).map(|v| v.x.abs().max(v.z.abs())).fold(0.0, f32::max);
        assert!(foot < 1.0, "the trunk ends at the strike: {foot}");
    }
}
