//! Conversion throughput on synthetic terrain (stone to y 60 with scattered blocks, grass, torches):
//! `cargo run --release -p acacia-anvil --example bench -- [side=20] [out dir]`. Prints columns/s for
//! convert (`update`) and for light + encode + write (`flush`).
use std::time::{Duration, Instant};

use acacia_anvil::{AnvilStore, StoreConfig};
use acacia_world::{BlockRegistry, Chunk, Dimension};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let side: i32 = args.next().map_or(Ok(20), |s| s.parse())?;
    let dir = args.next().map_or_else(
        || std::env::temp_dir().join("acacia-anvil-bench"),
        Into::into,
    );
    let _ = std::fs::remove_dir_all(&dir);
    let reg = BlockRegistry::vanilla_arc();
    let id = |n: &str| reg.states_of(n).next().unwrap().0;
    let (stone, dirt, grass, ore, torch) = (
        id("minecraft:stone"),
        id("minecraft:dirt"),
        id("minecraft:grass_block"),
        id("minecraft:iron_ore"),
        id("minecraft:torch"),
    );
    let mut rng = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    let chunks: Vec<Chunk> = (0..side * side)
        .map(|i| {
            let (cx, cz) = (i % side, i / side);
            let mut c = Chunk::empty(cx, cz, Dimension::overworld(reg.air_id()));
            for x in cx * 16..cx * 16 + 16 {
                for z in cz * 16..cz * 16 + 16 {
                    let top = 56 + (next() % 8) as i32;
                    for y in -64..top {
                        let b = if next() % 50 == 0 {
                            ore
                        } else if y > top - 4 {
                            dirt
                        } else {
                            stone
                        };
                        c.set(x, y, z, 0, b);
                    }
                    c.set(x, top, z, 0, grass);
                    if next() % 97 == 0 {
                        c.set(x, top + 1, z, 0, torch);
                    }
                }
            }
            c
        })
        .collect();

    let cfg = StoreConfig {
        region_dir: dir.clone(),
        has_sky: true,
        max_wait: Duration::ZERO,
        region_interval: Duration::ZERO,
        memory_limit: 1 << 32,
        threads: std::env::var("ANVIL_THREADS")
            .ok()
            .and_then(|t| t.parse().ok())
            .unwrap_or(0),
    };
    let mut store = AnvilStore::new(reg.clone(), cfg)?;
    let t = Instant::now();
    for c in &chunks {
        store.update(c, []);
    }
    let convert = t.elapsed();
    let t = Instant::now();
    let stats = store.flush(true)?;
    let flush = t.elapsed();
    let n = chunks.len() as f64;
    println!(
        "{} columns: convert {:.0}/s, light+encode+write {:.0}/s ({} regions), {} MB held",
        chunks.len(),
        n / convert.as_secs_f64(),
        stats.columns as f64 / flush.as_secs_f64(),
        stats.regions,
        store.heap_bytes() >> 20
    );
    Ok(())
}
