//! Two bots on house16 from the live pad, over several seeds (start spacing and how late each sees the other's
//! blocks): no bot stands in an open target cell for long, and no task's walks fail again and again. Live
//! 2026-10-29: a bot wedged in the west window opening failed every walk ("Stuck 10 times") for minutes.

use std::collections::HashMap;
use std::sync::Arc;

use acacia_physics::BlockPos;

use super::build_sim::{builder, World};
use super::build_tests::{wrong, ORIGIN};
use super::grid::Grid;
use super::swarm_sim::{bot, house16_items, journal_file, run_many, SimBot, LEASE};
use crate::build::journal::Journal;
use crate::build::LocalBoard;

/// Ticks in an open target cell that count as wedged.
const WEDGED: u32 = 100;

/// Failed walks per task, from the journals in `dir`.
fn failed_walks(dir: &std::path::Path) -> HashMap<(String, BlockPos), u32> {
    let mut out = HashMap::new();
    for e in std::fs::read_dir(dir).expect("journals").flatten() {
        for l in std::fs::read_to_string(e.path()).unwrap_or_default().lines() {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(l) else { continue };
            if v["e"] == "walk_end" && v["end"] == "failed" {
                let task: Vec<i32> = v["task"].as_array().into_iter().flatten().filter_map(|n| n.as_i64()).map(|n| n as i32).collect();
                if let [x, y, z] = task[..] {
                    *out.entry((v["bot"].as_str().unwrap_or("?").to_string(), [x, y, z])).or_default() += 1;
                }
            }
        }
    }
    out
}

/// One seed: the verdict line, `Err` when a bot wedged, a task's walks failed twice, or the build went wrong.
fn seed(k: u32) -> Result<String, String> {
    let s = crate::build::tests::house16::house16();
    let g = Grid::floor(-20, 40);
    let at = |p: BlockPos| [ORIGIN[0] + p[0], ORIGIN[1] + p[1], ORIGIN[2] + p[2]];
    // examples/build.rs's pad, as `builds_house16_as_live`.
    g.fill(at([2, 0, 2]), at([4, 1, 4]), "dirt");
    g.fill(at([8, 2, 10]), at([9, 2, 11]), "stone");
    let mut w = World::new(&g, &[]);
    w.delay = 1 + k % 4;
    // Not the bottom layer: a bot stands on the ground the floor will cover.
    w.targets = s.cells().filter(|(p, b)| p[1] > 0 && !b.is_air()).map(|(p, _)| at(p)).collect();
    let board = Arc::new(LocalBoard::manual(LEASE));
    let dir = std::env::temp_dir().join(format!("acacia-swarm-wedge-{}-{k}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let clock = w.clock.clone();
    let a = [ORIGIN[0] as f32 - 3.5, 1.0, ORIGIN[2] as f32 + 8.5];
    let starts = [("a", a), ("b", [a[0] + 2.0 + (k / 4) as f32, 1.0, a[2] - (k % 3) as f32])];
    let mut bots: Vec<SimBot> = starts
        .into_iter()
        .map(|(who, start)| {
            let (b, st) = builder(&s, ORIGIN, &[]);
            let journal = Journal::new(who, Box::new(std::fs::File::create(journal_file(&dir, who)).unwrap()), false).clocked(clock.clone());
            bot(b.with_journal(Arc::new(journal)), st, &board, who, start, &house16_items())
        })
        .collect();
    let ticks = run_many(&mut w, &mut bots, &board, 6000);
    let fails = failed_walks(&dir);
    let repeated: Vec<_> = fails.iter().filter(|(_, n)| **n >= 2).collect();
    let wedged: Vec<_> = bots.iter().map(|b| b.worst_open).filter(|(n, _)| *n > WEDGED).collect();
    let done = bots.iter().all(|b| b.arb.outcome(b.id) == Some(&Ok(()))) && wrong(&g, &s).is_empty();
    let line = format!("seed {k} delay {}: {ticks} ticks, failed walks {}, repeated {repeated:?}, wedged {wedged:?}, done {done}", w.delay, fails.values().sum::<u32>());
    if done && repeated.is_empty() && wedged.is_empty() { Ok(line) } else { Err(line) }
}

#[test]
#[ignore = "slow in debug: cargo test --release -- --ignored never_wedge"]
fn two_bots_never_wedge_in_open_cells() {
    let seeds: u32 = std::env::var("WEDGE_SEEDS").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
    let results: Vec<Result<String, String>> = (0..seeds).map(seed).collect();
    results.iter().for_each(|r| eprintln!("{}", r.as_ref().map_or_else(|e| format!("BAD {e}"), |l| format!("ok  {l}"))));
    assert!(results.iter().all(|r| r.is_ok()));
}
