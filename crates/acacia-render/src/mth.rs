//! Java's `Mth` trigonometry, which rounds differently from libm: a 65536-entry sine table and a
//! table-driven arctangent. Ported from Horizon's `horizon-core` `geometry::trig` (MIT).

use std::sync::OnceLock;

const SCALE: f64 = 10430.378350470453;

fn sine_table() -> &'static [f32] {
    static SINE: OnceLock<Box<[f32]>> = OnceLock::new();
    SINE.get_or_init(|| (0..65536).map(|i| (f64::from(i) / SCALE).sin() as f32).collect())
}

/// `Mth.sin(double)`.
pub fn sin(angle: f64) -> f32 {
    sine_table()[((angle * SCALE) as i64 & 65535) as usize]
}

/// `Mth.cos(double)`.
pub fn cos(angle: f64) -> f32 {
    sine_table()[((angle * SCALE + 16384.0) as i64 & 65535) as usize]
}

/// `Mth.atan2`.
pub fn atan2(mut y: f64, mut x: f64) -> f64 {
    static TABLES: OnceLock<(Vec<f64>, Vec<f64>)> = OnceLock::new();
    let (asin, cos) = TABLES.get_or_init(|| {
        (0..257)
            .map(|i| {
                let a = fdlibm_asin(f64::from(i) / 256.0);
                // HotSpot's x86-64 Math.cos intrinsic rounds entry 61 one ulp above libm.
                let c = if i == 61 { f64::from_bits(0x3fef_140a_0a08_6af1) } else { a.cos() };
                (a, c)
            })
            .unzip()
    });
    let frac_bias = f64::from_bits(4_805_340_802_404_319_232);
    let d2 = x * x + y * y;
    if d2.is_nan() {
        return f64::NAN;
    }
    let negative_y = y < 0.0;
    if negative_y {
        y = -y;
    }
    let negative_x = x < 0.0;
    if negative_x {
        x = -x;
    }
    let steep = y > x;
    if steep {
        std::mem::swap(&mut x, &mut y);
    }
    // Mth.fastInvSqrt
    let half = 0.5 * d2;
    let mut r = f64::from_bits((6_910_469_410_427_058_090i64 - (d2.to_bits() as i64 >> 1)) as u64);
    r *= 1.5 - half * r * r;
    x *= r;
    y *= r;
    let yp = frac_bias + y;
    let index = yp.to_bits() as i32 as usize;
    let s_phi = yp - frac_bias;
    let sd = y * cos[index] - x * s_phi;
    let mut theta = asin[index] + (6.0 + sd * sd) * sd * 0.166_666_666_666_666_66;
    if steep {
        theta = std::f64::consts::FRAC_PI_2 - theta;
    }
    if negative_x {
        theta = std::f64::consts::PI - theta;
    }
    if negative_y {
        theta = -theta;
    }
    theta
}

/// fdlibm `__ieee754_asin`, as `StrictMath.asin` (and so `Math.asin`) computes it.
#[allow(clippy::excessive_precision, clippy::approx_constant, clippy::eq_op)]
fn fdlibm_asin(x: f64) -> f64 {
    const PIO2_HI: f64 = 1.570_796_326_794_896_558_00e+00;
    const PIO2_LO: f64 = 6.123_233_995_736_766_035_87e-17;
    const PIO4_HI: f64 = 7.853_981_633_974_482_789_99e-01;
    const PS0: f64 = 1.666_666_666_666_666_574_15e-01;
    const PS1: f64 = -3.255_658_186_224_009_154_05e-01;
    const PS2: f64 = 2.012_125_321_348_629_258_81e-01;
    const PS3: f64 = -4.005_553_450_067_941_140_27e-02;
    const PS4: f64 = 7.915_349_942_898_145_321_76e-04;
    const PS5: f64 = 3.479_331_075_960_211_675_70e-05;
    const QS1: f64 = -2.403_394_911_734_414_218_78e+00;
    const QS2: f64 = 2.020_945_760_233_505_694_71e+00;
    const QS3: f64 = -6.882_839_716_054_532_930_30e-01;
    const QS4: f64 = 7.703_815_055_590_193_527_91e-02;
    let bits = x.to_bits();
    let hx = (bits >> 32) as i32;
    let ix = hx & 0x7fff_ffff;
    let ratio = |t: f64| {
        let p = t * (PS0 + t * (PS1 + t * (PS2 + t * (PS3 + t * (PS4 + t * PS5)))));
        let q = 1.0 + t * (QS1 + t * (QS2 + t * (QS3 + t * QS4)));
        (p, q)
    };
    if ix >= 0x3ff0_0000 {
        if ((ix - 0x3ff0_0000) as u32 | bits as u32) == 0 {
            return x * PIO2_HI + x * PIO2_LO;
        }
        return (x - x) / (x - x);
    } else if ix < 0x3fe0_0000 {
        if ix < 0x3e40_0000 && 1.0e300 + x > 1.0 {
            return x;
        }
        let (p, q) = ratio(x * x);
        return x + x * (p / q);
    }
    let t = (1.0 - x.abs()) * 0.5;
    let (p, q) = ratio(t);
    let s = t.sqrt();
    let t = if ix >= 0x3fef_3333 {
        PIO2_HI - (2.0 * (s + s * (p / q)) - PIO2_LO)
    } else {
        let w = f64::from_bits(s.to_bits() & 0xffff_ffff_0000_0000);
        let c = (t - w * w) / (s + w);
        let p = 2.0 * s * (p / q) - (PIO2_LO - 2.0 * c);
        PIO4_HI - (p - (PIO4_HI - 2.0 * w))
    };
    if hx > 0 { t } else { -t }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_follow_libm_closely() {
        for (y, x) in [(1.0, 0.0), (0.0, 1.0), (-0.5, 0.25), (0.3, -0.9), (-1.0, -1.0), (0.1, 0.7)] {
            assert!((atan2(y, x) - f64::atan2(y, x)).abs() < 1e-6, "{y} {x}");
        }
        assert!(atan2(f64::NAN, 1.0).is_nan());
        for a in [0.0, 0.5, -2.0, 3.0] {
            assert!((sin(a) - (a as f32).sin()).abs() < 1e-3 && (cos(a) - (a as f32).cos()).abs() < 1e-3, "{a}");
        }
        assert_eq!((sin(0.0), cos(0.0)), (0.0, 1.0));
        for v in [0.0, 0.25, 0.5, 0.9, 0.99, 1.0, -0.3] {
            assert!((fdlibm_asin(v) - v.asin()).abs() < 1e-15, "{v}");
        }
    }
}
