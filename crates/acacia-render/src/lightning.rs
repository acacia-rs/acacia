//! A lightning bolt's geometry, as Java's `LightningBoltRenderer` builds it: a trunk of 8 segments
//! 16 blocks tall, each jittering up to 5 blocks off the last, and 2 branches jittering up to 15,
//! drawn 4 times at growing widths as translucent quads (0.45, 0.45, 0.5 at alpha 0.3, added).

use glam::Vec3;

const SEGMENTS: usize = 8;
const SEGMENT_HEIGHT: f32 = 16.0;
pub const COLOR: [f32; 4] = [0.45, 0.45, 0.5, 0.3];

/// `java.util.Random`, which Java's bolt is seeded with (`RandomSource.create(seed)`).
struct JavaRandom(u64);

impl JavaRandom {
    fn new(seed: u64) -> Self {
        JavaRandom((seed ^ 0x5DEECE66D) & ((1 << 48) - 1))
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.0 = (self.0.wrapping_mul(0x5DEECE66D).wrapping_add(0xB)) & ((1 << 48) - 1);
        (self.0 >> (48 - bits)) as i32
    }

    fn next_int(&mut self, bound: i32) -> i32 {
        if bound & -bound == bound {
            return ((i64::from(bound) * i64::from(self.next(31))) >> 31) as i32;
        }
        loop {
            let bits = self.next(31);
            let value = bits % bound;
            if bits - value + (bound - 1) >= 0 {
                return value;
            }
        }
    }
}

/// Triangles (in blocks, relative to the bolt's foot) of the bolt with `seed`.
pub fn bolt(seed: u64) -> Vec<Vec3> {
    let mut trunk_x = [0.0f32; SEGMENTS];
    let mut trunk_z = [0.0f32; SEGMENTS];
    let (mut x, mut z) = (0.0f32, 0.0f32);
    let mut random = JavaRandom::new(seed);
    for h in (0..SEGMENTS).rev() {
        (trunk_x[h], trunk_z[h]) = (x, z);
        x += (random.next_int(11) - 5) as f32;
        z += (random.next_int(11) - 5) as f32;
    }
    let mut out = Vec::new();
    for layer in 0..4 {
        let mut random = JavaRandom::new(seed);
        for branch in 0..3 {
            let top = if branch > 0 { 7 - branch } else { 7 };
            let bottom = if branch > 0 { top - 2 } else { 0 };
            let (mut m, mut n) = (trunk_x[top] - x, trunk_z[top] - z);
            for o in (bottom..=top).rev() {
                let (p, q) = (m, n);
                let spread = if branch == 0 { 5 } else { 15 };
                m += (random.next_int(spread * 2 + 1) - spread) as f32;
                n += (random.next_int(spread * 2 + 1) - spread) as f32;
                let base = 0.1 + layer as f32 * 0.2;
                let (low, high) = match branch {
                    0 => (base * ((o as f32 - 1.0) * 0.1 + 1.0), base * (o as f32 * 0.1 + 1.0)),
                    _ => (base, base),
                };
                for corners in [[false, false, true, false], [true, false, true, true], [true, true, false, true], [false, true, false, false]] {
                    quad(&mut out, [m, n], [p, q], o as f32, high, low, corners);
                }
            }
        }
    }
    out
}

/// Java's `LightningBoltRenderer.quad`: from `(x1, z1)` at segment `o`'s foot to `(x2, z2)` a
/// segment up, `low` wide below and `high` above, on the side the four flags pick.
fn quad(out: &mut Vec<Vec3>, [x1, z1]: [f32; 2], [x2, z2]: [f32; 2], o: f32, high: f32, low: f32, [a, b, c, d]: [bool; 4]) {
    let side = |flag: bool, w: f32| if flag { w } else { -w };
    let (y1, y2) = (o * SEGMENT_HEIGHT, (o + 1.0) * SEGMENT_HEIGHT);
    let v = [
        Vec3::new(x1 + side(a, low), y1, z1 + side(b, low)),
        Vec3::new(x2 + side(a, high), y2, z2 + side(b, high)),
        Vec3::new(x2 + side(c, high), y2, z2 + side(d, high)),
        Vec3::new(x1 + side(c, low), y1, z1 + side(d, low)),
    ];
    out.extend([v[0], v[1], v[2], v[0], v[2], v[3]]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_random_matches_java() {
        // new java.util.Random(42): nextInt(11) ×3, nextInt(31), nextInt(16), printed by JDK 11.
        let mut r = JavaRandom::new(42);
        assert_eq!([r.next_int(11), r.next_int(11), r.next_int(11), r.next_int(31), r.next_int(16)], [7, 4, 8, 23, 4]);
    }

    #[test]
    fn the_bolt_stands_on_its_foot_and_reaches_the_sky() {
        let tris = bolt(7);
        // 4 layers × (8 trunk + 3 + 3 branch segments) × 4 quads × 6 vertices.
        assert_eq!(tris.len(), 4 * 14 * 4 * 6);
        let top = tris.iter().map(|v| v.y).fold(f32::MIN, f32::max);
        assert_eq!((tris.iter().map(|v| v.y).fold(f32::MAX, f32::min), top), (0.0, 128.0));
        let foot = tris.iter().filter(|v| v.y == 0.0).map(|v| v.x.abs().max(v.z.abs())).fold(0.0, f32::max);
        assert!(foot < 1.0, "the trunk ends at the strike: {foot}");
    }
}
