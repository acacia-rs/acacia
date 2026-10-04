//! Writing a [`Storage`] in the network format [`Storage::decode`] reads.

use std::borrow::Cow;

use rustc_hash::FxHashMap;

use super::storage::{Storage, VALID_BITS, VOLUME, packed_get};
use super::writer::var_i32;

impl Storage {
    /// The same blocks with unused and duplicate palette entries dropped (`set` only grows the
    /// palette, and `map_ids` can merge ids) at the smallest width that fits. Entries kept stay in
    /// order, so a storage that was already minimal is borrowed and encodes to the bytes it came from.
    pub(super) fn compacted(&self) -> Cow<'_, Storage> {
        let Storage::Packed { bits, palette, words } = self else { return Cow::Borrowed(self) };
        // Malformed indices read as entry 0, see `Storage::get`.
        let entry = |i: usize| Some(packed_get(words, *bits, i)).filter(|&p| p < palette.len());
        let mut used = vec![false; palette.len()];
        let mut malformed = false;
        for i in 0..VOLUME {
            match entry(i) {
                Some(p) => used[p] = true,
                None => (used[0], malformed) = (true, true),
            }
        }
        let mut kept = Vec::new();
        let mut by_id = FxHashMap::default();
        let remap: Vec<usize> = palette
            .iter()
            .zip(&used)
            .map(|(&id, &used)| match used {
                true => *by_id.entry(id).or_insert_with(|| {
                    kept.push(id);
                    kept.len() - 1
                }),
                false => 0,
            })
            .collect();
        let minimal = VALID_BITS.iter().find(|&&b| kept.len() <= 1 << b) == Some(bits);
        if kept.len() == palette.len() && minimal && !malformed {
            return Cow::Borrowed(self);
        }
        Cow::Owned(Storage::from_indices(kept, (0..VOLUME).map(|i| remap[entry(i).unwrap_or(0)])))
    }

    /// Writes the storage as it is; callers compact first. `map` translates each palette id.
    pub(super) fn encode(&self, out: &mut Vec<u8>, map: &dyn Fn(u32) -> u32) {
        match self {
            Storage::Single(id) => {
                out.push(1);
                var_i32(out, map(*id) as i32);
            }
            Storage::Packed { bits, palette, words } => {
                out.push((bits << 1) | 1);
                words.iter().for_each(|w| out.extend_from_slice(&w.to_le_bytes()));
                var_i32(out, palette.len() as i32);
                palette.iter().for_each(|&id| var_i32(out, map(id) as i32));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::reader::Reader;
    use super::*;

    fn encoded(s: &Storage) -> Vec<u8> {
        let mut out = Vec::new();
        s.compacted().encode(&mut out, &|id| id);
        out
    }

    #[test]
    fn compaction_drops_unused_entries_and_shrinks_the_width() {
        let mut s = Storage::Single(7);
        for i in 0..VOLUME {
            s.set(i, (i % 40) as u32 + 100);
        }
        for i in 0..VOLUME {
            s.set(i, (i % 3) as u32 + 100);
        }
        let Storage::Packed { bits: 6, palette, .. } = &s else { panic!("{s:?}") };
        assert_eq!(palette.len(), 41);
        let compact = s.compacted();
        let Storage::Packed { bits: 2, palette, .. } = &*compact else { panic!("{compact:?}") };
        assert_eq!(palette, &[100, 101, 102]);
        assert!((0..VOLUME).all(|i| compact.get(i) == s.get(i)));

        (0..VOLUME).for_each(|i| s.set(i, 5));
        assert_eq!(*s.compacted(), Storage::Single(5));
        assert_eq!(encoded(&s), [1, 10]);
    }

    #[test]
    fn duplicate_ids_merge() {
        let mut s = Storage::from_values(&std::array::from_fn(|i| (i % 4) as u32));
        s.map_ids(&|id| id / 2);
        let compact = s.compacted();
        assert_eq!(compact.palette(), [0, 1]);
        assert!((0..VOLUME).all(|i| compact.get(i) == (i % 4) as u32 / 2));
    }

    #[test]
    fn minimal_storages_encode_to_the_bytes_they_decoded_from() {
        for distinct in [2u32, 3, 5, 9, 17, 33, 65, 257, 4096] {
            let s = Storage::from_values(&std::array::from_fn(|i| (i as u32).wrapping_mul(2654435761) % distinct));
            assert!(matches!(s.compacted(), Cow::Borrowed(_)));
            let bytes = encoded(&s);
            let decoded = Storage::decode(&mut Reader::new(&bytes)).unwrap();
            assert_eq!(decoded, s, "{distinct} ids");
            assert_eq!(encoded(&decoded), bytes);
        }
    }

    #[test]
    fn hashed_ids_round_trip_through_the_zigzag_palette() {
        let s = Storage::from_values(&std::array::from_fn(|i| if i % 2 == 0 { 0xdead_beef } else { 0x7fff_ffff }));
        assert_eq!(Storage::decode(&mut Reader::new(&encoded(&s))).unwrap(), s);
    }
}
