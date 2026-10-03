use acacia_client::proto::nbt::{self, Nbt, Network, Value};
use acacia_client::proto::packets::{
    BlockEntityData, ChangeDimension, LevelChunk, NetworkChunkPublisherUpdate, Subchunk, UpdateBlock, UpdateSubchunkBlocks,
};
use acacia_client::proto::types::{
    BlockCoordinates, BlockUpdate, BlockUpdateTransitionType, HeightMapDataType, SubChunkEntryItem, SubChunkEntryItemResult,
    UpdateBlockFlags, Vec3f, Vec3li,
};
use acacia_client::proto::{Packet, RawPacket};
use bytes::{Bytes, BytesMut};

use super::{BlockEntities, BlockEntityTracking};
use crate::signs::{sign_nbt, SideEdit};
use crate::state::queries::test_support::{fixtures, raw};

const AIR: u32 = 99;

fn sign(pos: [i32; 3], front: &str) -> Nbt {
    sign_nbt(None, pos, false, &SideEdit { front: true, text: front }, 0)
}

fn chest([x, y, z]: [i32; 3]) -> Nbt {
    let entries = vec![("id".into(), Value::String("Chest".into())), ("x".into(), Value::Int(x)), ("y".into(), Value::Int(y)), ("z".into(), Value::Int(z))];
    Nbt { name: String::new(), value: Value::Compound(entries) }
}

fn encoded(nbts: &[Nbt]) -> Vec<u8> {
    let mut w = BytesMut::new();
    nbts.iter().for_each(|n| nbt::write::<Network>(&mut w, n));
    w.to_vec()
}

/// A v9 section of one block (single-value storage).
fn section(y: i8) -> Vec<u8> {
    vec![9, 1, y as u8, 0x01, 0x02]
}

/// One sub-chunk, 24 biome storages (one value, then "same as below"), no border blocks, `nbts`.
fn full_chunk(x: i32, z: i32, nbts: &[Nbt]) -> RawPacket {
    let mut payload = section(-4);
    payload.extend([0x01, 0x00]);
    payload.extend([0xff; 23]);
    payload.push(0);
    payload.extend(encoded(nbts));
    let c = LevelChunk { x, z, dimension: 0, sub_chunk_count: 1, highest_subchunk_count: None, cache_enabled: false, blobs: Vec::new(), payload: payload.into() };
    raw(&c)
}

fn entry(dy: i8, result: SubChunkEntryItemResult, payload: Option<Vec<u8>>, blob_id: Option<u64>) -> SubChunkEntryItem {
    SubChunkEntryItem {
        dx: 0,
        dy,
        dz: 0,
        result,
        payload: payload.map(Bytes::from),
        heightmap_type: HeightMapDataType::NoData,
        heightmap: None,
        render_heightmap_type: HeightMapDataType::NoData,
        render_heightmap: None,
        blob_id,
    }
}

fn sub_chunk(cache_enabled: bool, entries: Vec<SubChunkEntryItem>) -> RawPacket {
    raw(&Subchunk { cache_enabled, dimension: 0, origin: Vec3li { x: 0, y: 0, z: 0 }, entries })
}

fn update(pos: [i32; 3], id: u32) -> RawPacket {
    let position = BlockCoordinates { x: pos[0], y: pos[1], z: pos[2] };
    raw(&UpdateBlock { position, block_runtime_id: id, flags: UpdateBlockFlags::NETWORK, layer: 0 })
}

fn tracker(tracking: BlockEntityTracking) -> BlockEntities {
    BlockEntities { air: Some(AIR), ..BlockEntities::new(tracking) }
}

fn positions(e: &BlockEntities) -> Vec<[i32; 3]> {
    let mut all: Vec<_> = e.iter().map(|(p, _)| *p).collect();
    all.sort();
    all
}

#[test]
fn full_chunks_fill_and_replace_their_column() {
    let mut e = tracker(BlockEntityTracking::All);
    e.apply(&full_chunk(0, 0, &[sign([1, 64, 2], "shop"), chest([3, -60, 4])])).unwrap();
    assert_eq!(e.sign_text([1, 64, 2], true), Some("shop"));
    assert_eq!(e.get([3, -60, 4]), Some(&chest([3, -60, 4])));
    assert!(e.sign([3, -60, 4]).is_none());

    e.apply(&full_chunk(0, 0, &[chest([5, 70, 5])])).unwrap();
    assert_eq!(positions(&e), [[5, 70, 5]], "a resent chunk replaces the column");
}

#[test]
fn sign_tracking_keeps_only_signs() {
    let mut e = tracker(BlockEntityTracking::Signs);
    e.apply(&full_chunk(1, 0, &[sign([17, 64, 2], "a"), chest([18, 64, 2])])).unwrap();
    e.apply(&raw(&BlockEntityData { position: BlockCoordinates { x: 19, y: 64, z: 2 }, nbt: chest([19, 64, 2]) })).unwrap();
    assert_eq!(positions(&e), [[17, 64, 2]]);
}

#[test]
fn sub_chunks_with_and_without_the_blob_cache() {
    let mut e = tracker(BlockEntityTracking::All);
    let mut inline = section(4);
    inline.extend(encoded(&[sign([1, 70, 1], "inline")]));
    let cached = encoded(&[sign([2, 90, 2], "cached")]);
    e.apply(&sub_chunk(false, vec![entry(4, SubChunkEntryItemResult::Success, Some(inline), None)])).unwrap();
    e.apply(&sub_chunk(true, vec![entry(5, SubChunkEntryItemResult::Success, Some(cached), Some(7))])).unwrap();
    assert_eq!(e.sign_text([1, 70, 1], true), Some("inline"));
    assert_eq!(e.sign_text([2, 90, 2], true), Some("cached"));

    e.apply(&sub_chunk(true, vec![entry(4, SubChunkEntryItemResult::SuccessAllAir, None, None)])).unwrap();
    assert_eq!(positions(&e), [[2, 90, 2]], "an all-air section has no block entities");
}

#[test]
fn only_updates_to_air_remove() {
    let mut e = tracker(BlockEntityTracking::All);
    e.apply(&full_chunk(0, 0, &[sign([1, 64, 2], "x"), chest([3, 64, 3]), chest([4, 64, 4])])).unwrap();
    e.apply(&update([1, 64, 2], 5)).unwrap();
    assert!(e.sign([1, 64, 2]).is_some(), "a resent block keeps its block entity");
    e.apply(&update([1, 64, 2], AIR)).unwrap();
    assert!(e.get([1, 64, 2]).is_none());

    let air = |x| BlockUpdate {
        position: BlockCoordinates { x, y: 64, z: x },
        runtime_id: AIR,
        flags: 0,
        entity_unique_id: 0,
        transition_type: BlockUpdateTransitionType::Entity,
    };
    let batch = UpdateSubchunkBlocks { x: 0, y: 4, z: 0, blocks: vec![air(3)], extra: vec![air(4)] };
    e.apply(&raw(&batch)).unwrap();
    assert_eq!(positions(&e), [[4, 64, 4]], "layer-1 air leaves the block entity");
}

#[test]
fn publisher_updates_and_dimension_changes_forget_columns() {
    let mut e = tracker(BlockEntityTracking::All);
    e.apply(&full_chunk(0, 0, &[chest([1, 64, 1])])).unwrap();
    e.apply(&full_chunk(10, 0, &[chest([161, 64, 1])])).unwrap();
    let center = BlockCoordinates { x: 0, y: 64, z: 0 };
    e.apply(&raw(&NetworkChunkPublisherUpdate { coordinates: center, radius: 64, saved_chunks: Vec::new() })).unwrap();
    assert_eq!(positions(&e), [[1, 64, 1]]);

    let change = ChangeDimension { dimension: 1, position: Vec3f { x: 0.0, y: 0.0, z: 0.0 }, respawn: false, loading_screen_id: None };
    e.apply(&raw(&change)).unwrap();
    assert!(e.is_empty());
}

#[test]
fn resolve_and_packets() {
    use BlockEntityTracking::*;
    assert_eq!((WithTerrain.resolve(true), WithTerrain.resolve(false), Signs.resolve(false)), (All, Off, Signs));
    assert!(!Off.packets().contains(&LevelChunk::ID), "idle bots decode no chunks for block entities");
    assert!(Signs.packets().contains(&Subchunk::ID));
}

#[test]
fn malformed_chunk_packets_are_ignored() {
    let mut fuzzed = tracker(BlockEntityTracking::All);
    for packet in fixtures::<LevelChunk>().iter().chain(&fixtures::<Subchunk>()) {
        let _ = fuzzed.apply(packet);
    }
    let mut e = tracker(BlockEntityTracking::All);
    let mut payload = section(0);
    payload.extend([0x0a, 0x00, 0x08]);
    e.apply(&sub_chunk(false, vec![entry(0, SubChunkEntryItemResult::Success, Some(payload), None)])).unwrap();
    assert!(e.is_empty());
}
