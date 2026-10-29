//! File mode of `examples/build.rs` (`SCHEMATIC=<path>`, research/schematic-trial.md): a `.mcstructure` built on
//! the bare pad, nothing pasted, from barrels stocked with its bill of materials.

use acacia_bot::build::{bill, BuildJob, Structure, Transform};
use acacia_bot::Bot;

use super::{Error, Pad};

/// Footprint corner (pad offset), as the scenes use.
const AT: [i32; 3] = [4, 0, -8];
/// Barrels go along this row (pad offsets [x, 0, -10]), one per 27 stacks.
const ROW_Z: i32 = -10;
const SLOTS: usize = 27;

fn stack_size(item: &str) -> u32 {
    let n = item.trim_start_matches("minecraft:");
    match () {
        _ if n.starts_with("bed") || n.ends_with("_bucket") || n == "cake" => 1,
        _ if n.ends_with("sign") || n.ends_with("banner") || n.ends_with("egg") || n == "snowball" => 16,
        _ => 64,
    }
}

/// The stacks for `structure`: its bill and half again (placed-wrong cells are broken and placed again), as
/// (item, aux, count) per slot.
pub fn stacks(structure: &Structure) -> Vec<(String, String, u32)> {
    let mut out = Vec::new();
    for (item, n) in bill(structure) {
        let (name, aux) = item.split_once('@').map_or((item.as_str(), "0"), |(n, a)| (n, a));
        let (max, mut left) = (stack_size(name), n + n / 2 + 2);
        while left > 0 {
            out.push((name.to_string(), aux.to_string(), left.min(max)));
            left = left.saturating_sub(max);
        }
    }
    out
}

/// Bare pad, barrels with the file's stacks, and the job.
pub async fn setup(a: &mut Bot, pad: &Pad, path: &str) -> Result<BuildJob, Error> {
    let structure = Structure::load(&std::fs::read(path)?)?;
    // Random ticks spread grass onto dirt targets and grow vines into air ones during long builds (outpost, 2026-10-28).
    pad.run(a, "/gamerule randomtickspeed 0");
    pad.build(a, &[], 5).await?;
    let stacks = stacks(&structure);
    let mut stations = Vec::new();
    for (i, chunk) in stacks.chunks(SLOTS).enumerate() {
        let [x, y, z] = pad.block([i as i32, 0, ROW_Z]);
        pad.run(a, &format!("/setblock {x} {y} {z} barrel"));
        a.wait_ticks(2).await?;
        for (slot, (item, aux, count)) in chunk.iter().enumerate() {
            pad.run(a, &format!("/replaceitem block {x} {y} {z} slot.container {slot} {item} {count} {aux}"));
        }
        a.wait_ticks(2).await?;
        stations.push([x, y, z]);
    }
    println!("stocked {} stacks in {} barrels: {:?}", stacks.len(), stations.len(), bill(&structure));
    a.wait_ticks(4).await?;
    let mut job = BuildJob::new(path, Transform { origin: pad.block(AT), ..Transform::default() });
    job.stations = stations;
    Ok(job)
}

/// Wall-time budget: 3 min plus 1.5 s a block to place (house16, ~600 blocks, takes ~250 s).
pub fn budget(path: &str) -> u64 {
    let blocks = std::fs::read(path).ok().and_then(|b| Structure::load(&b).ok()).map_or(0, |s| s.cells().filter(|(_, b)| !b.is_air()).count());
    180 + blocks as u64 * 3 / 2
}
