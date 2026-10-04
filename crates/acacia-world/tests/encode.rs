//! Encoders against the crate's own decoders: whatever a server builds must read back unchanged.

use acacia_proto::xxh64;
use acacia_world::{Chunk, Dimension, Heightmap, SECTION_VOLUME, level_chunk_block_entities, sub_chunk_block_entities};

const AIR: u32 = 17025;
const WATER: u32 = 900;
const DIMENSIONS: [Dimension; 3] = [Dimension::overworld(AIR), Dimension::nether(AIR), Dimension::end(AIR)];
/// Palette sizes on both sides of every bits-per-block step (1, 2, 3, 4, 5, 6, 8, 16).
const DISTINCT: [u32; 17] = [1, 2, 3, 4, 5, 8, 9, 16, 17, 32, 33, 64, 65, 256, 257, 1000, 4096];
const RUNTIME: &dyn Fn(u32) -> u32 = &|id| id;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

fn section_ys(dim: Dimension) -> std::ops::Range<i32> {
    (dim.min_y >> 4)..((dim.min_y + dim.height as i32) >> 4)
}

/// Sections that are absent, uniform or hold 1..=4096 distinct ids, some waterlogged, with biomes
/// that sometimes repeat the section below.
fn random_chunk(rng: &mut Rng, dim: Dimension) -> Chunk {
    let mut chunk = Chunk::empty(3, -7, dim);
    for section_y in section_ys(dim) {
        let distinct = DISTINCT[rng.below(DISTINCT.len() as u32) as usize];
        let base = rng.below(20_000);
        match rng.below(5) {
            0 => {}
            1 => chunk.fill_section(section_y, base),
            kind => {
                let blocks: [u32; SECTION_VOLUME] = match distinct {
                    4096 => std::array::from_fn(|i| base + i as u32),
                    _ => std::array::from_fn(|_| base + rng.below(distinct)),
                };
                let liquid: [u32; SECTION_VOLUME] = std::array::from_fn(|_| if rng.below(7) == 0 { WATER } else { AIR });
                chunk.set_section(section_y, &blocks, (kind == 2).then_some(&liquid));
            }
        }
        if rng.below(3) > 0 {
            for _ in 0..rng.below(40) {
                let (x, y, z) = (rng.below(16) as i32, section_y * 16 + rng.below(16) as i32, rng.below(16) as i32);
                chunk.set_biome(x, y, z, rng.below(60));
            }
        }
    }
    chunk
}

fn assert_same_content(a: &Chunk, b: &Chunk, what: &str) {
    let mut bufs = [[0u32; SECTION_VOLUME]; 4];
    let [ab, al, bb, bl] = &mut bufs;
    for i in 0..a.section_count() {
        // A section that was never stored reads as air, like one stored as air.
        ab.fill(a.dimension().air);
        al.fill(a.dimension().air);
        bb.fill(a.dimension().air);
        bl.fill(a.dimension().air);
        a.copy_section(i, ab, al);
        b.copy_section(i, bb, bl);
        assert!(ab == bb, "{what}: blocks of section {i}");
        assert!(al == bl, "{what}: liquid of section {i}");
        // Sections the builder never gave biomes encode as a copy of the one below.
        if a.copy_biomes(i, ab) {
            assert!(b.copy_biomes(i, bb) && ab == bb, "{what}: biomes of section {i}");
        }
    }
}

#[test]
fn full_chunks_round_trip() {
    let block_entities = [0x0a, 0x00, 0x00];
    for (d, &dim) in DIMENSIONS.iter().enumerate() {
        for seed in 1..=12 {
            let what = format!("dimension {d} seed {seed}");
            let chunk = random_chunk(&mut Rng(seed * 0x9e37_79b9 + d as u64), dim);
            let data = chunk.level_chunk(&block_entities, RUNTIME);
            assert_eq!(data.sub_chunk_count, chunk.sub_chunk_count(), "{what}");
            assert_eq!((data.highest_subchunk_count, data.blobs.len()), (None, 0));

            let decoded = Chunk::decode(3, -7, dim, data.sub_chunk_count, &data.payload).unwrap_or_else(|e| panic!("{what}: {e}"));
            assert_same_content(&chunk, &decoded, &what);
            let offset = level_chunk_block_entities(&data.payload, data.sub_chunk_count, Some(dim)).unwrap();
            assert_eq!(data.payload[offset..], block_entities, "{what}");
            assert_eq!(decoded.level_chunk(&block_entities, RUNTIME), data, "{what}: re-encoding is stable");

            // Hashed ids: the wire ids are large and the decoder maps them back.
            let hashed = chunk.level_chunk(&[], &|id| !id);
            let decoded = Chunk::decode_mapped(3, -7, dim, hashed.sub_chunk_count, &hashed.payload, &|id| !id).unwrap();
            assert_same_content(&chunk, &decoded, &what);
        }
    }
}

#[test]
fn request_mode_carries_biomes_and_the_section_limit() {
    for &dim in &DIMENSIONS {
        let chunk = random_chunk(&mut Rng(77), dim);
        let data = chunk.level_chunk_request();
        assert_eq!((data.sub_chunk_count, data.highest_subchunk_count), (0, Some(chunk.sub_chunk_count() as i32)));
        assert_eq!(level_chunk_block_entities(&data.payload, 0, Some(dim)), Ok(data.payload.len()));

        let heights = chunk.column_heights(&|id| id != AIR);
        let mut rebuilt = Chunk::empty(3, -7, dim);
        rebuilt.set_biomes(&data.payload);
        for section_y in section_ys(dim) {
            let sub = chunk.sub_chunk(section_y, &heights, b"\x0a\x00\x00", RUNTIME).unwrap();
            let Some(payload) = &sub.payload else { continue };
            assert_eq!(sub_chunk_block_entities(payload), Ok(payload.len() - 3));
            // The version-9 y index is the absolute section y.
            assert_eq!(payload[..3], [9, payload[1], section_y as i8 as u8]);
            rebuilt.set_sub_chunk(section_y, payload).unwrap();
        }
        assert_same_content(&chunk, &rebuilt, "request mode");
        let ys = section_ys(dim);
        assert!(chunk.sub_chunk(ys.start - 1, &heights, &[], RUNTIME).is_none());
        assert!(chunk.sub_chunk(ys.end, &heights, &[], RUNTIME).is_none());
    }
}

#[test]
fn cache_mode_splits_the_same_bytes_into_blobs() {
    let dim = DIMENSIONS[0];
    let chunk = random_chunk(&mut Rng(5), dim);
    let (plain, cached) = (chunk.level_chunk(b"be", RUNTIME), chunk.level_chunk_cached(b"be", RUNTIME));
    assert_eq!(cached.blobs.len() as u32, cached.sub_chunk_count + 1);
    assert!(cached.blobs.iter().all(|b| b.hash == xxh64(&b.payload)));
    let joined: Vec<u8> = cached.blobs.iter().flat_map(|b| b.payload.iter().copied()).chain(cached.payload.iter().copied()).collect();
    assert_eq!(joined, plain.payload);
    assert_eq!(cached.payload[..], [0, b'b', b'e']);
    assert!(cached.packet(3, -7, 0).cache_enabled && !plain.packet(3, -7, 0).cache_enabled);

    let request = chunk.level_chunk_request_cached();
    assert_eq!((request.blobs.len(), &request.payload[..]), (1, &[0][..]));
    assert_eq!(request.blobs[0], *cached.blobs.last().unwrap());
    assert_eq!(request.blobs[0].payload, chunk.biome_bytes());

    let heights = chunk.column_heights(&|id| id != AIR);
    for section_y in section_ys(dim) {
        let plain = chunk.sub_chunk(section_y, &heights, b"be", RUNTIME).unwrap();
        let cached = chunk.sub_chunk_cached(section_y, &heights, b"be", RUNTIME).unwrap();
        assert_eq!(plain.payload.is_some(), cached.blob.is_some());
        let (Some(whole), Some(blob), Some(rest)) = (&plain.payload, &cached.blob, &cached.payload) else { continue };
        assert_eq!([&blob.payload[..], &rest[..]].concat(), whole[..]);
        assert_eq!((blob.hash, &rest[..]), (xxh64(&blob.payload), &b"be"[..]));
        assert_eq!(cached.entry(0, section_y as i8, 0).blob_id, Some(blob.hash));
    }
}

#[test]
fn biomes_equal_to_the_section_below_are_a_marker() {
    let mut chunk = Chunk::empty(0, 0, DIMENSIONS[0]);
    assert_eq!(chunk.biome_bytes(), [&[1, 0][..], &[0xff; 23]].concat(), "unset biomes are biome 0");
    chunk.fill_biomes(5);
    assert_eq!(chunk.biome_bytes(), [&[1, 10][..], &[0xff; 23]].concat());
    chunk.set_biome(0, -30, 0, 7);
    let bytes = chunk.biome_bytes();
    // Section 2 differs, section 3 is uniform again and 4.. copy it.
    assert_eq!(bytes[..4], [1, 10, 0xff, 3]);
    assert_eq!(bytes[bytes.len() - 22..], [&[1, 10][..], &[0xff; 20]].concat());
    let mut decoded = Chunk::empty(0, 0, DIMENSIONS[0]);
    decoded.set_biomes(&bytes);
    assert_eq!((decoded.biome(0, -30, 0), decoded.biome(1, -30, 0), decoded.biome(0, 319, 0)), (Some(7), Some(5), Some(5)));
}

#[test]
fn edits_are_compacted_away() {
    let mut chunk = Chunk::empty(0, 0, DIMENSIONS[0]);
    chunk.fill_section(0, 1);
    let uniform = chunk.level_chunk(&[], RUNTIME);
    for i in 0..300 {
        chunk.set(i % 16, i / 16 % 16, 3, 0, 1000 + i as u32);
    }
    chunk.set(1, 1, 1, 1, WATER);
    assert!(chunk.level_chunk(&[], RUNTIME).payload.len() > 8192, "300 ids need 16 bits");
    for i in 0..300 {
        chunk.set(i % 16, i / 16 % 16, 3, 0, 1);
    }
    chunk.set(1, 1, 1, 1, AIR);
    assert_eq!(chunk.level_chunk(&[], RUNTIME), uniform);
    // Four air sections below it, without storages; then one single-value storage of id 1.
    assert_eq!(uniform.sub_chunk_count, 5);
    assert_eq!(uniform.payload[..17], [9, 0, 0xfc, 9, 0, 0xfd, 9, 0, 0xfe, 9, 0, 0xff, 9, 1, 0, 1, 2]);

    chunk.fill_section(0, AIR);
    assert_eq!(chunk.sub_chunk_count(), 0);
    let heights = chunk.column_heights(&|id| id != AIR);
    assert_eq!(chunk.sub_chunk(0, &heights, &[], RUNTIME).unwrap().payload, None, "all air");
}

#[test]
fn heightmaps_are_relative_to_the_sub_chunk() {
    let mut chunk = Chunk::empty(0, 0, DIMENSIONS[0]);
    chunk.fill_section(-4, 1);
    chunk.set(2, 5, 9, 0, 1);
    chunk.set(3, 15, 9, 0, 1);
    chunk.set(4, 40, 9, 0, 1);
    let heights = chunk.column_heights(&|id| id != AIR);
    assert_eq!((heights.get(0, 0), heights.get(2, 9), heights.get(3, 9), heights.get(4, 9)), (-48, 6, 16, 41));

    assert_eq!(heights.sub_chunk(-5), Heightmap::TooHigh);
    assert_eq!(heights.sub_chunk(3), Heightmap::TooLow);
    assert_eq!(heights.sub_chunk(-4), Heightmap::TooHigh, "a full section tops out in the one above");
    let Heightmap::Data(above) = heights.sub_chunk(-3) else { panic!() };
    assert_eq!((above[0], above[(9 << 4) | 2]), (0, 16));
    let Heightmap::Data(ground) = heights.sub_chunk(0) else { panic!() };
    let at = |x: usize, z: usize| ground[(z << 4) | x];
    assert_eq!((at(0, 0), at(2, 9), at(3, 9), at(4, 9)), (-1, 6, 16, 16));

    let entry = chunk.sub_chunk(0, &heights, &[], RUNTIME).unwrap().entry(1, 0, -1);
    let rows = entry.heightmap.unwrap();
    assert_eq!((rows[9][2], rows[9][3], rows[0][0]), (6, 16, -1));
    assert!(rows.iter().all(|r| r.len() == 16));
    assert_eq!(chunk.sub_chunk(3, &heights, &[], RUNTIME).unwrap().entry(0, 3, 0).heightmap, None);

    let empty = Chunk::empty(0, 0, DIMENSIONS[1]).column_heights(&|id| id != AIR);
    assert_eq!((empty.get(0, 0), empty.sub_chunk(0), empty.sub_chunk(1)), (0, Heightmap::Data([0; 256]), Heightmap::TooLow));
}
