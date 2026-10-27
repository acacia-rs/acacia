//! fastutil 8.5's `IntOpenHashSet` (and `Int2ObjectOpenHashMap`'s keys), kept only for the order it
//! iterates in: `FontSet.glyphsByWidth` lists codepoints in that order and obfuscated text picks
//! from those lists by index, so the order decides which glyph is drawn.

const PHI: u32 = 0x9E37_79B9;
const LOAD: f32 = 0.75;

fn mix(x: i32) -> i32 {
    let h = (x as u32).wrapping_mul(PHI);
    (h ^ (h >> 16)) as i32
}

/// `HashCommon.arraySize(expected, f)`.
fn array_size(expected: usize) -> usize {
    ((expected as f32 / LOAD) as f64).ceil().max(2.0).min(f64::from(1u32 << 30)) as usize
}

fn power_of_two(n: usize) -> usize {
    n.max(2).next_power_of_two()
}

fn max_fill(n: usize) -> usize {
    ((n as f32 * LOAD) as f64).ceil().min((n - 1) as f64) as usize
}

pub struct IntSet {
    /// `n` slots, then the null-key slot.
    key: Vec<i32>,
    has_zero: bool,
    size: usize,
    max_fill: usize,
}

impl Default for IntSet {
    fn default() -> IntSet {
        IntSet::with_expected(16)
    }
}

impl IntSet {
    /// `new IntOpenHashSet(expected)` / `new Int2ObjectOpenHashMap(expected)`.
    pub fn with_expected(expected: usize) -> IntSet {
        let n = power_of_two(array_size(expected));
        IntSet { key: vec![0; n + 1], has_zero: false, size: 0, max_fill: max_fill(n) }
    }

    fn n(&self) -> usize {
        self.key.len() - 1
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    pub fn add(&mut self, k: i32) -> bool {
        if k == 0 {
            if self.has_zero {
                return false;
            }
            self.has_zero = true;
        } else {
            let mask = self.n() - 1;
            let mut pos = mix(k) as usize & mask;
            while self.key[pos] != 0 {
                if self.key[pos] == k {
                    return false;
                }
                pos = (pos + 1) & mask;
            }
            self.key[pos] = k;
        }
        self.size += 1;
        if self.size - 1 >= self.max_fill {
            self.rehash(power_of_two(array_size(self.size + 1)));
        }
        true
    }

    /// `addAll(IntCollection)`: sized for both first (`tryCapacity`), then each of `other` in its order.
    pub fn add_all(&mut self, other: &IntSet) {
        let needed = power_of_two(array_size(self.size + other.size));
        if needed > self.n() {
            self.rehash(needed);
        }
        for k in other.iter() {
            self.add(k);
        }
    }

    fn rehash(&mut self, new_n: usize) {
        let mask = new_n - 1;
        let mut key = vec![0; new_n + 1];
        let mut i = self.n();
        for _ in 0..self.size - usize::from(self.has_zero) {
            i -= 1;
            while self.key[i] == 0 {
                i -= 1;
            }
            let mut pos = mix(self.key[i]) as usize & mask;
            while key[pos] != 0 {
                pos = (pos + 1) & mask;
            }
            key[pos] = self.key[i];
        }
        self.key = key;
        self.max_fill = max_fill(new_n);
    }

    /// The iterator's (and `forEach`'s) order: the null key, then the slots from the last down.
    pub fn iter(&self) -> impl Iterator<Item = i32> + '_ {
        let zero = self.has_zero.then_some(0);
        zero.into_iter().chain(self.key[..self.n()].iter().rev().copied().filter(|&k| k != 0))
    }
}

/// A `CodepointMap.keySet()`: its codepoints added in ascending order to a fresh set.
pub fn ascending(codepoints: impl IntoIterator<Item = u32>) -> IntSet {
    let mut set = IntSet::default();
    for c in codepoints {
        set.add(c as i32);
    }
    set
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grows_and_iterates_backwards() {
        let mut s = IntSet::default();
        for k in 1..=100 {
            assert!(s.add(k));
        }
        assert!(!s.add(50));
        assert_eq!(s.len(), 100);
        assert_eq!(s.n(), 256);
        let all: Vec<i32> = s.iter().collect();
        assert_eq!(all.len(), 100);
        assert!(all.windows(2).all(|w| w[0] != w[1]));
        s.add(0);
        assert_eq!(s.iter().next(), Some(0));
    }
}
