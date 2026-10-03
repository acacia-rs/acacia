mod common;

use std::sync::Arc;

use acacia_world::{BlockAccess, BlockIds, BlockRegistry, ChunkChange, ChunkView, Inserted, SECTION_VOLUME, World};
use common::*;

fn world() -> Arc<World> {
    World::new(BlockRegistry::vanilla_arc(), 0, BlockIds::Runtime)
}

#[test]
fn hashed_ids_are_translated() {
    let r = BlockRegistry::vanilla();
    let stone = r.find("minecraft:stone", "").unwrap();
    let dirt = r.find("minecraft:dirt", "").unwrap();
    let hash = |id| r.get(id).unwrap().network_hash;
    let w = World::new(BlockRegistry::vanilla_arc(), 0, BlockIds::Hashed);
    let mut v = ChunkView::new(w);
    v.insert_level_chunk(0, 0, 1, &section_v9(0, &[&filled(hash(stone))])).unwrap();
    assert_eq!(v.block(1, 1, 1), stone);
    v.set_block(1, 1, 1, 0, hash(dirt));
    assert_eq!(v.block(1, 1, 1), dirt);
    v.set_block(1, 2, 1, 0, 12345);
    assert_eq!(v.block(1, 2, 1), r.air_id(), "unknown hash");
    v.insert_sub_chunk(0, 1, 0, &section_v9(1, &[&filled(hash(dirt))])).unwrap();
    assert_eq!(v.block(0, 16, 0), dirt);
    assert_eq!(v.world().wire_id(dirt), hash(dirt));
    assert_eq!(world().wire_id(dirt), dirt);
}

fn chunk_payload(id: u32) -> Vec<u8> {
    section_v9(4, &[&filled(id)])
}

#[test]
fn identical_payload_is_shared_not_decoded() {
    let w = world();
    let (mut a, mut b) = (ChunkView::new(w.clone()), ChunkView::new(w.clone()));
    let p = chunk_payload(1);
    assert_eq!(a.insert_level_chunk(2, 3, 1, &p).unwrap(), Inserted::Decoded);
    assert_eq!(b.insert_level_chunk(2, 3, 1, &p).unwrap(), Inserted::Shared);
    assert!(Arc::ptr_eq(a.chunk(2, 3).unwrap(), b.chunk(2, 3).unwrap()));
    assert_eq!(w.live_chunks(), 1);
    assert_eq!(b.block(32, 64, 48), 1);
    drop(a);
    assert_eq!(w.live_chunks(), 1);
    b.retain_within(10, 10, 2);
    assert!(b.is_empty());
    assert_eq!(w.live_chunks(), 0);
    assert!(w.get(2, 3).is_none());
}

#[test]
fn the_owners_new_payload_replaces_in_place_for_every_view() {
    let w = world();
    let (mut a, mut b) = (ChunkView::new(w.clone()), ChunkView::new(w.clone()));
    a.insert_level_chunk(0, 0, 1, &chunk_payload(1)).unwrap();
    b.insert_level_chunk(0, 0, 1, &chunk_payload(1)).unwrap();
    assert_eq!(a.insert_level_chunk(0, 0, 1, &chunk_payload(2)).unwrap(), Inserted::Decoded);
    assert_eq!(b.block(0, 64, 0), 2);
}

#[test]
fn block_updates_are_shared_and_invalidate_the_hash() {
    let w = world();
    let (mut a, mut b) = (ChunkView::new(w.clone()), ChunkView::new(w.clone()));
    let p = chunk_payload(1);
    a.insert_level_chunk(-1, -1, 1, &p).unwrap();
    b.insert_level_chunk(-1, -1, 1, &p).unwrap();
    assert!(a.set_block(-3, 70, -5, 0, 9));
    assert!(a.set_block(-3, 70, -5, 1, 8));
    assert_eq!((b.block(-3, 70, -5), b.liquid(-3, 70, -5)), (9, 8));
    assert!(!a.set_block(100, 70, 100, 0, 9), "unloaded chunk");
    assert_eq!(a.insert_level_chunk(-1, -1, 1, &p).unwrap(), Inserted::Decoded);
    assert_eq!(b.block(-3, 70, -5), 1);
}

#[test]
fn a_lagging_view_does_not_revert_newer_updates() {
    let w = world();
    let (mut a, mut b) = (ChunkView::new(w.clone()), ChunkView::new(w.clone()));
    let p = chunk_payload(1);
    a.insert_level_chunk(0, 0, 1, &p).unwrap();
    b.insert_level_chunk(0, 0, 1, &p).unwrap();
    // Both connections receive the same updates; `b` processes its copies later.
    a.set_block(1, 70, 1, 0, 5);
    a.set_block(1, 70, 1, 0, 6);
    b.set_block(1, 70, 1, 0, 5);
    assert_eq!(b.block(1, 70, 1), 6, "a late copy of an older update must not win");
    b.insert_level_chunk(0, 0, 1, &chunk_payload(2)).unwrap();
    assert_eq!(a.block(0, 64, 0), 1, "nor a late copy of an older chunk");
}

#[test]
fn updates_pass_to_another_view_when_the_owner_drops_the_chunk() {
    let w = world();
    let (mut a, mut b) = (ChunkView::new(w.clone()), ChunkView::new(w.clone()));
    let p = chunk_payload(1);
    a.insert_level_chunk(0, 0, 1, &p).unwrap();
    b.insert_level_chunk(0, 0, 1, &p).unwrap();
    a.retain_within(50, 50, 1);
    b.set_block(1, 70, 1, 0, 7);
    assert_eq!(b.block(1, 70, 1), 7);
    drop(b);
    let mut c = ChunkView::new(w.clone());
    c.insert_level_chunk(0, 0, 1, &p).unwrap();
    assert_eq!(c.block(1, 70, 1), 1, "every view gone: the chunk was freed and decoded afresh");
}

#[test]
fn unloaded_reads_as_air() {
    let v = ChunkView::new(world());
    let air = BlockRegistry::vanilla().air_id();
    assert_eq!((v.block(5, 64, 5), v.liquid(5, 64, 5)), (air, air));
}

#[test]
fn concurrent_inserts_end_up_with_one_chunk() {
    let w = world();
    let p = Arc::new(chunk_payload(3));
    let views: Vec<ChunkView> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..8)
            .map(|_| {
                let (w, p) = (w.clone(), p.clone());
                s.spawn(move || {
                    let mut v = ChunkView::new(w);
                    v.insert_level_chunk(7, 7, 1, &p).unwrap();
                    v
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let first = views[0].chunk(7, 7).unwrap();
    assert!(views.iter().all(|v| Arc::ptr_eq(first, v.chunk(7, 7).unwrap())));
    assert_eq!(w.live_chunks(), 1);
}

#[test]
fn subscribers_see_applied_changes_only() {
    let w = world();
    let changes = w.subscribe();
    let (mut a, mut b) = (ChunkView::new(w.clone()), ChunkView::new(w.clone()));
    a.insert_level_chunk(1, 2, 1, &chunk_payload(1)).unwrap();
    b.insert_level_chunk(1, 2, 1, &chunk_payload(1)).unwrap();
    b.set_block(17, 64, 33, 0, 2);
    a.set_block(17, 64, 33, 0, 2);
    a.insert_sub_chunk(1, 5, 2, &section_v9(5, &[&filled(3)])).unwrap();
    let got: Vec<_> = changes.try_iter().collect();
    assert_eq!(got, [
        ChunkChange::Column { x: 1, z: 2 },
        ChunkChange::Block { x: 17, y: 64, z: 33 },
        ChunkChange::Section { x: 1, section_y: 5, z: 2 },
    ]);
    assert_eq!(w.chunk_positions(), [(1, 2)]);
}

#[test]
fn copy_section_unpacks_xzy() {
    let w = world();
    let mut v = ChunkView::new(w.clone());
    v.insert_level_chunk(0, 0, 1, &chunk_payload(1)).unwrap();
    v.set_block(3, 66, 5, 0, 9);
    let chunk = v.chunk(0, 0).unwrap().read();
    let (mut blocks, mut liquid) = ([0; SECTION_VOLUME], [0; SECTION_VOLUME]);
    assert!(!chunk.copy_section(0, &mut blocks, &mut liquid), "section 0 never sent");
    assert_eq!(chunk.section_uniform(0), Some(w.dimension().air));
    assert!(chunk.copy_section(8, &mut blocks, &mut liquid));
    assert_eq!(blocks[(3 << 8) | (5 << 4) | 2], 9);
    assert_eq!(blocks.iter().filter(|&&b| b == 1).count(), SECTION_VOLUME - 1);
    assert!(liquid.iter().all(|&l| l == w.dimension().air));
    assert_eq!(chunk.section_uniform(8), None);
}
