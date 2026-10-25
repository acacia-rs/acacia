//! Dumps the world a movement trace had loaded at its marks, for tools/parity's harness
//! `world_light`: at each mark's first time, the chunks within `radius` of the player's chunk (all of them
//! loaded), as Bedrock block states. One JSON line per scene: dimension, min_y, height, the
//! centre chunk, a palette of state keys (`lookpack::state_key`), and per chunk its non-air
//! sections: block and liquid layer palette indices, `(x << 8) | (z << 4) | y`.
//! `cargo run -p acacia-bot --example trace_region -- <trace.btrc.gz> <radius> <out.jsonl> <mark>...`

use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;

use acacia_bot::trace::{self, Event};
use acacia_bot::world::{SharedWorlds, WorldTracker};
use acacia_client::proto::Packet;
use acacia_client::proto::packets::ClientCacheMissResponse;
use acacia_client::{BlobStore, MemoryBlobStore};
use acacia_world::World;
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [path, radius, out, marks @ ..] = args.as_slice() else {
        return Err("usage: trace_region <trace> <radius> <out.jsonl> <mark>...".into());
    };
    let radius: i32 = radius.parse()?;
    let mut world = WorldTracker::new("region".into(), SharedWorlds::new());
    let blobs = Arc::new(MemoryBlobStore::with_payloads());
    world.set_blob_store(blobs.clone());
    let mut feet = [0.0f32; 3];
    let mut file = std::fs::File::create(out)?;
    let mut written = 0;
    let mut seen = std::collections::HashSet::new();
    for event in trace::read(std::path::Path::new(path))? {
        match event {
            Event::Start { feet: f, .. } => feet = f,
            Event::Packet(p) => {
                if p.id == ClientCacheMissResponse::ID
                    && let Ok(r) = p.decode::<ClientCacheMissResponse>()
                {
                    r.blobs.iter().for_each(|b| blobs.insert(b.hash, &b.payload));
                }
                let _ = world.apply(&p);
            }
            Event::Mark(label) if marks.contains(&label) && seen.insert(label.clone()) => {
                let Some(view) = world.view() else { continue };
                let centre = ((feet[0].floor() as i32) >> 4, (feet[2].floor() as i32) >> 4);
                match scene(view.world(), &label, centre, radius) {
                    Some(line) => {
                        writeln!(file, "{line}")?;
                        written += 1;
                    }
                    None => eprintln!("{label}: chunks around {centre:?} not all loaded"),
                }
            }
            _ => {}
        }
    }
    println!("{written} scenes");
    Ok(())
}

fn scene(world: &World, label: &str, (cx, cz): (i32, i32), radius: i32) -> Option<serde_json::Value> {
    let dim = world.dimension();
    let registry = world.registry();
    let mut palette: Vec<String> = Vec::new();
    let mut index: HashMap<u32, usize> = HashMap::new();
    let mut key = |id: u32| -> usize {
        *index.entry(id).or_insert_with(|| {
            let s = registry.get(id).map_or_else(|| "minecraft:air".to_owned(), |s| {
                if s.properties.is_empty() { s.name.to_owned() } else { format!("{}[{}]", s.name, s.properties) }
            });
            palette.push(s);
            palette.len() - 1
        })
    };
    let mut chunks = Vec::new();
    for x in cx - radius..=cx + radius {
        for z in cz - radius..=cz + radius {
            let shared = world.get(x, z)?;
            let chunk = shared.read();
            let mut sections = Vec::new();
            for sy in 0..dim.height as i32 / 16 {
                let y0 = dim.min_y + sy * 16;
                let (mut blocks, mut liquid) = (vec![0usize; 4096], vec![0usize; 4096]);
                let (mut solid, mut wet) = (false, false);
                for i in 0..4096i32 {
                    let (bx, bz, by) = (x * 16 + (i >> 8), z * 16 + (i >> 4 & 15), y0 + (i & 15));
                    let (b, l) = (chunk.block(bx, by, bz), chunk.liquid(bx, by, bz));
                    solid |= b != dim.air;
                    wet |= l != dim.air;
                    blocks[i as usize] = key(b);
                    liquid[i as usize] = key(l);
                }
                if solid || wet {
                    sections.push(json!({ "y": sy, "blocks": blocks, "liquid": if wet { json!(liquid) } else { json!(null) } }));
                }
            }
            chunks.push(json!({ "x": x, "z": z, "sections": sections }));
        }
    }
    let name = match (dim.min_y, dim.height) {
        (-64, _) => "overworld",
        (_, 128) => "the_nether",
        _ => "the_end",
    };
    Some(json!({ "scene": label, "dimension": name, "min_y": dim.min_y, "height": dim.height, "centre": [cx, cz], "radius": radius, "palette": palette, "chunks": chunks }))
}
