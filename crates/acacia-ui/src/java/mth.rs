//! Java's arithmetic as the GUI code uses it: `Mth`, `ARGB` and `java.util.Random`, rounding and
//! overflowing exactly as the JVM does.

/// `Mth.floor`.
pub fn floor(x: f32) -> i32 {
    let i = x as i32;
    if x < i as f32 { i - 1 } else { i }
}

/// `Mth.floor(double)`.
pub fn floor_d(x: f64) -> i32 {
    let i = x as i32;
    if x < f64::from(i) { i - 1 } else { i }
}

/// `Mth.ceil`.
pub fn ceil(x: f32) -> i32 {
    let i = x as i32;
    if x > i as f32 { i + 1 } else { i }
}

/// `Mth.ceil(double)`.
pub fn ceil_d(x: f64) -> i32 {
    let i = x as i32;
    if x > f64::from(i) { i + 1 } else { i }
}

/// `Mth.clamp(float)`.
pub fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo { lo } else { v.min(hi) }
}

/// `Mth.lerp(float)`.
pub fn lerp(t: f32, a: f32, b: f32) -> f32 {
    a + t * (b - a)
}

/// `Mth.lerpDiscrete`.
pub fn lerp_discrete(alpha: f32, p0: i32, p1: i32) -> i32 {
    p0 + floor(alpha * (p1 - p0 - 1) as f32) + i32::from(alpha > 0.0)
}

/// `Mth.hsvToArgb`.
pub fn hsv_to_argb(hue: f32, saturation: f32, value: f32, alpha: u32) -> u32 {
    let h = (hue * 6.0) as i32 % 6;
    let f = hue * 6.0 - h as f32;
    let p = value * (1.0 - saturation);
    let q = value * (1.0 - f * saturation);
    let t = value * (1.0 - (1.0 - f) * saturation);
    let (r, g, b) = match h {
        0 => (value, t, p),
        1 => (q, value, p),
        2 => (p, value, t),
        3 => (p, q, value),
        4 => (t, p, value),
        _ => (value, p, q),
    };
    let c = |v: f32| ((v * 255.0) as i32).clamp(0, 255) as u32;
    argb(alpha, c(r), c(g), c(b))
}

pub const WHITE: u32 = 0xFFFF_FFFF;
pub const BLACK: u32 = 0xFF00_0000;

/// `ARGB.color(a, r, g, b)`.
pub fn argb(a: u32, r: u32, g: u32, b: u32) -> u32 {
    (a & 255) << 24 | (r & 255) << 16 | (g & 255) << 8 | (b & 255)
}

/// `ARGB.alpha`.
pub fn alpha(c: u32) -> u32 {
    c >> 24
}

/// `ARGB.as8BitChannel`.
pub fn as8(v: f32) -> u32 {
    floor(v * 255.0) as u32
}

/// `ARGB.white(float)`.
pub fn white(alpha: f32) -> u32 {
    (as8(alpha) << 24) | 0xFF_FFFF
}

/// `ARGB.color(int alpha, int rgb)`.
pub fn with_alpha(alpha: u32, rgb: u32) -> u32 {
    (alpha & 255) << 24 | (rgb & 0xFF_FFFF)
}

/// `ARGB.colorFromFloat`.
pub fn color_from_float(a: f32, r: f32, g: f32, b: f32) -> u32 {
    argb(as8(a), as8(r), as8(g), as8(b))
}

/// `ARGB.multiply`.
pub fn multiply(l: u32, r: u32) -> u32 {
    if l == WHITE {
        return r;
    }
    if r == WHITE {
        return l;
    }
    let c = |s: u32| ((l >> s) & 255) * ((r >> s) & 255) / 255;
    argb(c(24), c(16), c(8), c(0))
}

/// `ARGB.scaleRGB(color, scale)`: each colour channel times `scale`, alpha kept.
pub fn scale_rgb(c: u32, scale: f32) -> u32 {
    let ch = |s: u32| ((((c >> s) & 255) as f32 * scale) as i32).clamp(0, 255) as u32;
    argb(c >> 24, ch(16), ch(8), ch(0))
}

/// `java.util.Random` (`LegacyRandomSource` draws the same numbers).
#[derive(Debug, Clone)]
pub struct JavaRandom(u64);

impl JavaRandom {
    const MULTIPLIER: u64 = 0x5_DEEC_E66D;
    const MASK: u64 = (1 << 48) - 1;

    pub fn new(seed: i64) -> JavaRandom {
        JavaRandom((seed as u64 ^ Self::MULTIPLIER) & Self::MASK)
    }

    fn bits(&mut self, count: u32) -> i32 {
        self.0 = self.0.wrapping_mul(Self::MULTIPLIER).wrapping_add(11) & Self::MASK;
        (self.0 >> (48 - count)) as i32
    }

    pub fn next_int(&mut self) -> i32 {
        self.bits(32)
    }

    /// `nextInt(bound)`; `bound` must be positive.
    pub fn next_int_bound(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "bound must be positive");
        if bound & (bound - 1) == 0 {
            return ((i64::from(bound) * i64::from(self.bits(31))) >> 31) as i32;
        }
        loop {
            let bits = self.bits(31);
            let value = bits % bound;
            // Java's signed overflow is the rejection test.
            if bits.wrapping_sub(value).wrapping_add(bound - 1) >= 0 {
                return value;
            }
        }
    }

    pub fn next_float(&mut self) -> f32 {
        self.bits(24) as f32 * (1.0 / 16_777_216.0)
    }

    pub fn next_long(&mut self) -> i64 {
        (i64::from(self.bits(32)) << 32).wrapping_add(i64::from(self.bits(32)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_random_matches_the_jdk() {
        // From jshell on JDK 25.
        let mut r = JavaRandom::new(42);
        assert_eq!((r.next_int(), r.next_int_bound(10)), (-1_170_105_035, 3));
        let mut r = JavaRandom::new(0);
        assert_eq!((r.next_int_bound(2), r.next_int_bound(3), r.next_float()), (1, 1, 0.240_536_39));
    }

    #[test]
    fn floors_and_colours() {
        assert_eq!((floor(-0.5), ceil(-0.5), floor(2.0), ceil(2.1)), (-1, 0, 2, 3));
        assert_eq!(white(0.5), 0x7FFF_FFFF);
        assert_eq!(multiply(0x80FF_FFFF, 0xFF80_8080), 0x8080_8080);
        assert_eq!(lerp_discrete(0.0, 0, 10), 0);
        assert_eq!(lerp_discrete(1.0, 0, 10), 10);
    }
}
