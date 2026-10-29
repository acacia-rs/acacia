//! Several builders sharing one job through a board, in one simulated world (see `build_sim.rs`).

use std::collections::HashMap;
use std::sync::Arc;

use acacia_physics::{BlockPos, Vec3};

use super::build_sim::{builder, items, run, step, view, World, LIVE_DELAY};
use super::build_tests::{scaffold_left, wrong, ORIGIN};
use super::grid::Grid;
use crate::build::findings;
use crate::build::journal::Journal;
use crate::build::{Block, Builder, LocalBoard, StatusHandle, Structure};
use crate::movement::Movement;
use crate::pathfind::process::{Arbiter, ProcId};

/// Board lease, in ticks.
pub(super) const LEASE: u64 = 200;

pub(super) struct SimBot {
    m: Movement,
    pub(super) arb: Arbiter,
    pub(super) id: ProcId,
    inv: HashMap<String, u32>,
    pub(super) status: StatusHandle,
    /// The bot stops ticking (killed) from this tick on.
    stop_at: Option<u32>,
    /// Most of one item ever held.
    most: u32,
    /// The tick its last act stops holding it.
    busy: u32,
    /// Ticks in a row its feet have been in (or just over) an open `World::targets` cell; the longest such run
    /// and its cell.
    open: u32,
    pub(super) worst_open: (u32, BlockPos),
}

pub(super) fn bot(b: Builder, status: StatusHandle, board: &Arc<LocalBoard>, who: &str, start: Vec3, inv: &[(&str, u32)]) -> SimBot {
    let mut arb = Arbiter::new();
    let id = arb.add(b.with_board(board.clone(), who), 10);
    let mut m = Movement::new();
    m.start(start, 0.0, 0.0);
    SimBot { m, arb, id, inv: items(inv), status, stop_at: None, most: 0, busy: 0, open: 0, worst_open: (0, [0; 3]) }
}

fn time_open(w: &World, b: &mut SimBot) {
    let Some(p) = b.m.position() else { return };
    let feet = p.map(|v| v.floor() as i32);
    let open = |c: BlockPos| w.targets.contains(&c) && view(w.g, c).0 == "minecraft:air";
    let at = [feet, [feet[0], feet[1] - 1, feet[2]]].into_iter().find(|c| open(*c));
    b.open = if at.is_some() { b.open + 1 } else { 0 };
    if let Some(c) = at.filter(|_| b.open > b.worst_open.0) {
        b.worst_open = (b.open, c);
    }
}

/// `dir/<who>.jsonl`, for a journal `findings::report` reads back.
pub(super) fn journal_file(dir: &std::path::Path, who: &str) -> std::path::PathBuf {
    std::fs::create_dir_all(dir).expect("journal dir");
    dir.join(format!("{who}.jsonl"))
}

fn live(b: &SimBot, tick: u32) -> bool {
    b.stop_at.is_none_or(|t| tick < t)
}

/// Ticks the bots in turn (each with its own inventory) until every live one finishes; returns the ticks taken.
pub(super) fn run_many(w: &mut World, bots: &mut [SimBot], board: &LocalBoard, max_ticks: u32) -> u32 {
    for tick in 0..max_ticks {
        board.set_now(tick as u64);
        for k in 0..bots.len() {
            if !live(&bots[k], tick) || bots[k].arb.outcome(bots[k].id).is_some() {
                continue;
            }
            w.others = bots.iter().enumerate().filter(|(j, o)| *j != k && live(o, tick)).filter_map(|(_, o)| o.m.position()).collect();
            (w.me, w.now) = (k, tick);
            w.clock.store(u64::from(tick), std::sync::atomic::Ordering::Relaxed);
            let b = &mut bots[k];
            if tick >= b.busy {
                std::mem::swap(&mut w.inv, &mut b.inv);
                b.busy = tick + step(w, &mut b.m, &mut b.arb, tick);
                std::mem::swap(&mut w.inv, &mut b.inv);
            }
            b.most = b.most.max(b.inv.values().copied().max().unwrap_or(0));
            b.m.tick(&w.g.physics());
            time_open(w, b);
        }
        if bots.iter().all(|b| !live(b, tick) || b.arb.outcome(b.id).is_some()) {
            return tick;
        }
    }
    max_ticks
}

fn plank_floor(side: i32) -> Structure {
    let mut s = Structure::new([side, 1, side]);
    (0..side).for_each(|x| (0..side).for_each(|z| s.set([x, 0, z], Some(Block::new("oak_planks", "")))));
    s
}

pub(super) fn house16_items() -> Vec<(&'static str, u32)> {
    ["cobblestone", "oak_planks", "oak_log", "glass_pane", "oak_slab", "oak_stairs", "torch", "ladder", "oak_fence", "cobblestone_wall", "stone_button", "furnace", "dispenser", "observer", "piston", "dirt"].iter().map(|i| (*i, 400)).collect()
}

#[test]
#[ignore = "slow in debug: cargo test --release -- --ignored two_bots"]
fn two_bots_build_house16_in_well_under_one_bots_time() {
    let s = crate::build::tests::house16::house16();
    let inv = house16_items();
    let solo = std::env::temp_dir().join(format!("acacia-swarm-sim-solo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&solo);
    let one = {
        let g = Grid::floor(-20, 40);
        let mut w = World::new(&g, &inv);
        let mut arb = Arbiter::new();
        let journal = Journal::new("solo", Box::new(std::fs::File::create(journal_file(&solo, "solo")).unwrap()), false).clocked(w.clock.clone());
        let id = arb.add(builder(&s, ORIGIN, &[]).0.with_journal(Arc::new(journal)), 10);
        run(&mut w, [0.5, 1.0, 0.5], &mut arb, id, 8000)
    };
    findings::report(&solo, &[]).unwrap().iter().for_each(|l| eprintln!("{l}"));
    let g = Grid::floor(-20, 40);
    let mut w = World::new(&g, &[]);
    // Each bot sees the other's blocks late, as live: the double-placed slab only happens so.
    w.delay = LIVE_DELAY;
    let board = Arc::new(LocalBoard::manual(LEASE));
    let dir = std::env::temp_dir().join(format!("acacia-swarm-sim-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let clock = w.clock.clone();
    let mut bots: Vec<SimBot> = [("a", [0.5, 1.0, 0.5]), ("b", [2.5, 1.0, 0.5])]
        .into_iter()
        .map(|(who, start)| {
            let (b, st) = builder(&s, ORIGIN, &[]);
            let journal = Journal::new(who, Box::new(std::fs::File::create(journal_file(&dir, who)).unwrap()), false).clocked(clock.clone());
            let b = b.with_journal(Arc::new(journal));
            bot(b, st, &board, who, start, &inv)
        })
        .collect();
    // Caps ~2.5x the expected ticks: a looping bot fails here in minutes, not after the testbox timeout.
    let two = run_many(&mut w, &mut bots, &board, 6000);
    for b in &bots {
        eprintln!("{:?}", b.status.lock().unwrap());
    }
    let report = findings::report(&dir, &wrong(&g, &s).iter().map(|m| m.0).collect::<Vec<_>>()).unwrap();
    report.iter().for_each(|l| eprintln!("{l}"));
    // Sim seconds against live (2026-10-21, 4 runs each): one bot 157.5 s, two bots 96.8 s.
    eprintln!("one bot {one} ticks ({:.0} s), two bots {two} ticks ({:.0} s): {:.2}", one as f32 / 20.0, two as f32 / 20.0, two as f32 / one as f32);
    assert!(bots.iter().all(|b| b.arb.outcome(b.id) == Some(&Ok(()))));
    assert_eq!(wrong(&g, &s), Vec::new());
    assert_eq!(scaffold_left(&g), Vec::<BlockPos>::new());
    let bugs: Vec<_> = report.iter().filter(|l| ["DOUBLE_PLACE", "STALE_CLAIM", "WALK_STUCK"].iter().any(|k| l.starts_with(k))).collect();
    assert!(bugs.is_empty(), "{bugs:?}");
    // A regression bound, not the B2 bar: sim ticks are not live seconds (sim 0.41 against live 0.62 once, and
    // a pilot fix that sped one bot up moved it the other way). The bar is judged live: tools/build-ab.sh.
    // 0.71: the pane-top ban (`moves.rs`) costs about 30 sim ticks; live two bots are at 0.67 of one (2026-10-29).
    assert!(two as f32 <= 0.71 * one as f32, "one {one} two {two}");
}

#[test]
#[ignore = "slow in debug: cargo test --release -- --ignored drops_after"]
fn house16_finishes_when_a_bot_drops_after_restocking() {
    // Live 2026-10-18: the kill run on share restock held forever after the drop.
    let s = crate::build::tests::house16::house16();
    let g = Grid::floor(-20, 40);
    let station = [-2, 1, -2];
    g.set_state(station, "barrel", "");
    let mut w = World::new(&g, &[]);
    w.stations.insert(station, house16_items().into_iter().map(|(i, n)| (format!("minecraft:{i}"), n * 2)).collect());
    let board = Arc::new(LocalBoard::manual(LEASE));
    let dir = std::env::temp_dir().join(format!("acacia-swarm-drops-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let clock = w.clock.clone();
    let mut bots: Vec<SimBot> = [("a", [0.5, 1.0, 0.5]), ("b", [2.5, 1.0, 0.5])]
        .into_iter()
        .map(|(who, start)| {
            let (b, st) = builder(&s, ORIGIN, &[station]);
            let journal = Journal::new(who, Box::new(std::fs::File::create(journal_file(&dir, who)).unwrap()), false).clocked(clock.clone());
            bot(b.with_journal(Arc::new(journal)), st, &board, who, start, &[("dirt", 64)])
        })
        .collect();
    eprintln!("journals in {}", dir.display());
    bots[1].stop_at = Some(600);
    let ticks = run_many(&mut w, &mut bots, &board, 8000);
    let (a, b) = (bots[0].status.lock().unwrap().clone(), bots[1].status.lock().unwrap().clone());
    eprintln!("{ticks} ticks: a {a:?}\nb {b:?}");
    assert!(b.restocks > 0 && b.placed > 0, "b restocked and built before it dropped");
    assert_eq!(bots[0].arb.outcome(bots[0].id), Some(&Ok(())), "a {a:?}");
    assert_eq!(wrong(&g, &s), Vec::new());
}

#[test]
fn a_dropped_bots_claim_returns_to_the_pool() {
    let g = Grid::floor(-20, 30);
    let s = plank_floor(10);
    let mut w = World::new(&g, &[]);
    let board = Arc::new(LocalBoard::manual(LEASE));
    let mut bots: Vec<SimBot> = [("a", [0.5, 1.0, 0.5]), ("b", [13.5, 1.0, 13.5])]
        .into_iter()
        .map(|(who, start)| {
            let (b, st) = builder(&s, ORIGIN, &[]);
            bot(b, st, &board, who, start, &[("oak_planks", 200), ("dirt", 64)])
        })
        .collect();
    bots[1].stop_at = Some(80);
    let ticks = run_many(&mut w, &mut bots, &board, 6000);
    let (a, b) = (bots[0].status.lock().unwrap().clone(), bots[1].status.lock().unwrap().clone());
    eprintln!("{ticks} ticks: a {a:?}\nb {b:?}");
    assert!(b.placed > 0, "b worked before it dropped");
    assert_eq!(bots[0].arb.outcome(bots[0].id), Some(&Ok(())));
    assert_eq!(wrong(&g, &s), Vec::new());
}

#[test]
fn bots_restock_only_their_share_of_the_need() {
    let g = Grid::floor(-20, 30);
    let station = [0, 1, 0];
    g.set_state(station, "barrel", "");
    let s = plank_floor(16);
    let mut w = World::new(&g, &[]);
    // Exactly the floor (four stacks): a bot taking for the whole job would leave the other none. The first bot
    // reaches the barrel on its first tick, before the other has ticked: a share must count it all the same.
    w.stations.insert(station, [("minecraft:oak_planks".to_string(), 256)].into());
    let board = Arc::new(LocalBoard::manual(LEASE));
    let mut bots: Vec<SimBot> = [("a", [1.5, 1.0, 0.5]), ("b", [0.5, 1.0, 1.5])]
        .into_iter()
        .map(|(who, start)| {
            let (b, st) = builder(&s, ORIGIN, &[station]);
            bot(b, st, &board, who, start, &[("dirt", 64)])
        })
        .collect();
    let ticks = run_many(&mut w, &mut bots, &board, 20_000);
    let placed: Vec<u32> = bots.iter().map(|b| b.status.lock().unwrap().placed).collect();
    let most: Vec<u32> = bots.iter().map(|b| b.most).collect();
    eprintln!("{ticks} ticks, placed {placed:?}, most held {most:?}");
    assert!(bots.iter().all(|b| b.arb.outcome(b.id) == Some(&Ok(()))));
    assert_eq!(wrong(&g, &s), Vec::new());
    assert!(placed.iter().all(|p| *p > 0), "both bots built: {placed:?}");
    assert!(most.iter().all(|m| *m <= 128), "most held {most:?}");
}
