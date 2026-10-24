use super::*;
use crate::region::{Region, region_file_name};
use acacia_world::Dimension;

fn store(dir: &Path) -> AnvilStore {
    store_with(dir, 1 << 30)
}

fn store_with(dir: &Path, memory_limit: usize) -> AnvilStore {
    let cfg = StoreConfig {
        region_dir: dir.to_owned(),
        has_sky: true,
        max_wait: Duration::ZERO,
        region_interval: Duration::ZERO,
        memory_limit,
        threads: 2,
    };
    AnvilStore::new(BlockRegistry::vanilla_arc(), cfg).unwrap()
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("acacia-anvil-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Stone floor at y 0; (1, 2) has a torch on its east edge; `changed` (2, 2) gains glowstone on both
/// edges and a roof on its west edge, so light changes on both sides of it.
fn terrain(cx: i32, cz: i32, changed: bool) -> Chunk {
    let reg = BlockRegistry::vanilla();
    let id = |n: &str| reg.states_of(n).next().unwrap().0;
    let mut c = Chunk::empty(cx, cz, Dimension::overworld(reg.air_id()));
    c.set_air_from(0);
    let (bx, bz) = (cx * 16, cz * 16);
    for x in bx..bx + 16 {
        for z in bz..bz + 16 {
            c.set(x, 0, z, 0, id("minecraft:stone"));
        }
    }
    if (cx, cz) == (1, 2) {
        c.set(bx + 15, 1, bz + 8, 0, id("minecraft:torch"));
    }
    if changed {
        c.set(bx + 15, 1, bz + 8, 0, id("minecraft:glowstone"));
        c.set(bx, 2, bz + 12, 0, id("minecraft:glowstone"));
        for x in bx..bx + 4 {
            for z in bz..bz + 16 {
                c.set(x, 4, z, 0, id("minecraft:stone"));
            }
        }
    }
    c
}

type Chunks = Vec<(Vec<u8>, u32)>;

fn read_area(dir: &Path) -> Chunks {
    let region = Region::load(&dir.join(region_file_name(0, 0))).unwrap();
    (0..25)
        .map(|i| {
            let (x, z) = (i % 5, i / 5);
            (region.chunk_nbt(x, z).unwrap(), region.timestamp(x, z).unwrap())
        })
        .collect()
}

/// Writes a 5×5 area, then changes its centre: every chunk's NBT and timestamp before and after.
fn relight_centre(dir: &Path, memory_limit: usize) -> (Chunks, Chunks) {
    let mut s = store_with(dir, memory_limit);
    for cz in 0..5 {
        for cx in 0..5 {
            s.update(&terrain(cx, cz, false), []);
        }
    }
    s.flush(true).unwrap();
    if memory_limit == 0 {
        assert_eq!(s.columns(), 0, "everything evicted");
    }
    let before = read_area(dir);
    s.update(&terrain(2, 2, true), []);
    s.flush(true).unwrap();
    (before, read_area(dir))
}

#[test]
fn nether_columns_are_java_height() {
    let reg = BlockRegistry::vanilla();
    let dim = Dimension::nether(reg.air_id());
    let dir = temp_dir("nether");
    let mut s = store_with(&region_dir(&dir, dim), 1 << 30);
    let mut c = Chunk::empty(0, 0, dim);
    assert!(!s.update(&c, []), "sections not received yet: skipped");
    c.set_air_from(0);
    c.set(1, 127, 1, 0, reg.states_of("minecraft:bedrock").next().unwrap().0);
    s.update(&c, []);
    s.flush(true).unwrap();
    let path = dir.join("dimensions/minecraft/the_nether/region").join(region_file_name(0, 0));
    let col = crate::decode::decode_chunk(&Region::load(&path).unwrap().chunk_nbt(0, 0).unwrap()).unwrap();
    assert_eq!((col.min_section, col.sections.len()), (0, 16));
    assert_ne!(col.block(1, 127, 1), crate::mapping::AIR);
    let mut unloaded = Chunk::empty(0, 0, dim);
    unloaded.set_air_from(0);
    assert!(!s.update(&unloaded, []), "an empty chunk does not replace a seen one");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn relighting_next_to_evicted_columns_matches_all_in_memory() {
    let (a, b) = (temp_dir("held"), temp_dir("evicted"));
    let (_, held) = relight_centre(&a, 1 << 30);
    let (before, evicted) = relight_centre(&b, 0);
    for (i, (h, e)) in held.iter().zip(&evicted).enumerate() {
        assert!(h.0 == e.0, "chunk {} {} differs", i % 5, i / 5);
    }
    let at = |x: usize, z: usize| z * 5 + x;
    for (x, z) in [(1, 2), (3, 2)] {
        assert!(
            before[at(x, z)].0 != evicted[at(x, z)].0,
            "light of evicted neighbour {x} {z} changed"
        );
    }
    // (1, 1) is read back and relit too, but its light does not change.
    for (x, z) in [(1, 1), (0, 0)] {
        assert_eq!(before[at(x, z)].1, evicted[at(x, z)].1, "{x} {z} keeps its timestamp");
    }
    std::fs::remove_dir_all(&a).unwrap();
    std::fs::remove_dir_all(&b).unwrap();
}

fn chunk(stone_at: i32) -> Chunk {
    let reg = BlockRegistry::vanilla();
    let mut c = Chunk::empty(3, -2, Dimension::overworld(reg.air_id()));
    c.set_air_from(0);
    let stone = reg.find("minecraft:stone", "").unwrap();
    for x in 48..64 {
        for z in -32..-16 {
            c.set(x, 0, z, 0, stone);
        }
    }
    c.set(50, stone_at, -30, 0, stone);
    c
}

#[test]
fn rewrites_bump_the_timestamp_and_unchanged_columns_keep_it() {
    let dir = std::env::temp_dir().join(format!("acacia-anvil-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut s = store(&dir);
    let path = dir.join(region_file_name(0, -1));
    let ts = || Region::load(&path).unwrap().timestamp(3, 30);

    s.update(&chunk(5), []);
    assert_eq!(s.flush(true).unwrap().columns, 1);
    let first = ts().expect("written");
    let nbt = Region::load(&path).unwrap().chunk_nbt(3, 30).unwrap();
    assert!(nbt.windows(14).any(|w| w == b"minecraft:full"));

    s.update(&chunk(5), []);
    s.flush(true).unwrap();
    assert_eq!(ts(), Some(first), "same content keeps the timestamp");

    s.update(&chunk(6), []);
    s.flush(true).unwrap();
    assert!(
        ts().unwrap() > first,
        "a change within the same second still moves it"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
