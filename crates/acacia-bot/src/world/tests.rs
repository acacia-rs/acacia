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

/// A column a server would build: stone floor, a dirt block with water in it higher up, one biome.
fn built_chunk(registry: &BlockRegistry, x: i32, z: i32) -> (acacia_world::Chunk, [u32; 3]) {
    let ids = ["minecraft:stone", "minecraft:dirt", "minecraft:water"].map(|name| registry.states_of(name).next().unwrap().0);
    let mut chunk = acacia_world::Chunk::empty(x, z, Dimension::overworld(registry.air_id()));
    chunk.fill_section(-4, ids[0]);
    chunk.set(5, 70, 3, 0, ids[1]);
    chunk.set(5, 70, 3, 1, ids[2]);
    chunk.fill_biomes(1);
    (chunk, ids)
}

fn assert_built(view: &ChunkView, registry: &BlockRegistry, (x, z): (i32, i32), [stone, dirt, water]: [u32; 3]) {
    let (bx, bz) = (x * 16 + 5, z * 16 + 3);
    assert_eq!((view.block(bx, -64, bz), view.block(bx, -49, bz), view.block(bx, -48, bz)), (stone, stone, registry.air_id()));
    assert_eq!((view.block(bx, 70, bz), view.liquid(bx, 70, bz), view.liquid(bx, 71, bz)), (dirt, water, registry.air_id()));
    assert_eq!(view.chunk(x, z).unwrap().read().biome(bx, 70, bz), Some(1));
}

#[test]
fn encoded_full_chunk_reaches_the_view() {
    let mut t = tracker();
    let registry = t.registry.clone().unwrap();
    let (chunk, ids) = built_chunk(&registry, 2, -1);
    t.apply(&raw(&chunk.level_chunk(&[], &|id| id).packet(2, -1, 0))).unwrap();
    assert_built(t.view().unwrap(), &registry, (2, -1), ids);
}

/// Request mode with hashed ids, as BDS does it: the tracker asks for the advertised sections.
#[test]
fn encoded_sub_chunks_answer_the_trackers_request() {
    for cached in [false, true] {
        let registry = BlockRegistry::vanilla_arc();
        let mut t = WorldTracker::new("test".into(), SharedWorlds::new());
        (t.registry, t.ids) = (Some(registry.clone()), BlockIds::Hashed);
        t.enter_dimension(0);
        let (chunk, ids) = built_chunk(&registry, 4, 7);
        let hashed = |id| registry.network_hash(id);

        let column = if cached { chunk.level_chunk_request_cached() } else { chunk.level_chunk_request() };
        t.apply(&raw(&column.packet(4, 7, 0))).unwrap();
        let request = t.outgoing.pop().unwrap();
        assert_eq!(request.requests.iter().map(|o| o.y).collect::<Vec<_>>(), (-4..5).collect::<Vec<_>>(), "up to the dirt's section");

        let heights = chunk.column_heights(&|id| id != registry.air_id());
        let sub_chunk = |y: i8| match cached {
            true => chunk.sub_chunk_cached(y.into(), &heights, &[], &hashed).unwrap(),
            false => chunk.sub_chunk(y.into(), &heights, &[], &hashed).unwrap(),
        };
        let answers: Vec<_> = request.requests.iter().map(|o| (o, sub_chunk(o.y))).collect();
        let entries = answers.iter().map(|(o, s)| s.entry(o.x, o.y, o.z)).collect();
        t.apply(&raw(&Subchunk { cache_enabled: cached, dimension: 0, origin: request.origin, entries })).unwrap();
        if cached {
            assert_eq!(t.view().unwrap().block(69, -64, 115), registry.air_id(), "sections wait for their blobs");
            let blobs = answers.into_iter().filter_map(|(_, s)| s.blob).chain(column.blobs).collect();
            t.apply(&raw(&ClientCacheMissResponse { blobs })).unwrap();
        }
        assert_built(t.view().unwrap(), &registry, (4, 7), ids);
    }
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
