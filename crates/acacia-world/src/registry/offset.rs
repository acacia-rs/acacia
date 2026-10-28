//! BDS's random offset of plant selection boxes (flowers, grass, corals...): a per-column shift from a position
//! hash (144c50eb0), ported statically; docs/selection-shapes.md.

/// Which axes a state's selection box is shifted on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionOffset {
    #[default]
    None,
    /// ±0.25 on x and z in 16 steps.
    Xz,
    /// As `Xz`, and -0.2..0 on y (short grass, fern, dry grass).
    Xyz,
}

const GOLDEN: u64 = 0x9e37_79b9_7f4a_7c15;
const SILVER: u64 = 0x6a09_e667_f3bc_c909;

impl SelectionOffset {
    pub(super) fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Xz,
            2 => Self::Xyz,
            _ => Self::None,
        }
    }

    /// The shift of the block at column (`x`, `z`); y does not enter the hash.
    pub fn at(self, x: i32, z: i32) -> [f32; 3] {
        let y = match self {
            Self::None => return [0.0; 3],
            Self::Xz => (0.0, 0.0),
            Self::Xyz => (-0.2, 0.0),
        };
        let seed = (i64::from(x).wrapping_mul(0x2f_c20f) ^ i64::from(z).wrapping_mul(0x6eb_fff5)) as u64;
        let s = (seed.wrapping_mul(0x285_b825).wrapping_add(0xb)).wrapping_mul(seed) >> 16;
        let lo = (s as u32 as i32 as i64 as u64) ^ SILVER;
        let (mut s0, mut s1) = (mix13(lo), mix13(lo.wrapping_add(GOLDEN)));
        if s0 == 0 && s1 == 0 {
            (s0, s1) = (GOLDEN, SILVER);
        }
        let mut axis = |(lo, hi): (f32, f32)| {
            let r = (s0.wrapping_add(s1).rotate_left(17).wrapping_add(s0) >> 40) as f32 * (1.0 / (1u32 << 24) as f32);
            s1 ^= s0;
            s0 = s0.rotate_left(49) ^ s1 ^ (s1 << 21);
            s1 = s1.rotate_left(28);
            if lo < hi { lo + (r * 16.0).floor() * ((hi - lo) / 15.0) } else { lo }
        };
        let dx = axis((-0.25, 0.25));
        let dy = axis(y);
        let dz = axis((-0.25, 0.25));
        [dx, dy, dz]
    }
}

fn mix13(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Values from research/bds-selection-shapes/tools/random_offset.py, the reference port.
    #[test]
    fn matches_the_reference_port() {
        let cases = [
            ((0, 0), [-0.183_333_34, -0.04, -0.116_666_67]),
            ((1, 0), [0.25, -0.106_666_67, -0.016_666_668]),
            ((0, 1), [0.216_666_67, -0.2, 0.05]),
            ((-7, 13), [0.216_666_67, -0.12, -0.05]),
            ((100, -250), [0.216_666_67, -0.173_333_33, -0.116_666_67]),
        ];
        for ((x, z), want) in cases {
            let got = SelectionOffset::Xyz.at(x, z);
            assert!(got.iter().zip(want).all(|(g, w)| (g - w).abs() < 1e-6), "({x}, {z}): {got:?} vs {want:?}");
            let xz = SelectionOffset::Xz.at(x, z);
            assert_eq!((xz[0], xz[1], xz[2]), (got[0], 0.0, got[2]));
        }
        assert_eq!(SelectionOffset::None.at(3, 4), [0.0; 3]);
    }
}
