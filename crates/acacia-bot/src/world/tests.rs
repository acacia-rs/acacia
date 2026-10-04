use super::*;
use crate::state::queries::test_support::raw;
use acacia_client::MemoryBlobStore;
use acacia_client::proto::types::Blob;
use acacia_world::BlockAccess;

fn tracker() -> WorldTracker {
    let mut t = WorldTracker::new("test".into(), SharedWorlds::new());
    t.registry = Some(BlockRegistry::vanilla_arc());
    t.enter_dimension(0);
    t
}

/// v8 section with one single-id storage (zigzag varint id).
fn uniform_section(id: u32) -> Bytes {
    let mut out = vec![8, 1, 1];
    let mut v = id << 1;
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
    out.into()
}

#[test]
fn cached_full_chunk_fills_sections_from_held_and_delivered_blobs() {
    let mut t = tracker();
    let registry = t.registry.clone().unwrap();
    let stone = registry.find("minecraft:stone", "").unwrap();
    let dirt = registry.find("minecraft:dirt", "").unwrap();
    let store = Arc::new(MemoryBlobStore::with_payloads());
    store.insert(1, &uniform_section(stone));
    t.set_blob_store(store);

    let chunk = LevelChunk {
        x: 2,
        z: -1,
        dimension: 0,
        sub_chunk_count: 2,
        highest_subchunk_count: None,
        cache_enabled: true,
        blobs: vec![1, 2, 3],
        payload: Bytes::from_static(&[0]),
    };
    t.apply(&raw(&chunk)).unwrap();
    let view = t.view().unwrap();
    assert_eq!(view.block(32, -64, -16), stone, "held blob: bottom section at once");
    assert_eq!(view.block(32, -48, -16), registry.air_id(), "missing blob: section waits");

    t.apply(&raw(&ClientCacheMissResponse { blobs: vec![Blob { hash: 2, payload: uniform_section(dirt) }] })).unwrap();
    assert_eq!(t.view().unwrap().block(32, -48, -16), dirt, "delivered blob fills the second section");
}
