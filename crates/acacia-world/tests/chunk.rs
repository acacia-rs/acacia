mod common;

use acacia_world::{Chunk, Dimension, Error};
use common::*;

const AIR: u32 = 17025;
const OW: Dimension = Dimension::overworld(AIR);

fn pattern(seed: u32, distinct: u32) -> Vec<u32> {
    (0..VOLUME as u32).map(|i| (i.wrapping_mul(2654435761) ^ seed) % distinct + 1).collect()
}

#[test]
fn decodes_v9_sections_by_y_index() {
    let stone = filled(1);
    let mut blocks = pattern(7, 9);
    blocks[idx(3, 5, 7)] = 42;
    let water = {
        let mut w = filled(AIR);
        w[idx(3, 5, 7)] = 99;
        w
    };
    let mut payload = section_v9(-4, &[&stone]);
    payload.extend(section_v9(2, &[&blocks, &water]));
    payload.extend([0xde, 0xad]); // biomes etc. are ignored

    let c = Chunk::decode(1, -2, OW, 2, &payload).unwrap();
    assert_eq!(c.block(0, -64, 0), 1);
    assert_eq!(c.block(15, -49, 15), 1);
    assert_eq!(c.block(0, -48, 0), AIR, "unsent section reads as air");
    assert_eq!(c.block(16 + 3, 32 + 5, -32 + 7), 42, "world coords map modulo 16");
    assert_eq!(c.liquid(3, 37, 7), 99);
    assert_eq!(c.liquid(4, 37, 7), AIR);
    for x in 0..16 {
        for z in 0..16 {
            for y in 0..16 {
                let i = idx(x, y, z);
                let want = if i == idx(3, 5, 7) { 42 } else { blocks[i] };
                assert_eq!(c.block(x as i32, 32 + y as i32, z as i32), want);
            }
        }
    }
    assert_eq!(c.block(0, -65, 0), AIR);
    assert_eq!(c.block(0, 320, 0), AIR);
}

#[test]
fn every_bit_width_round_trips() {
    for (bits, distinct) in [(0, 1), (1, 2), (2, 4), (3, 8), (4, 16), (5, 32), (6, 64), (8, 256), (16, 4096)] {
        let values = if distinct == 4096 { (0..VOLUME as u32).collect() } else { pattern(bits as u32, distinct) };
        let mut payload = vec![9, 1, 0];
        storage(&mut payload, &values, Some(bits));
        let c = Chunk::decode(0, 0, OW, 1, &payload).unwrap_or_else(|e| panic!("bits {bits}: {e}"));
        for (i, v) in values.iter().enumerate() {
            let (x, z, y) = ((i >> 8) as i32, ((i >> 4) & 15) as i32, (i & 15) as i32);
            assert_eq!(c.block(x, y, z), *v, "bits {bits} index {i}");
        }
    }
}

#[test]
fn v8_and_v1_sections_are_positional() {
    let mut payload = vec![8, 1];
    storage(&mut payload, &filled(5), None);
    payload.push(1);
    storage(&mut payload, &filled(6), None);
    let c = Chunk::decode(0, 0, Dimension::nether(AIR), 2, &payload).unwrap();
    assert_eq!(c.block(0, 0, 0), 5);
    assert_eq!(c.block(0, 16, 0), 6);
}

#[test]
fn updates_grow_the_palette() {
    let mut c = Chunk::decode(0, 0, OW, 1, &section_v9(4, &[&filled(1)])).unwrap();
    for i in 0..300u32 {
        c.set(i as i32 % 16, 64 + (i as i32 / 16) % 16, i as i32 / 256, 0, 1000 + i);
    }
    for i in 0..300u32 {
        assert_eq!(c.block(i as i32 % 16, 64 + (i as i32 / 16) % 16, i as i32 / 256), 1000 + i);
    }
    c.set(0, 0, 0, 1, 77);
    assert_eq!(c.liquid(0, 0, 0), 77);
    assert_eq!(c.block(0, 0, 0), AIR, "updating an unsent section creates it filled with air");
    c.set(0, 1000, 0, 0, 1);
}

#[test]
fn sub_chunk_entry_decodes_into_place() {
    let mut c = Chunk::empty(0, 0, OW);
    c.set_sub_chunk(3, &section_v9(3, &[&filled(8)])).unwrap();
    assert_eq!(c.block(0, 48, 0), 8);
    assert_eq!(c.block(0, 47, 0), AIR);
}

#[test]
fn malformed_payloads_error() {
    let full = section_v9(0, &[&pattern(1, 5)]);
    assert_eq!(Chunk::decode(0, 0, OW, 1, &full[..full.len() - 1]), Err(Error::UnexpectedEof));
    assert_eq!(Chunk::decode(0, 0, OW, 1, &[7, 1, 0]), Err(Error::SectionVersion(7)));
    assert_eq!(Chunk::decode(0, 0, OW, 1, &[9, 1, 0, 2 << 1]), Err(Error::PersistentPalette));
    assert_eq!(Chunk::decode(0, 0, OW, 1, &[9, 1, 0, (7 << 1) | 1]), Err(Error::BitsPerBlock(7)));
}
