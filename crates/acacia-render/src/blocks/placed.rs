//! What a block draws that depends on where it is: one of several weighted alternatives, a shift
//! off the grid. Both follow Java's position hash and random numbers (`Mth.getSeed`,
//! `SingleThreadedRandomSource`), so a world looks as that game draws it.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::{ModelFace, RenderBlock};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Random {
    /// The block is drawn as one of these instead.
    Whole(Weighted<RenderBlock>),
    /// One of each list is drawn with the block's own faces. The lists share a draw, so their
    /// picks are not independent.
    Parts(Box<[Weighted<Arc<[ModelFace]>>]>),
}

/// Alternatives with their weights.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Weighted<T>(pub Box<[(u32, T)]>);

impl<T> Weighted<T> {
    pub fn pick(&self, mut draw: Draw) -> Option<&T> {
        let total: u32 = self.0.iter().map(|(weight, _)| weight).sum();
        let mut at = draw.below(total.try_into().ok().filter(|t| *t > 0)?) as u32;
        self.0.iter().find_map(|(weight, alternative)| {
            let here = at < *weight;
            at = at.wrapping_sub(*weight);
            here.then_some(alternative)
        })
    }

    pub fn map<U>(&self, f: impl Fn(&T) -> U) -> Weighted<U> {
        Weighted(self.0.iter().map(|(weight, alternative)| (*weight, f(alternative))).collect())
    }
}

/// The hash of a block position that seeds its [`Draw`].
pub fn seed([x, y, z]: [i32; 3]) -> i64 {
    let seed = i64::from(x.wrapping_mul(3_129_871)) ^ i64::from(z).wrapping_mul(116_129_781) ^ i64::from(y);
    seed.wrapping_mul(seed).wrapping_mul(42_317_861).wrapping_add(seed.wrapping_mul(11)) >> 16
}

/// Java's 48-bit linear congruential generator.
#[derive(Debug, Clone, Copy)]
pub struct Draw(i64);

const MULTIPLIER: i64 = 0x5_DEEC_E66D;
const MASK: i64 = (1 << 48) - 1;

impl Draw {
    pub fn new(seed: i64) -> Draw {
        Draw((seed ^ MULTIPLIER) & MASK)
    }

    /// Seeded from this one's next 64 bits, as a multipart block seeds each of its parts.
    pub fn again(mut self) -> Draw {
        let (upper, lower) = (self.next(32), self.next(32));
        Draw::new((i64::from(upper) << 32).wrapping_add(i64::from(lower)))
    }

    pub(crate) fn next(&mut self, bits: u32) -> i32 {
        self.0 = self.0.wrapping_mul(MULTIPLIER).wrapping_add(11) & MASK;
        (self.0 >> (48 - bits)) as i32
    }

    /// Uniform in `0..bound`.
    pub(crate) fn below(&mut self, bound: i32) -> i32 {
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

/// A shift off the grid that depends on the column, as plants stand.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Offset {
    /// The furthest sideways, in blocks.
    pub horizontal: f32,
    /// The furthest down; 0 keeps the block on the ground.
    pub vertical: f32,
}

impl Offset {
    /// In blocks, for the column of world position `(x, z)`.
    pub fn at(&self, x: i32, z: i32) -> [f32; 3] {
        let seed = seed([x, 0, z]);
        let sixteenth = |shift: u32| f64::from(((seed >> shift) & 15) as f32 / 15.0);
        let sideways = |shift: u32| ((sixteenth(shift) - 0.5) * 0.5).clamp(f64::from(-self.horizontal), f64::from(self.horizontal)) as f32;
        [sideways(0), ((sixteenth(4) - 1.0) * f64::from(self.vertical)) as f32, sideways(8)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alternatives_are_picked_by_weight() {
        let list = Weighted(Box::new([(1, 'a'), (0, 'b'), (3, 'c')]));
        let picks: Vec<char> = (0..40).map(|x| *list.pick(Draw::new(seed([x, 70, -3]))).unwrap()).collect();
        assert!(picks.contains(&'a') && !picks.contains(&'b'));
        assert!(picks.iter().filter(|p| **p == 'c').count() > 20);
        assert_eq!(Weighted::<char>(Box::new([])).pick(Draw::new(0)), None);
        assert_eq!(Weighted(Box::new([(0, 'a')])).pick(Draw::new(0)), None);
    }

    #[test]
    fn a_shift_stays_within_its_limits() {
        let offset = Offset { horizontal: 0.125, vertical: 0.2 };
        for x in -20..20 {
            let [dx, dy, dz] = offset.at(x, 7 - x);
            assert!(dx.abs() <= 0.125 && dz.abs() <= 0.125 && (-0.2..=0.0).contains(&dy), "{dx} {dy} {dz}");
        }
        assert_eq!(Offset { horizontal: 0.25, vertical: 0.0 }.at(3, 4)[1], 0.0);
    }
}
