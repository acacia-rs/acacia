use super::batch::*;
use super::blocks::*;
use super::chunk_packet::*;
use super::heightmap::*;
use super::light::*;
use super::sections::*;
use crate::actions::BlockPos;
use crate::chunk::{Section, Values, read_sections};
use crate::server::round_trip;
use crate::tag::Tag;
use crate::types::{ChunkPos, HeightmapKind, SectionBlockPos, SectionPos};
use crate::{VarInt, Wire};

const WIDTHS: Widths = Widths { block_bits: 16, biome_bits: 7 };

fn each(f: impl Fn(usize) -> u32, n: usize) -> Values {
    Values::Each((0..n).map(f).collect::<Vec<_>>().into())
}

fn section(blocks: Values, biomes: Values) -> Section {
    let mut s = Section { non_empty: 0, fluids: 0, blocks, biomes };
    s.recount(|b| b == 0, |b| b == 9);
    s
}

fn write(sections: &[Section]) -> Vec<u8> {
    let mut out = Vec::new();
    write_sections(sections, WIDTHS, &mut out);
    out
}

/// One section per palette kind the blocks and biomes can take.
fn mixed_sections() -> Vec<Section> {
    vec![
        section(Values::Single(0), Values::Single(3)),
        section(each(|i| if i % 7 == 0 { 1 } else { 0 }, 4096), each(|i| (i % 3) as u32, 64)),
        section(each(|i| (i % 20) as u32, 4096), each(|i| (i % 8) as u32, 64)),
        section(each(|i| (i % 300) as u32 + 1, 4096), each(|i| (i % 40) as u32, 64)),
        section(each(|i| if i < 2048 { 9 } else { 12 }, 4096), Values::Single(64)),
    ]
}

#[test]
fn sections_of_every_palette_kind_round_trip() {
    let sections = mixed_sections();
    assert_eq!(read_sections(&write(&sections)).unwrap(), sections);
}

#[test]
fn palette_bit_widths_follow_the_strategy() {
    // After the two counts: the block bits byte. Single, 4-bit linear (never 1..3), 5-bit linear, global, 4-bit.
    let block_bits: Vec<u8> = mixed_sections().iter().map(|s| write(std::slice::from_ref(s))[4]).collect();
    assert_eq!(block_bits, [0, 4, 5, 16, 4]);
    // With a single-value block container ([0, id]) the biome bits byte is at 6.
    let biome_bits: Vec<u8> = [2usize, 3, 8, 40]
        .iter()
        .map(|&n| write(&[section(Values::Single(0), each(|i| (i % n) as u32, 64))])[6])
        .collect();
    assert_eq!(biome_bits, [1, 2, 3, 7], "1..=3 linear, then the global width");
}

#[test]
fn two_entry_containers_have_the_hand_computed_bytes() {
    let mut blocks = vec![5u32; 4096];
    blocks[1] = 6;
    let mut biomes = vec![3u32; 64];
    biomes[1] = 5;
    let bytes = write(&[section(each(|i| blocks[i], 4096), each(|i| biomes[i], 64))]);
    // Blocks: always 4-bit linear, 16 entries a long: entry 1 is index 1 << 4.
    assert_eq!(&bytes[4..8], [4, 2, 5, 6]);
    assert_eq!(&bytes[8..16], 0x10u64.to_be_bytes());
    assert!(bytes[16..8 + 256 * 8].iter().all(|&b| b == 0));
    // Biomes: one bit, 64 entries in one long.
    assert_eq!(bytes[8 + 256 * 8..], [1, 2, 3, 5, 0, 0, 0, 0, 0, 0, 0, 2]);
}

#[test]
fn counts_come_from_the_contents() {
    let s = &mixed_sections()[4];
    assert_eq!((s.non_empty, s.fluids), (4096, 2048));
    assert_eq!(&write(std::slice::from_ref(s))[..4], [0x10, 0x00, 0x08, 0x00]);
    assert_eq!(mixed_sections()[0].non_empty, 0);
}

#[test]
fn a_uniform_each_writes_as_a_single_value() {
    let s = section(each(|_| 7, 4096), each(|_| 1, 64));
    let back = &read_sections(&write(&[s])).unwrap()[0];
    assert_eq!((&back.blocks, &back.biomes), (&Values::Single(7), &Values::Single(1)));
}

#[test]
fn heightmaps_pack_nine_bits_seven_to_a_long() {
    // A 384-high world: ceillog2(385) = 9 bits.
    assert_eq!(bits_for(384), 9);
    let longs = pack_heights(&[1, 2, 3, 4, 5, 6, 7, 8], 9);
    assert_eq!(longs[0], 1 | 2 << 9 | 3 << 18 | 4 << 27 | 5 << 36 | 6 << 45 | 7 << 54);
    assert_eq!(longs[1], 8);
    let heights: Vec<u16> = (0..256).map(|i| (i * 3 % 385) as u16).collect();
    let map = Heightmap::from_heights(HeightmapKind::MotionBlocking, &heights, 384);
    assert_eq!(map.data.len(), 37);
    assert_eq!(map.heights(384).unwrap(), heights);
    assert!(map.heights(200).is_err(), "a different width is a different longs count");
    assert_eq!(round_trip(map).len(), 1 + 1 + 37 * 8);
}

fn chunk() -> LevelChunkWithLight {
    let heights = |h: u16| vec![h; 256];
    let heightmaps = CLIENT_KINDS.iter().map(|&k| Heightmap::from_heights(k, &heights(70), 384)).collect();
    let sign = Tag::Compound(vec![("is_waxed".into(), Tag::Byte(0)), ("front_text".into(), Tag::Compound(vec![("messages".into(), Tag::List(vec![Tag::String("hi".into())]))]))]);
    let block_entities = vec![BlockEntity::new(-3, -60, 17, 7, Some(sign)), BlockEntity::new(0, 70, 0, 12, None)];
    let mut light = LightData::default();
    light.add_sky(0, &[0; LIGHT_BYTES]);
    light.add_sky(1, &[0xAB; LIGHT_BYTES]);
    light.add_block(1, &[0x11; LIGHT_BYTES]);
    light.add_block(2, &[0; LIGHT_BYTES]);
    LevelChunkWithLight {
        x: -12,
        z: 40,
        chunk: ChunkData::with_sections(heightmaps, &mixed_sections(), WIDTHS, block_entities),
        light,
    }
}

#[test]
fn a_full_chunk_with_block_entities_and_light_round_trips() {
    let packet = chunk();
    round_trip(packet.clone());
    assert_eq!(packet.chunk.sections().unwrap(), mixed_sections());
    assert_eq!(packet.chunk.heightmaps.iter().map(|h| h.kind as i32).collect::<Vec<_>>(), [1, 4, 5]);
    assert_eq!(packet.light.sky_mask.bits(), [1]);
    assert_eq!(packet.light.empty_sky_mask.bits(), [0]);
    assert_eq!(packet.light.block_mask.bits(), [1]);
    assert_eq!(packet.light.empty_block_mask.bits(), [2]);
}

#[test]
fn block_entity_bytes_pack_xz_then_a_short_y() {
    let bytes = round_trip(BlockEntity::new(-3, -60, 17, 7, None));
    // x = -3 & 15 = 13, z = 17 & 15 = 1, y = -60 = 0xFFC4, type 7, no tag.
    assert_eq!(bytes, [0xD1, 0xFF, 0xC4, 7, 0]);
}

#[test]
fn an_empty_chunk_matches_the_hand_written_layout() {
    let bytes = round_trip(LevelChunkWithLight { x: -1, z: 2, ..Default::default() });
    // int x, int z, no heightmaps, empty buffer, no block entities, four empty bit sets, two empty arrays lists.
    assert_eq!(bytes, [0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn light_arrays_must_be_2048_bytes() {
    let mut bytes = Vec::new();
    LightData { sky_mask: BitSet::from_bits([0]), sky: vec![vec![1; 10]], ..Default::default() }.write(&mut bytes);
    assert!(LightData::read(&mut &bytes[..]).is_err());
}

#[test]
fn bit_sets_drop_trailing_zero_bytes_like_to_byte_array() {
    let mut out = Vec::new();
    BitSet(vec![0b101, 0, 0]).write(&mut out);
    assert_eq!(out, [1, 0b101]);
    let mut out = Vec::new();
    BitSet::from_bits([9]).write(&mut out);
    assert_eq!(out, [2, 0, 2]);
    assert_eq!(BitSet::from_bits([0, 9, 63]).bits(), [0, 9, 63]);
}

#[test]
fn light_update_uses_var_int_coordinates() {
    let bytes = round_trip(LightUpdate { x: VarInt(-1), z: VarInt(300), light: LightData::default() });
    assert_eq!(bytes[..5], [0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
    assert_eq!(bytes[5..7], [0xAC, 0x02]);
}

#[test]
fn forget_level_chunk_packs_like_vanilla() {
    // ChunkPos.pack: z in the high int, x in the low one, whatever x's sign (azalea sign-extends and loses z).
    let bytes = round_trip(ForgetLevelChunk { pos: ChunkPos { x: -1, z: 5 } });
    assert_eq!(bytes, [0, 0, 0, 5, 0xFF, 0xFF, 0xFF, 0xFF]);
    round_trip(ForgetLevelChunk { pos: ChunkPos { x: -300, z: -9 } });
}

#[test]
fn block_packets_round_trip_with_the_vanilla_widths() {
    let pos = BlockPos([-5, 64, 9]);
    round_trip(BlockUpdate { pos, state: VarInt(35000) });
    round_trip(BlockChangedAck { sequence: VarInt(12) });
    let bytes = round_trip(BlockEvent { pos, b0: 1, b1: 2, block: VarInt(300) });
    assert_eq!(bytes[8..], [1, 2, 0xAC, 0x02]);
    let bytes = round_trip(BlockDestruction { id: VarInt(77), pos, progress: 9 });
    assert_eq!((bytes[0], bytes[9]), (77, 9), "progress is one byte");
}

#[test]
fn block_entity_data_carries_a_nameless_compound() {
    let tag = Tag::Compound(vec![("x".into(), Tag::Int(1))]);
    let bytes = round_trip(BlockEntityData { pos: BlockPos([1, 2, 3]), kind: VarInt(8), tag });
    assert_eq!(bytes[8..], [8, 10, 3, 0, 1, b'x', 0, 0, 0, 1, 0]);
    assert!(BlockEntityData::read(&mut &[0u8; 10][..]).is_err(), "TAG_End is no compound");
}

#[test]
fn section_blocks_update_packs_state_and_position() {
    let packet = SectionBlocksUpdate {
        section: SectionPos { x: -2, y: -4, z: 7 },
        changes: vec![
            BlockChange { pos: SectionBlockPos { x: 15, y: 1, z: 2 }, state: 35000 },
            BlockChange { pos: SectionBlockPos::of(BlockPos([-1, 63, 17])), state: 0 },
        ],
    };
    let bytes = round_trip(packet);
    assert_eq!(bytes[8], 2);
    // state << 12 | x << 8 | z << 4 | y as a VarLong.
    let mut input = &bytes[9..];
    assert_eq!(crate::read_varlong(&mut input).unwrap(), 35000 << 12 | 15 << 8 | 2 << 4 | 1);
}

#[test]
fn batch_and_cache_packets() {
    assert_eq!(round_trip(ChunkBatchFinished { batch_size: VarInt(25) }), [25]);
    assert_eq!(round_trip(ChunkBatchStart {}), []);
    assert_eq!(round_trip(SetChunkCacheCenter { x: VarInt(-3), z: VarInt(4) }), [0xFD, 0xFF, 0xFF, 0xFF, 0x0F, 4]);
    assert_eq!(round_trip(SetChunkCacheRadius { radius: VarInt(10) }), [10]);
    assert_eq!(round_trip(ChunkBatchReceived { chunks_per_tick: 7.0 }), 7f32.to_be_bytes());
}
