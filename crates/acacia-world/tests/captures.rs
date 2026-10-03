//! Real packets captured from servers; see tests/fixtures/README.md.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use acacia_proto::nbt::Value;
use acacia_proto::packets::{LevelChunk, StartGame};
use acacia_proto::{Packet, RawPacket};
use acacia_world::{BlockAccess, BlockIds, BlockRegistry, ChunkView, CustomBlock, Inserted, World};
use bytes::Bytes;

fn fixture_dir(server: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(server)
}

fn packet<T: Packet>(path: PathBuf) -> T {
    let raw = RawPacket::parse(Bytes::from(std::fs::read(&path).unwrap())).unwrap();
    raw.decode().unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn start_game(server: &str) -> StartGame {
    packet(fixture_dir(server).join("start_game.bin"))
}

fn level_chunks(server: &str) -> Vec<LevelChunk> {
    let mut paths: Vec<_> = std::fs::read_dir(fixture_dir(server))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.file_name().unwrap().to_str().unwrap().starts_with("level_chunk"))
        .collect();
    paths.sort();
    paths.into_iter().map(packet).collect()
}

/// `block_properties[].state.properties[].enum` sizes multiplied, as the client permutes them.
fn custom_blocks(sg: &StartGame) -> Vec<CustomBlock> {
    let enum_len = |p: &Value| match p.get("enum") {
        Some(Value::List(e)) => e.items.len().max(1) as u32,
        _ => 1,
    };
    sg.block_properties
        .iter()
        .map(|b| {
            let state_count = match b.state.value.get("properties") {
                Some(Value::List(l)) => l.items.iter().map(enum_len).product(),
                _ => 1,
            };
            CustomBlock { name: b.name.clone(), state_count }
        })
        .collect()
}

fn geyser_world() -> Arc<World> {
    let sg = start_game("geyser");
    assert!(!sg.block_network_ids_are_hashes);
    let registry = BlockRegistry::vanilla().with_custom_blocks(&custom_blocks(&sg));
    World::new(Arc::new(registry), 0, BlockIds::Runtime)
}

#[test]
fn geyser_start_game_lists_custom_blocks() {
    let vanilla = BlockRegistry::vanilla();
    let custom: Vec<_> = custom_blocks(&start_game("geyser"))
        .into_iter()
        .filter(|b| vanilla.states_of(&b.name).next().is_none())
        .collect();
    assert!(custom.iter().all(|b| b.name.starts_with("geyser_custom:")), "{custom:?}");
    let states: u32 = custom.iter().map(|b| b.state_count).sum();
    assert_eq!((custom.len(), states), (7, 199), "{custom:?}");
    assert_eq!(geyser_world().registry().len(), vanilla.len() + states as usize);
}

/// Geyser sends full chunks (`sub_chunk_count` > 0, cache disabled); the lab world is superflat.
#[test]
fn geyser_chunks_decode_to_plausible_terrain() {
    let world = geyser_world();
    let registry = world.registry().clone();
    let mut view = ChunkView::new(world);
    let chunks = level_chunks("geyser");
    assert_eq!(chunks.iter().filter(|c| c.sub_chunk_count > 0).count(), 9);
    for c in &chunks {
        assert!(!c.cache_enabled);
        view.insert_level_chunk(c.x, c.z, c.sub_chunk_count, &c.payload).unwrap();
        let chunk = view.chunk(c.x, c.z).unwrap().read();
        for id in chunk.palette_ids() {
            assert!(registry.get(id).is_some(), "chunk {},{}: runtime id {id} out of range", c.x, c.z);
        }
    }

    let name = |x, y, z| registry.get(view.block(x, y, z)).unwrap().name;
    let mut floor = HashMap::<&str, usize>::new();
    for c in chunks.iter().filter(|c| c.sub_chunk_count > 0) {
        for dx in 0..16 {
            for dz in 0..16 {
                *floor.entry(name(c.x * 16 + dx, -64, c.z * 16 + dz)).or_default() += 1;
            }
        }
    }
    // Stone is a test arena filled down to bedrock level around spawn.
    let expected = HashMap::from([("minecraft:bedrock", 1545), ("minecraft:stone", 759)]);
    assert_eq!(floor, expected, "y=-64 floor");

    // The server logged the spawn at feet y=-61.0: stone up to -62, then room to stand.
    let (x, z) = (60, 6);
    let column: Vec<_> = (-64..-59).map(|y| name(x, y, z)).collect();
    assert_eq!(column, ["minecraft:stone", "minecraft:stone", "minecraft:stone", "minecraft:air", "minecraft:air"]);
}

#[test]
fn geyser_second_bot_shares_every_chunk() {
    let world = geyser_world();
    let (mut a, mut b) = (ChunkView::new(world.clone()), ChunkView::new(world.clone()));
    let chunks = level_chunks("geyser");
    for c in &chunks {
        a.insert_level_chunk(c.x, c.z, c.sub_chunk_count, &c.payload).unwrap();
    }
    for c in &chunks {
        assert_eq!(b.insert_level_chunk(c.x, c.z, c.sub_chunk_count, &c.payload).unwrap(), Inserted::Shared);
    }
    assert_eq!(world.live_chunks(), chunks.len());
}

/// BDS uses hashed block ids and sub-chunk request mode: LevelChunk carries no sections.
#[test]
fn bds_uses_hashed_ids_and_request_mode() {
    assert!(start_game("bds").block_network_ids_are_hashes);
    let world = World::new(BlockRegistry::vanilla_arc(), 0, BlockIds::Hashed);
    for c in level_chunks("bds") {
        assert_eq!(c.sub_chunk_count, 0);
        assert!(c.highest_subchunk_count.is_some());
        let (chunk, _) = world.insert_level_chunk(c.x, c.z, c.sub_chunk_count, &c.payload).unwrap();
        assert_eq!(chunk.read().palette_ids().count(), 0);
    }
}

/// Real chunks end in whole NBT compounds once sections, biomes and border blocks are skipped.
#[test]
fn captured_chunks_end_in_block_entities() {
    for server in ["geyser", "bds"] {
        for c in level_chunks(server) {
            let dim = acacia_world::Dimension::from_id(c.dimension, 0);
            let offset = acacia_world::level_chunk_block_entities(&c.payload, c.sub_chunk_count, Some(dim)).unwrap();
            let mut tail = &c.payload[offset..];
            let mut count = 0;
            while !tail.is_empty() {
                let nbt = acacia_proto::nbt::read::<acacia_proto::nbt::Network>(&mut tail).unwrap();
                assert!(matches!(nbt.value.get("x"), Some(Value::Int(_))), "{server} {},{}: {nbt:?}", c.x, c.z);
                count += 1;
            }
            eprintln!("{server} chunk {},{}: {count} block entities", c.x, c.z);
        }
    }
}

#[test]
fn air_id_with_geyser_custom_blocks() {
    let custom = custom_blocks(&start_game("geyser"));
    let vanilla = BlockRegistry::vanilla();
    assert_eq!(vanilla.air_id_with_custom_blocks(&custom), vanilla.with_custom_blocks(&custom).air_id());
    let early = CustomBlock { name: "a:b".into(), state_count: 3 };
    let shift = vanilla.with_custom_blocks(std::slice::from_ref(&early)).air_id() - vanilla.air_id();
    assert_eq!(vanilla.air_id_with_custom_blocks(&[early]), vanilla.air_id() + shift);
}
