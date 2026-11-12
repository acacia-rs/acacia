//! Java's 2D `SimplexNoise` without offsets, seeded as `Biome` seeds its climate noises from a `LegacyRandomSource`.

/// x and y of `GradientNoise.GRADIENT`'s first twelve.
const GRADIENT: [[f64; 2]; 12] =
    [[1.0, 1.0], [-1.0, 1.0], [1.0, -1.0], [-1.0, -1.0], [1.0, 0.0], [-1.0, 0.0], [1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0], [0.0, 1.0], [0.0, -1.0]];

/// `java.util.Random`'s 48-bit generator, as `LegacyRandomSource` draws.
pub(crate) struct Lcg(i64);

const MULTIPLIER: i64 = 0x5_DEEC_E66D;
const MASK: i64 = (1 << 48) - 1;

impl Lcg {
    pub(crate) fn new(seed: i64) -> Lcg {
        Lcg((seed ^ MULTIPLIER) & MASK)
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.0 = self.0.wrapping_mul(MULTIPLIER).wrapping_add(11) & MASK;
        (self.0 >> (48 - bits)) as i32
    }

    fn below(&mut self, bound: i32) -> i32 {
        if bound & (bound - 1) == 0 {
            return ((i64::from(bound) * i64::from(self.next(31))) >> 31) as i32;
        }
        loop {
            let sample = self.next(31);
            let modulo = sample % bound;
            if sample.wrapping_sub(modulo).wrapping_add(bound - 1) >= 0 {
                return modulo;
            }
        }
    }
}

pub(crate) struct Simplex {
    permutation: [u8; 256],
}

impl Simplex {
    /// The next noise drawn from `random` (`new SimplexNoise(random, true)`).
    pub(crate) fn draw(random: &mut Lcg) -> Simplex {
        // The three offsets the noise draws and then discards: two draws a double.
        for _ in 0..6 {
            random.next(26);
        }
        let mut permutation: [u8; 256] = std::array::from_fn(|i| i as u8);
        for i in 0..256 {
            permutation.swap(i, i + random.below(256 - i as i32) as usize);
        }
        Simplex { permutation }
    }

    pub(crate) fn get(&self, x: f64, y: f64) -> f32 {
        let sqrt3 = 3f64.sqrt();
        let (f2, g2) = (0.5 * (sqrt3 - 1.0), (3.0 - sqrt3) / 6.0);
        let s = (x + y) * f2;
        let (i, j) = ((x + s).floor() as i32, (y + s).floor() as i32);
        let t = f64::from(i.wrapping_add(j)) * g2;
        let (x0, y0) = (x - (f64::from(i) - t), y - (f64::from(j) - t));
        let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };
        let corners = [(0, 0, x0, y0), (i1, j1, x0 - f64::from(i1) + g2, y0 - f64::from(j1) + g2), (1, 1, x0 - 1.0 + 2.0 * g2, y0 - 1.0 + 2.0 * g2)];
        let permute = |v: i32| i32::from(self.permutation[(v & 0xff) as usize]);
        let sum: f64 = corners
            .iter()
            .map(|&(di, dj, x, y)| {
                let [gx, gy] = GRADIENT[(permute((i & 0xff) + di + permute((j & 0xff) + dj)) % 12) as usize];
                let t = 0.5 - x * x - y * y;
                let squared = t * t;
                if t < 0.0 { 0.0 } else { squared * squared * (gx * x + gy * y) }
            })
            .sum();
        (70.0 * sum) as f32
    }
}
