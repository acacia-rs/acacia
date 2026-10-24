use super::*;
use crate::encode::{EncodeScratch, encode_chunk};
use crate::light::SectionLight;
use crate::mapping::{AIR, java};
use crate::props::PropsTable;

#[test]
fn reencoding_a_decoded_chunk_gives_the_same_bytes() {
    let j = java();
    let stone = j.state("minecraft:stone", &[]).unwrap();
    let mut full = [AIR; VOLUME];
    for (i, s) in full.iter_mut().enumerate() {
        *s = match i % 7 {
            0 => stone,
            1 => j
                .state("minecraft:oak_stairs", &[("facing", "east"), ("half", "top")])
                .unwrap(),
            2 => j.state("minecraft:water", &[("level", "3")]).unwrap(),
            _ => AIR,
        };
    }
    let wide: [JavaState; VOLUME] = std::array::from_fn(|i| (i * 7 % 3000) as JavaState);
    let mut biomes = vec![[3u16; 64]; 24];
    biomes[5] = std::array::from_fn(|i| (i % 5) as u16);
    let col = JavaColumn {
        x: -7,
        z: 40,
        min_section: -4,
        sections: (0..24)
            .map(|s| match s {
                0..4 => Blocks::Uniform(stone),
                4 => Blocks::from_full(&full),
                5 => Blocks::from_full(&wide),
                _ => Blocks::Uniform(AIR),
            })
            .collect(),
        biomes,
    };
    let light: Vec<SectionLight> = (0..24)
        .map(|s| SectionLight {
            sky: Box::new([s as u8 * 11; 2048]),
            block: Box::new(std::array::from_fn(|i| i as u8)),
        })
        .collect();
    let props = PropsTable::new();
    let mut scratch = EncodeScratch::default();
    let (mut a, mut b) = (Vec::new(), Vec::new());
    encode_chunk(&col, &light, &[], &props, &mut scratch, &mut a);
    let back = decode_chunk(&a).unwrap();
    assert_eq!((back.x, back.z, back.min_section), (-7, 40, -4));
    encode_chunk(&back, &light, &[], &props, &mut scratch, &mut b);
    assert!(a == b, "re-encoded NBT differs");
}

#[test]
fn truncated_input_is_an_error() {
    assert!(decode_chunk(&[10, 0, 0, 3, 0, 4, b'x']).is_err());
}
