//! A small seeded random source with the draws Java's `RandomSource` offers particles.

#[derive(Debug, Clone, Default)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    /// 0 to 1, exclusive.
    pub fn f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }

    /// 0 to `n`, exclusive; 0 when `n` is 0.
    pub fn below(&mut self, n: u32) -> u32 {
        ((self.f32() * n as f32) as u32).min(n.saturating_sub(1))
    }

    pub fn bool(&mut self) -> bool {
        self.f32() < 0.5
    }

    pub fn sign(&mut self) -> f32 {
        if self.bool() { 1.0 } else { -1.0 }
    }

    /// Between `low` and `high`.
    pub fn range(&mut self, low: f32, high: f32) -> f32 {
        low + self.f32() * (high - low)
    }
}
