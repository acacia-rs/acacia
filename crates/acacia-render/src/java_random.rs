//! Minecraft's `SingleThreadedRandomSource` (`java.util.Random`'s generator), for geometry the
//! game builds from a fixed seed.

pub struct JavaRandom(u64);

impl JavaRandom {
    pub fn new(seed: u64) -> Self {
        JavaRandom((seed ^ 0x5DEECE66D) & ((1 << 48) - 1))
    }

    pub fn next(&mut self, bits: u32) -> i32 {
        self.0 = (self.0.wrapping_mul(0x5DEECE66D).wrapping_add(0xB)) & ((1 << 48) - 1);
        (self.0 >> (48 - bits)) as i32
    }

    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 / (1 << 24) as f32
    }

    /// `BitRandomSource.nextDouble`: its unit is the float `1.110223E-16F`, not 2^-53.
    pub fn next_double(&mut self) -> f64 {
        let high = i64::from(self.next(26)) << 27;
        (high + i64::from(self.next(27))) as f64 * f64::from(1.110223e-16f32)
    }
}

#[test]
fn draws_match_java() {
    // new Random(42): nextInt() = -1170105035, then nextFloat() = 0.054665208.
    let mut r = JavaRandom::new(42);
    assert_eq!(r.next(32), -1170105035);
    assert_eq!(r.next_float(), 0.054665208);
}
