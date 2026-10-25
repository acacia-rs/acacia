//! Java's `LegacyRandomSource` (`java.util.Random`'s LCG) and `Mth`'s table sine, as the client's
//! particle code draws them.

use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct JavaRandom {
    state: u64,
    gaussian: Option<f64>,
}

const MULTIPLIER: u64 = 0x5_DEEC_E66D;
const MASK: u64 = (1 << 48) - 1;

impl JavaRandom {
    pub fn new(seed: i64) -> JavaRandom {
        JavaRandom { state: (seed as u64 ^ MULTIPLIER) & MASK, gaussian: None }
    }

    fn bits(&mut self, count: u32) -> i32 {
        self.state = self.state.wrapping_mul(MULTIPLIER).wrapping_add(11) & MASK;
        (self.state >> (48 - count)) as i32
    }

    pub fn next_int(&mut self) -> i32 {
        self.bits(32)
    }

    /// `nextInt(bound)`; 0 for a bound below 1 (Java throws), without drawing.
    pub fn below(&mut self, bound: i32) -> i32 {
        if bound <= 0 {
            return 0;
        }
        if bound & (bound - 1) == 0 {
            return ((i64::from(bound) * i64::from(self.bits(31))) >> 31) as i32;
        }
        loop {
            let bits = self.bits(31);
            let value = bits % bound;
            // Java's signed overflow is part of the rejection rule.
            if bits.wrapping_sub(value).wrapping_add(bound - 1) >= 0 {
                return value;
            }
        }
    }

    pub fn next_long(&mut self) -> i64 {
        // The low int is sign-extended before the add.
        (i64::from(self.bits(32)) << 32).wrapping_add(i64::from(self.bits(32)))
    }

    pub fn boolean(&mut self) -> bool {
        self.bits(1) != 0
    }

    pub fn float(&mut self) -> f32 {
        self.bits(24) as f32 * (1.0 / 16_777_216.0)
    }

    pub fn double(&mut self) -> f64 {
        let value = (i64::from(self.bits(26)) << 27) + i64::from(self.bits(27));
        value as f64 * (1.0 / 9_007_199_254_740_992.0)
    }

    /// `MarsagliaPolarGaussian`. Its `Math.log` is HotSpot's intrinsic, which glibc's `ln` matches in all
    /// but ~0.1% of draws (fdlibm/`StrictMath`, as `java.util.Random` uses, misses ~7%).
    pub fn gaussian(&mut self) -> f64 {
        if let Some(cached) = self.gaussian.take() {
            return cached;
        }
        loop {
            let x = 2.0 * self.double() - 1.0;
            let y = 2.0 * self.double() - 1.0;
            let radius = x * x + y * y;
            if radius >= 1.0 || radius == 0.0 {
                continue;
            }
            let multiplier = (-2.0 * radius.ln() / radius).sqrt();
            self.gaussian = Some(y * multiplier);
            return x * multiplier;
        }
    }

    /// `±1` from `nextBoolean`.
    pub fn sign(&mut self) -> f64 {
        if self.boolean() { 1.0 } else { -1.0 }
    }
}

/// `Mth.sin`: the 65536-entry float table, indexed by the angle as Java casts it.
pub fn mth_sin(angle: f64) -> f32 {
    static TABLE: OnceLock<Box<[f32]>> = OnceLock::new();
    const SCALE: f64 = 10430.378350470453;
    let table = TABLE.get_or_init(|| (0..65536).map(|i| (f64::from(i) / SCALE).sin() as f32).collect());
    table[((angle * SCALE) as i64 & 65535) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_follow_java_util_random() {
        // new java.util.Random(42): nextInt() -1170105035, nextInt(10) 0, nextLong() 3851010212393627394 ... (JDK).
        let mut r = JavaRandom::new(42);
        assert_eq!(r.next_int(), -1170105035);
        let mut r = JavaRandom::new(42);
        assert_eq!((r.below(10), r.below(10), r.below(10)), (0, 3, 8));
        let mut r = JavaRandom::new(42);
        assert_eq!(r.float(), 0.7275637);
        let mut r = JavaRandom::new(42);
        assert_eq!(r.double(), 0.7275636800328681);
        let mut r = JavaRandom::new(42);
        // HotSpot Math.log (JDK 25); java.util.Random's StrictMath gives ...0547.
        assert_eq!(r.gaussian(), 1.141905315473055);
        let mut r = JavaRandom::new(0);
        assert_eq!(r.next_long(), -4962768465676381896);
        // The second gaussian comes from the cache, then draws resume (JDK 21).
        let mut r = JavaRandom::new(7);
        assert_eq!((r.gaussian(), r.gaussian(), r.below(6), r.boolean()), (0.8452060657049847, 0.9128761787534405, 4, false));
    }

    #[test]
    fn the_sine_table_wraps_like_javas() {
        assert_eq!(mth_sin(0.0), 0.0);
        assert!((mth_sin(std::f64::consts::FRAC_PI_2) - 1.0).abs() < 1e-6);
        assert_eq!(mth_sin(std::f64::consts::TAU + 0.5), mth_sin(0.5));
    }
}
