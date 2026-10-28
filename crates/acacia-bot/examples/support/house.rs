//! The live builder setups (`examples/build.rs`, `examples/swarm_build.rs`): a scene's structure on the pad, two
//! stocked barrels, junk to clear, and an observer's cell-by-cell check afterwards. `house16` is the acceptance
//! build; the small scenes reproduce single live failures in seconds (regression checks).

use std::path::PathBuf;

use acacia_bot::build::journal::Journal;
use acacia_bot::build::{check, findings, Block, BuildJob, BuildStatus, Builder, Mismatch, Phase, Structure, Transform};
use acacia_bot::pathfind::process::{BlockView, Status};
use acacia_bot::pathfind::Processes;
use acacia_bot::Bot;

use super::{Error, Fill, Pad};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/build/house16.mcstructure");
const MAX_TICKS: u32 = 40_000;
/// Footprint corner (pad offset).
const AT: [i32; 3] = [4, 0, -8];
const STATIONS: [[i32; 3]; 2] = [[0, 0, -10], [1, 0, -10]];
/// Stacks per barrel, as (item, stacks): house16 twice over. An offline bot that reconnects comes back with an
/// empty inventory (BDS 1.26.52, seen 2026-10-17), so what a dropped bot took is gone.
const STOCK: [&[(&str, u32)]; 2] = [
    &[("cobblestone", 9), ("oak_planks", 6), ("oak_log", 2), ("glass_pane", 2), ("oak_slab", 3), ("oak_stairs", 1)],
    &[("torch", 2), ("ladder", 2), ("oak_fence", 2), ("cobblestone_wall", 2), ("stone_button", 2), ("furnace", 2), ("dispenser", 2), ("observer", 2), ("piston", 2), ("oak_stairs", 1)],
];

/// The structure for `scene` and junk to clear (pad offsets): `house16`; `wall`, house16's wall with its log
/// ends and slab rim; `ladder`, the same wall over a floor with house16's ladder column inside (live 2026-10-18:
/// the bot climbed the half-built ladder and stranded in its top cell, every stand below unreachable); `logrow`, a
/// floor strip with house16's x-axis log row between cobblestone ends; `junk`, a plank floor under a
/// furnace where junk dirt stood (the cell BDS kept mining out).
fn scene(name: &str) -> Result<(PathBuf, Vec<Fill<'static>>), Error> {
    let at = |p: [i32; 3]| [AT[0] + p[0], AT[1] + p[1], AT[2] + p[2]];
    let put = |s: &mut Structure, p: [i32; 3], name: &str, props: &str| s.set(p, Some(Block::new(name, props)));
    let s = match name {
        "house16" => return Ok((FIXTURE.into(), vec![([6, 0, -6], [8, 1, -4], "dirt"), ([12, 2, 2], [13, 2, 3], "stone")])),
        "wall" => {
            let mut s = Structure::new([8, 6, 1]);
            for x in 0..8 {
                for y in 0..6 {
                    let (n, p) = match () {
                        _ if x == 0 || x == 7 => ("oak_log", "pillar_axis=y"),
                        _ if y == 5 => ("oak_slab", "minecraft:vertical_half=bottom"),
                        _ => ("cobblestone", ""),
                    };
                    put(&mut s, [x, y, 0], n, p);
                }
            }
            (s, vec![])
        }
        "ladder" => {
            let mut s = Structure::new([8, 6, 3]);
            for x in 0..8 {
                for z in 0..3 {
                    put(&mut s, [x, 0, z], "oak_planks", "");
                    for y in 1..6 {
                        let (n, p) = match () {
                            _ if z > 0 => ("air", ""),
                            _ if x == 0 || x == 7 => ("oak_log", "pillar_axis=y"),
                            _ if y == 5 => ("oak_slab", "minecraft:vertical_half=bottom"),
                            _ => ("cobblestone", ""),
                        };
                        put(&mut s, [x, y, z], n, p);
                    }
                }
            }
            (1..5).for_each(|y| put(&mut s, [4, y, 1], "ladder", "facing_direction=3"));
            (s, vec![])
        }
        "logrow" => {
            // house16's floor at its x-axis log row: cobblestone ends, logs between, planks either side (live
            // 2026-10-19: the log beside the cobblestone end ended air).
            let mut s = Structure::new([8, 1, 3]);
            for x in 0..8 {
                for z in 0..3 {
                    let (n, p) = match () {
                        _ if x == 0 || x == 7 => ("cobblestone", ""),
                        _ if z == 1 => ("oak_log", "pillar_axis=x"),
                        _ => ("oak_planks", ""),
                    };
                    put(&mut s, [x, 0, z], n, p);
                }
            }
            (s, vec![])
        }
        "junk" => {
            let mut s = Structure::new([3, 2, 3]);
            for x in 0..3 {
                for z in 0..3 {
                    put(&mut s, [x, 0, z], "oak_planks", "");
                    put(&mut s, [x, 1, z], "air", "");
                }
            }
            put(&mut s, [1, 1, 1], "furnace", "minecraft:cardinal_direction=south");
            (s, vec![(at([0, 0, 0]), at([2, 1, 2]), "dirt")])
        }
        _ => match (super::scenes_twocell::scene(name, at), super::oneclick::scene(name)) {
            (Some((s, junk, _)), _) => (s, junk),
            (None, Some(s)) => (s, vec![]),
            (None, None) => return Err(format!("unknown scene {name}").into()),
        },
    };
    let (s, junk) = s;
    let path = std::env::temp_dir().join(format!("acacia-scene-{name}.mcstructure"));
    std::fs::write(&path, s.to_bytes())?;
    Ok((path, junk))
}

/// Rebuilds the pad with the scene's junk, stocks the barrels, and returns the job.
pub async fn setup(a: &mut Bot, pad: &Pad, name: &str) -> Result<BuildJob, Error> {
    let (structure, junk) = scene(name)?;
    pad.build(a, &junk, 5).await?;
    let stock = super::oneclick::stock(name).map_or(STOCK, |s| [s, &[]]);
    for (station, items) in STATIONS.iter().zip(stock) {
        let [x, y, z] = pad.block(*station);
        pad.run(a, &format!("/setblock {x} {y} {z} barrel"));
        a.wait_ticks(2).await?;
        let mut slot = 0;
        for (item, stacks) in items {
            for _ in 0..*stacks {
                pad.run(a, &format!("/replaceitem block {x} {y} {z} slot.container {slot} {item} 64"));
                a.wait_ticks(2).await?;
                slot += 1;
            }
        }
    }
    let mut job = BuildJob::new(structure, Transform { origin: pad.block(AT), ..Transform::default() });
    job.stations = STATIONS.iter().map(|&s| pad.block(s)).collect();
    if let Some((_, _, stock)) = super::scenes_twocell::scene(name, |p| p) {
        job.stations.push(pad.block(super::scenes_twocell::station(a, pad, stock).await?));
    }
    Ok(job)
}

/// Empties the inventory, then a pickaxe, a shovel and scaffold dirt. Without the shovel each scaffold dirt
/// took the hand's ~780 ms to break (34 a house, live).
pub async fn kit(bot: &mut Bot, pad: &Pad) -> Result<(), Error> {
    for c in ["/clear @s", "/give @s diamond_pickaxe", "/give @s diamond_shovel", "/give @s dirt 64"] {
        pad.run(bot, c);
        bot.wait_ticks(4).await?;
    }
    Ok(())
}

/// Runs `builder` until done, `kill_at` placements (0: never), the tick cap or a long stall; returns ticks, the
/// last status and corrections. `label` prefixes the progress lines.
pub async fn drive(bot: &mut Bot, builder: Builder, kill_at: u32, label: &str) -> Result<(u32, BuildStatus, u32), Error> {
    let before = super::corrections(bot);
    let diag = Builder::new(builder.job().clone())?;
    let (mut last_done, mut still) = (0, 0);
    let status = builder.status();
    let mut procs = Processes::new();
    let id = procs.add(builder, 10);
    let mut ticks = 0;
    // Wall time by what the tick did (status / build phase): where a build's time goes.
    let mut split: std::collections::BTreeMap<String, std::time::Duration> = Default::default();
    let profile = |split: &std::collections::BTreeMap<String, std::time::Duration>| {
        let mut v: Vec<_> = split.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1));
        println!("{label} time: {}", v.iter().map(|(k, d)| format!("{k} {:.0}s", d.as_secs_f32())).collect::<Vec<_>>().join(", "));
    };
    loop {
        let t0 = std::time::Instant::now();
        let st = procs.tick(bot).await?;
        ticks += 1;
        let mut s = status.lock().expect("status").clone();
        // Moving by the pilot's state: searching ticks hold the keys (research/build-walk-overhead.md).
        let key = match st {
            Status::Moving(nav) => format!("moving.{nav:?}"),
            Status::Holding => "holding".into(),
            Status::Acted => "acting".into(),
            _ => "idle".into(),
        };
        *split.entry(format!("{key}/{:?}", s.phase)).or_default() += t0.elapsed();
        if ticks % 400 == 0 {
            println!("{label} tick {ticks}: {:?} done {}/{} place {} break {} placed {} restocks {} missing {:?}", s.phase, s.done, s.cells, s.to_place, s.to_break, s.placed, s.restocks, s.missing);
        }
        (still, last_done) = if s.done == last_done { (still + 1, last_done) } else { (0, s.done) };
        if still > 0 && still % 1000 == 0 {
            let m = bot.movement();
            println!("{label} STALL at {:?} ground {:?} status {st:?} failed {:?}", m.and_then(|m| m.position()), m.map(|m| m.on_ground()), s.failed);
            diag.explain_for(bot).iter().take(12).for_each(|l| println!("  OPEN {l}"));
        }
        if st == Status::Empty || procs.outcome(id).is_some() {
            println!("{label} outcome {:?}", procs.outcome(id));
            profile(&split);
            println!("{label} act timing:\n{}", bot.act_timing().table());
            if procs.outcome(id) == Some(&Ok(())) {
                s.phase = Phase::Done;
            }
            return Ok((ticks, s, super::corrections(bot) - before));
        }
        if (kill_at > 0 && s.placed >= kill_at) || ticks >= MAX_TICKS || still > 3000 {
            profile(&split);
            println!("{label} act timing:\n{}", bot.act_timing().table());
            return Ok((ticks, s, super::corrections(bot) - before));
        }
    }
}

/// Where the run's journals go (`BUILD_JOURNAL`, default `journal/` in the working directory).
fn journal_dir() -> PathBuf {
    std::env::var_os("BUILD_JOURNAL").map_or_else(|| "journal".into(), PathBuf::from)
}

/// Clears the journals of an earlier run.
pub fn fresh_journals() {
    let _ = std::fs::remove_dir_all(journal_dir());
}

/// `builder` writing its journal as `name`.
pub fn journaled(builder: Builder, name: &str) -> Result<Builder, Error> {
    Ok(builder.with_journal(std::sync::Arc::new(Journal::create(&journal_dir(), name)?)))
}

/// Prints the journals' summary and findings, and the history of every wrong cell.
pub fn report(wrong: &[Mismatch]) {
    match findings::report(&journal_dir(), &wrong.iter().take(20).map(|m| m.pos).collect::<Vec<_>>()) {
        Ok(lines) => lines.iter().for_each(|l| println!("{l}")),
        Err(e) => println!("no journal report: {e}"),
    }
}

/// Connects `observer` over the pad and compares every cell with the job's structure.
pub async fn observe(server: &str, observer: &str, pad: &Pad, job: &BuildJob) -> Result<Vec<Mismatch>, Error> {
    let structure = Structure::load(&std::fs::read(&job.structure)?)?;
    let (mut o, _) = super::connect(server, observer).await?;
    let spot = pad.block([12, 9, 0]);
    pad.run(&o, &format!("/tp @s {} {} {}", spot[0] as f32 + 0.5, spot[1], spot[2] as f32 + 0.5));
    o.wait_ticks(80).await?;
    let mut wrong = check(&structure, &job.at, |p| o.block_state(p).map(BlockView::of));
    wrong.extend(acacia_bot::build::colors(&structure, &job.at, |p| match o.state().block_entities.get(p)?.value.get("color") {
        Some(acacia_client::proto::nbt::Value::Byte(c)) => Some(*c as u8),
        _ => None,
    }));
    o.disconnect().await;
    Ok(wrong)
}
