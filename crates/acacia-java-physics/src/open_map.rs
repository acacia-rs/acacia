//! fastutil's `Object2ObjectOpenHashMap` keyed by an `Identifier`, kept only for its iteration order: an attribute
//! sums and multiplies its modifiers in that order, and float rounding makes the order visible.

const INITIAL_SLOTS: usize = 32;

/// `Object2ObjectOpenHashMap()`: 32 slots (16 expected at load 0.75); four modifiers never rehash or shrink it.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenMap<V> {
    slots: Vec<Option<(String, V)>>,
}

impl<V> Default for OpenMap<V> {
    fn default() -> Self {
        Self { slots: (0..INITIAL_SLOTS).map(|_| None).collect() }
    }
}

/// `String.hashCode` over UTF-16 units.
fn java_hash(s: &str) -> i32 {
    s.encode_utf16().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(i32::from(c)))
}

/// `Identifier.hashCode` of `namespace:path` (a bare path is `minecraft`'s), through `HashCommon.mix`.
fn slot_hash(id: &str) -> u32 {
    let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
    let h = java_hash(namespace).wrapping_mul(31).wrapping_add(java_hash(path)) as u32;
    let h = h.wrapping_mul(0x9E37_79B9);
    h ^ (h >> 16)
}

impl<V> OpenMap<V> {
    fn mask(&self) -> usize {
        self.slots.len() - 1
    }

    fn home(&self, id: &str) -> usize {
        slot_hash(id) as usize & self.mask()
    }

    fn find(&self, id: &str) -> Option<usize> {
        let mut pos = self.home(id);
        while let Some((key, _)) = &self.slots[pos] {
            if key == id {
                return Some(pos);
            }
            pos = (pos + 1) & self.mask();
        }
        None
    }

    /// `put`: a present key keeps its slot.
    pub fn insert(&mut self, id: &str, value: V) {
        if let Some(pos) = self.find(id) {
            self.slots[pos] = Some((id.to_owned(), value));
            return;
        }
        let mut pos = self.home(id);
        while self.slots[pos].is_some() {
            pos = (pos + 1) & self.mask();
        }
        self.slots[pos] = Some((id.to_owned(), value));
    }

    /// `remove` with `shiftKeys`, which moves later keys of the probe run back into the gap.
    pub fn remove(&mut self, id: &str) {
        let Some(mut pos) = self.find(id) else { return };
        loop {
            let last = pos;
            pos = (last + 1) & self.mask();
            loop {
                let Some((key, _)) = &self.slots[pos] else {
                    self.slots[last] = None;
                    return;
                };
                let slot = self.home(key);
                let stays = if last <= pos { last >= slot || slot > pos } else { last >= slot && slot > pos };
                if stays {
                    break;
                }
                pos = (pos + 1) & self.mask();
            }
            self.slots[last] = self.slots[pos].take();
        }
    }

    pub fn clear(&mut self) {
        self.slots.iter_mut().for_each(|s| *s = None);
    }

    /// The map iterator's order: slots from the top down.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.slots.iter().rev().filter_map(|s| s.as_ref().map(|(_, v)| v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_string_hash() {
        assert_eq!(java_hash("minecraft"), 695_073_197);
        assert_eq!(java_hash("effect.speed"), -1_444_448_854);
    }

    #[test]
    fn removal_keeps_probe_runs_reachable() {
        let mut map = OpenMap::default();
        let ids: Vec<String> = (0..20).map(|i| format!("m{i}")).collect();
        for (i, id) in ids.iter().enumerate() {
            map.insert(id, i);
        }
        for id in ids.iter().step_by(2) {
            map.remove(id);
        }
        let mut left: Vec<usize> = map.values().copied().collect();
        left.sort_unstable();
        assert_eq!(left, (1..20).step_by(2).collect::<Vec<_>>());
        assert!(ids.iter().skip(1).step_by(2).all(|id| map.find(id).is_some()));
    }
}
