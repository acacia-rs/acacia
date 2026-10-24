use super::*;
use crate::region::{Region, region_file_name};
use acacia_world::Dimension;

fn store(dir: &Path) -> AnvilStore {
    let cfg = StoreConfig {
        region_dir: dir.to_owned(),
        has_sky: true,
        max_wait: Duration::ZERO,
        region_interval: Duration::ZERO,
        memory_limit: 1 << 30,
        threads: 2,
    };
    AnvilStore::new(BlockRegistry::vanilla_arc(), cfg).unwrap()
}

fn chunk(stone_at: i32) -> Chunk {
    let reg = BlockRegistry::vanilla();
    let mut c = Chunk::empty(3, -2, Dimension::overworld(reg.air_id()));
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
