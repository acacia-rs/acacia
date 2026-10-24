//! A store driven line by line on stdin, for `tools/live.py` (bluemap `-u` against a world being written):
//! `cargo run --release -p acacia-anvil --example live -- <world dir>`. Commands, each answered `ok …`:
//! `init <side>` (synthetic terrain, chunks 0..side on both axes, written), `set <x> <y> <z> <block>`
//! (Bedrock name), `touch <cx> <cz>` (the chunk again, unchanged), `flush`, `quit`.
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::time::Duration;

use acacia_anvil::{AnvilStore, StoreConfig};
use acacia_world::{BlockRegistry, Chunk, Dimension};

fn terrain(reg: &BlockRegistry, cx: i32, cz: i32) -> Chunk {
    let id = |n: &str| reg.states_of(n).next().unwrap().0;
    let (stone, grass, torch) = (
        id("minecraft:stone"),
        id("minecraft:grass_block"),
        id("minecraft:torch"),
    );
    let mut c = Chunk::empty(cx, cz, Dimension::overworld(reg.air_id()));
    for x in cx * 16..cx * 16 + 16 {
        for z in cz * 16..cz * 16 + 16 {
            let h = (x.wrapping_mul(7919) ^ z.wrapping_mul(104_729)).rem_euclid(5);
            let top = 60 + h + ((x / 24 + z / 24) & 3);
            for y in 40..top {
                c.set(x, y, z, 0, stone);
            }
            c.set(x, top, z, 0, grass);
            if (x * 31 + z * 17).rem_euclid(97) == 0 {
                c.set(x, top + 1, z, 0, torch);
            }
        }
    }
    c
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let world = std::env::args().nth(1).ok_or("usage: live <world dir>")?;
    let reg = BlockRegistry::vanilla_arc();
    let dim = Dimension::overworld(reg.air_id());
    let cfg = StoreConfig {
        region_dir: acacia_anvil::region_dir(std::path::Path::new(&world), dim),
        has_sky: true,
        max_wait: Duration::ZERO,
        region_interval: Duration::ZERO,
        memory_limit: 1 << 30,
        threads: 0,
    };
    let mut store = AnvilStore::new(reg.clone(), cfg)?;
    let mut chunks: HashMap<(i32, i32), Chunk> = HashMap::new();
    let mut out = std::io::stdout().lock();
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        let w: Vec<&str> = line.split_whitespace().collect();
        let num = |i: usize| -> Result<i32, String> {
            w.get(i).and_then(|s| s.parse().ok()).ok_or(format!("bad argument {i}: {line}"))
        };
        let reply = match w.first().copied() {
            Some("init") => {
                let side = num(1)?;
                for cz in 0..side {
                    for cx in 0..side {
                        let c = terrain(&reg, cx, cz);
                        store.update(&c, []);
                        chunks.insert((cx, cz), c);
                    }
                }
                format!("{:?}", store.flush(true)?)
            }
            Some("set") => {
                let (x, y, z) = (num(1)?, num(2)?, num(3)?);
                let name = w.get(4).ok_or("missing block")?;
                let id = reg.states_of(name).next().ok_or("unknown block")?.0;
                let c = chunks.get_mut(&(x >> 4, z >> 4)).ok_or("no such chunk")?;
                c.set(x, y, z, 0, id);
                store.update(c, []);
                "set".into()
            }
            Some("touch") => {
                let c = chunks.get(&(num(1)?, num(2)?)).ok_or("no such chunk")?;
                store.update(c, []);
                "touched".into()
            }
            Some("flush") => format!("{:?}", store.flush(true)?),
            Some("quit") => break,
            _ => format!("unknown command: {line}"),
        };
        writeln!(out, "ok {reply}")?;
        out.flush()?;
    }
    Ok(())
}
