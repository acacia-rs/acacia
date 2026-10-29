//! glibc's `sinf`/`cosf` (sysdeps/ieee754/flt-32, the algorithm since 2.28), which Linux BDS links and builds
//! its sine table with (`math::sin_table`). Reproduces all 65536 entries dumped from a live BDS 1.26.52.3, with
//! and without fused multiply-adds (glibc picks an FMA build on FMA CPUs).

struct SinCos {
    c: [f64; 5],
    s: [f64; 3],
}

const SIGN: [f64; 4] = [1.0, -1.0, -1.0, 1.0];
const HPI_INV: f64 = f64::from_bits(0x4164_5F30_6DC9_C883); // 2/pi * 2^24
const HPI: f64 = f64::from_bits(0x3FF9_21FB_5444_2D18);
const S: [f64; 3] = [
    f64::from_bits(0xBFC5_5554_5995_A603),
    f64::from_bits(0x3F81_1076_0523_0BC4),
    f64::from_bits(0xBF29_94EB_3774_CF24),
];
const C: [f64; 5] = [
    1.0,
    f64::from_bits(0xBFDF_FFFF_FD0C_621C),
    f64::from_bits(0x3FA5_5553_E106_8F19),
    f64::from_bits(0xBF56_C087_E89A_359D),
    f64::from_bits(0x3EF9_9343_027B_F8C3),
];
const TABLE: [SinCos; 2] = [
    SinCos { c: C, s: S },
    SinCos { c: [-C[0], -C[1], -C[2], -C[3], -C[4]], s: S },
];

const PIO4: f32 = f32::from_bits(0x3F49_0FDB);
const TINY: f32 = f32::from_bits(0x3980_0000); // 2^-12

fn top12(x: f32) -> u32 {
    (x.to_bits() >> 20) & 0x7ff
}

fn poly(x: f64, x2: f64, p: &SinCos, n: i32) -> f32 {
    if n & 1 == 0 {
        let x3 = x * x2;
        let s1 = p.s[1] + x2 * p.s[2];
        let x7 = x3 * x2;
        let s = x + x3 * p.s[0];
        (s + x7 * s1) as f32
    } else {
        let x4 = x2 * x2;
        let c2 = p.c[3] + x2 * p.c[4];
        let c1 = p.c[0] + x2 * p.c[1];
        let x6 = x4 * x2;
        let c = c1 + x4 * p.c[2];
        (c + x6 * c2) as f32
    }
}

/// (reduced argument times its quadrant's sign, quadrant, coefficient set) for `|y| < 120`.
fn reduce(y: f32) -> (f64, f64, i32, &'static SinCos) {
    let x = f64::from(y);
    let n = (((x * HPI_INV) as i32).wrapping_add(0x80_0000)) >> 24;
    let r = x - f64::from(n) * HPI;
    (r * SIGN[(n & 3) as usize], r * r, n, &TABLE[usize::from(n & 2 != 0)])
}

pub(crate) fn sinf(y: f32) -> f32 {
    if top12(y) < top12(PIO4) {
        if top12(y) < top12(TINY) {
            return y;
        }
        let x = f64::from(y);
        return poly(x, x * x, &TABLE[0], 0);
    }
    if top12(y) >= top12(120.0) {
        return y.sin();
    }
    let (x, x2, n, p) = reduce(y);
    poly(x, x2, p, n)
}

pub(crate) fn cosf(y: f32) -> f32 {
    if top12(y) < top12(PIO4) {
        if top12(y) < top12(TINY) {
            return 1.0;
        }
        let x = f64::from(y);
        return poly(x, x * x, &TABLE[0], 1);
    }
    if top12(y) >= top12(120.0) {
        return y.cos();
    }
    let (x, x2, n, p) = reduce(y);
    poly(x, x2, p, n ^ 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Linux links glibc, whose `sinf`/`cosf` this is.
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    #[test]
    fn matches_glibc() {
        for i in 0..2_000_000 {
            let x = i as f32 * 7e-6 - 7.0;
            assert_eq!(sinf(x).to_bits(), x.sin().to_bits(), "sinf({x})");
            assert_eq!(cosf(x).to_bits(), x.cos().to_bits(), "cosf({x})");
        }
    }

    #[test]
    fn small_arguments() {
        assert_eq!(sinf(1e-5), 1e-5);
        assert_eq!(cosf(1e-5), 1.0);
        assert_eq!(sinf(0.0), 0.0);
    }
}
