//! Java's `LegacyRandomSource` (the `java.util.Random` generator), as the client seeds it for
//! weather columns, rain splashes and lightning bolts.

const MULTIPLIER: i64 = 0x5_DEEC_E66D;
const MASK: i64 = (1 << 48) - 1;

#[derive(Debug, Clone)]
pub struct JavaRandom {
    seed: i64,
    gaussian: Option<f64>,
}

impl JavaRandom {
    pub fn new(seed: i64) -> JavaRandom {
        JavaRandom { seed: (seed ^ MULTIPLIER) & MASK, gaussian: None }
    }

    /// `setSeed`, which also drops a cached gaussian.
    pub fn set_seed(&mut self, seed: i64) {
        *self = JavaRandom::new(seed);
    }

    pub fn next(&mut self, bits: u32) -> i32 {
        self.seed = self.seed.wrapping_mul(MULTIPLIER).wrapping_add(11) & MASK;
        (self.seed >> (48 - bits)) as i32
    }

    pub fn next_int(&mut self, bound: i32) -> i32 {
        if bound.isolate_lowest_one() == bound {
            return ((i64::from(bound) * i64::from(self.next(31))) >> 31) as i32;
        }
        loop {
            let bits = self.next(31);
            let value = bits % bound;
            if bits.wrapping_sub(value).wrapping_add(bound - 1) >= 0 {
                return value;
            }
        }
    }

    pub fn next_long(&mut self) -> i64 {
        let high = i64::from(self.next(32)) << 32;
        // The low int is sign-extended before the add, as Java does.
        high.wrapping_add(i64::from(self.next(32)))
    }

    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 / (1 << 24) as f32
    }

    pub fn next_double(&mut self) -> f64 {
        let value = (i64::from(self.next(26)) << 27) + i64::from(self.next(27));
        value as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// `MarsagliaPolarGaussian`: pairs, the second cached for the next call.
    pub fn next_gaussian(&mut self) -> f64 {
        if let Some(value) = self.gaussian.take() {
            return value;
        }
        loop {
            let x = 2.0 * self.next_double() - 1.0;
            let y = 2.0 * self.next_double() - 1.0;
            let radius = x * x + y * y;
            if radius < 1.0 && radius != 0.0 {
                let multiplier = (-2.0 * radius.ln() / radius).sqrt();
                self.gaussian = Some(y * multiplier);
                return x * multiplier;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_match_java_util_random() {
        // java.util.Random's, printed by jshell.
        let mut r = JavaRandom::new(42);
        assert_eq!([r.next_int(11), r.next_int(11), r.next_int(11), r.next_int(31), r.next_int(16)], [7, 4, 8, 23, 4]);
        let mut r = JavaRandom::new(0);
        assert_eq!((r.next_long(), r.next_double(), r.next_float()), (-4962768465676381896, 0.24053641567148587, 0.6374174));
        let mut r = JavaRandom::new(0);
        assert_eq!((r.next_gaussian(), r.next_gaussian()), (0.8025330637390305, -0.9015460884175122));
        // MarsagliaPolarGaussian takes Math.log, an ulp off java.util.Random's StrictMath.log here
        // (1.1419053154730547, 0.9194079489827879); the game's draw is this one.
        let mut r = JavaRandom::new(42);
        assert_eq!((r.next_gaussian(), r.next_gaussian()), (1.141905315473055, 0.919407948982788));
    }
}
