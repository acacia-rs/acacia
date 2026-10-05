//! Real documents (corpus/, see README.md) and seeded mutations of them.

use acacia_nbt::{Flavor, LittleEndian, Network, read, write};
use bytes::BytesMut;

#[path = "common/corpus.rs"]
mod corpus;
#[path = "common/props.rs"]
mod props;

const MUTANTS_PER_DOCUMENT: usize = 200;

fn byte_exact<F: Flavor>(flavour: &str) {
    let docs = corpus::load(flavour);
    assert!(!docs.is_empty(), "no {flavour} corpus");
    for (name, bytes) in &docs {
        let mut r = &bytes[..];
        let nbt = read::<F>(&mut r).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(r.is_empty(), "{name}: trailing bytes");
        let mut w = BytesMut::new();
        write::<F>(&mut w, &nbt);
        assert_eq!(&w[..], &bytes[..], "{name}: re-encoding is not byte-exact");
        props::decode::<F>(bytes);
    }
}

#[test]
fn network_corpus_is_byte_exact() {
    byte_exact::<Network>("network");
}

#[test]
fn le_corpus_is_byte_exact() {
    byte_exact::<LittleEndian>("le");
}

struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn mutate(doc: &[u8], rng: &mut XorShift) -> Vec<u8> {
    let mut m = doc.to_vec();
    for _ in 0..1 + rng.below(3) {
        if m.is_empty() {
            break;
        }
        let at = rng.below(m.len());
        match rng.below(6) {
            0 => m[at] = rng.next() as u8,
            1 => m[at] ^= 1 << rng.below(8),
            // Bytes that turn a length or count into something huge or negative.
            2 => m[at] = [0x00, 0x7f, 0x80, 0xff][rng.below(4)],
            3 => m.truncate(at),
            4 => m.insert(at, rng.next() as u8),
            _ => drop(m.remove(at)),
        }
    }
    m
}

/// The fuzz targets' `decode` property on a stable toolchain, so CI covers it without nightly.
#[test]
fn mutated_corpus_holds_decode_property() {
    let mut rng = XorShift(0x9e37_79b9_7f4a_7c15);
    for flavour in ["network", "le"] {
        for (name, bytes) in corpus::load(flavour) {
            for _ in 0..MUTANTS_PER_DOCUMENT {
                let mutant = mutate(&bytes, &mut rng);
                let held = std::panic::catch_unwind(|| {
                    props::decode::<Network>(&mutant);
                    props::decode::<LittleEndian>(&mutant);
                });
                assert!(held.is_ok(), "mutant of {flavour}/{name}: {mutant:02x?}");
            }
        }
    }
}
