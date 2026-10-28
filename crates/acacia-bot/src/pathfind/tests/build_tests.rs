//! Builder runs in the headless simulation (see `build_sim.rs`).

use acacia_physics::BlockPos;

use super::build_sim::{builder, explain, run, view, World};
use super::grid::Grid;
use crate::build::{Block, Structure};
use crate::pathfind::process::{Act, Arbiter};

pub(super) const ORIGIN: BlockPos = [3, 1, 3];

fn b(name: &str, props: &str) -> Option<Block> {
    Some(Block::new(name, props))
}

/// 5×3×5: a plank floor with a log, a double slab and a furnace; walls with stairs (both halves), slabs and a
/// torch; air inside.
fn house() -> Structure {
    let mut s = Structure::new([5, 3, 5]);
    for x in 0..5 {
        for z in 0..5 {
            s.set([x, 0, z], b("oak_planks", ""));
            for y in 1..3 {
                let edge = x == 0 || x == 4 || z == 0 || z == 4;
                s.set([x, y, z], if edge && y == 1 { b("cobblestone", "") } else { b("air", "") });
            }
        }
    }
    s.set([1, 0, 1], b("oak_log", "pillar_axis=x"));
    s.set([3, 0, 1], b("oak_double_slab", "minecraft:vertical_half=bottom"));
    s.set([2, 0, 3], b("furnace", "minecraft:cardinal_direction=north"));
    s.set([2, 1, 0], b("oak_stairs", "upside_down_bit=0,weirdo_direction=2"));
    s.set([0, 1, 2], b("oak_stairs", "upside_down_bit=1,weirdo_direction=0"));
    s.set([4, 1, 2], b("oak_slab", "minecraft:vertical_half=top"));
    s.set([2, 2, 4], b("oak_slab", "minecraft:vertical_half=bottom"));
    s.set([1, 2, 0], b("cobblestone", ""));
    s.set([1, 2, 1], b("torch", "torch_facing_direction=north"));
    s
}

const ITEMS: &[(&str, u32)] = &[("oak_planks", 64), ("cobblestone", 64), ("oak_log", 8), ("oak_slab", 8), ("furnace", 2), ("oak_stairs", 8), ("torch", 4), ("dirt", 64)];

/// Cells whose block or click-decided states differ from the target.
pub(super) fn wrong(g: &Grid, s: &Structure) -> Vec<(BlockPos, String, String)> {
    let skip = |k: &str| k.contains("connection") || k.contains("corner") || k.starts_with(crate::build::BED_COLOR);
    s.cells()
        .filter_map(|(p, t)| {
            let pos = [ORIGIN[0] + p[0], ORIGIN[1] + p[1], ORIGIN[2] + p[2]];
            let (name, props) = view(g, pos);
            let double = t.name.ends_with("_double_slab");
            let same = name == t.name && t.props.split(',').filter(|kv| !kv.is_empty()).all(|kv| skip(kv) || (double && kv.contains("vertical_half")) || props.split(',').any(|w| w == kv));
            (!same).then(|| (pos, format!("{} [{}]", t.name, t.props), format!("{name} [{props}]")))
        })
        .collect()
}

pub(super) fn scaffold_left(g: &Grid) -> Vec<BlockPos> {
    let mut out = Vec::new();
    for x in -6..16 {
        for y in 1..10 {
            for z in -6..16 {
                if view(g, [x, y, z]).0 == "minecraft:dirt" {
                    out.push([x, y, z]);
                }
            }
        }
    }
    out
}

#[test]
fn builds_a_mixed_structure_block_exact() {
    let g = Grid::floor(-20, 30);
    // Mismatches to clear: dirt where air belongs, gravel where planks do.
    g.fill([4, 2, 4], [6, 3, 6], "dirt");
    g.set_state([5, 1, 5], "gravel", "");
    let s = house();
    let mut w = World::new(&g, ITEMS);
    let (proc, status) = builder(&s, ORIGIN, &[]);
    let mut arb = Arbiter::new();
    let id = arb.add(proc, 10);
    let ticks = run(&mut w, [0.5, 1.0, 0.5], &mut arb, id, 12_000);
    let st = status.lock().unwrap().clone();
    eprintln!("{ticks} ticks, {st:?}");
    explain(&w, &s, ORIGIN).iter().for_each(|l| eprintln!("OPEN {l}"));
    assert_eq!(arb.outcome(id), Some(&Ok(())), "status {st:?}");
    assert_eq!(wrong(&g, &s), Vec::new());
    assert!(st.broken >= 10, "cleared the dirt and gravel: {st:?}");
    assert_eq!(scaffold_left(&g), Vec::<BlockPos>::new(),"scaffold removed");
}

#[test]
#[ignore = "slow in debug: cargo test --release -- --ignored builds_house16"]
fn builds_house16() {
    house16_from(&Grid::floor(-20, 40), [0.5, 1.0, 0.5]);
}

#[test]
#[ignore = "slow in debug: cargo test --release -- --ignored builds_house16"]
fn builds_house16_as_live() {
    // examples/build.rs's pad: junk in the footprint, the barrels beside it, the bot's start.
    let g = Grid::floor(-20, 40);
    let at = |p: BlockPos| [ORIGIN[0] + p[0], ORIGIN[1] + p[1], ORIGIN[2] + p[2]];
    g.fill(at([2, 0, 2]), at([4, 1, 4]), "dirt");
    g.fill(at([8, 2, 10]), at([9, 2, 11]), "stone");
    g.set_state(at([-4, 0, -2]), "barrel", "").set_state(at([-3, 0, -2]), "barrel", "");
    house16_from(&g, [ORIGIN[0] as f32 - 3.5, 1.0, ORIGIN[2] as f32 + 8.5]);
}

fn house16_from(g: &Grid, start: [f32; 3]) {
    let s = crate::build::tests::house16::house16();
    let items: Vec<(&str, u32)> = ["cobblestone", "oak_planks", "oak_log", "glass_pane", "oak_slab", "oak_stairs", "torch", "ladder", "oak_fence", "cobblestone_wall", "stone_button", "furnace", "dispenser", "observer", "piston", "dirt"]
        .iter()
        .map(|i| (*i, 400))
        .collect();
    let mut w = World::new(g, &items);
    let (proc, status) = builder(&s, ORIGIN, &[]);
    let mut arb = Arbiter::new();
    let id = arb.add(proc, 10);
    let ticks = run(&mut w, start, &mut arb, id, 8000);
    let st = status.lock().unwrap().clone();
    eprintln!("{ticks} ticks, {st:?}");
    explain(&w, &s, ORIGIN).iter().take(30).for_each(|l| eprintln!("OPEN {l}"));
    eprintln!("scaffold left {:?}", scaffold_left(g));
    assert_eq!(arb.outcome(id), Some(&Ok(())), "status {st:?}");
    assert_eq!(wrong(g, &s), Vec::new());
    assert_eq!(scaffold_left(g), Vec::<BlockPos>::new());
}

#[test]
fn restocks_what_is_still_needed_from_a_station() {
    let g = Grid::floor(-20, 30);
    let station = [0, 1, 6];
    g.set_state(station, "barrel", "");
    let mut s = Structure::new([4, 1, 4]);
    (0..4).for_each(|x| (0..4).for_each(|z| s.set([x, 0, z], b("oak_planks", ""))));
    let mut w = World::new(&g, &[("dirt", 16)]);
    w.stations.insert(station, [("minecraft:oak_planks".to_string(), 200), ("minecraft:stone".to_string(), 64)].into());
    let (proc, status) = builder(&s, ORIGIN, &[station]);
    let mut arb = Arbiter::new();
    let id = arb.add(proc, 10);
    run(&mut w, [0.5, 1.0, 0.5], &mut arb, id, 6000);
    let st = status.lock().unwrap().clone();
    explain(&w, &s, ORIGIN).iter().for_each(|l| eprintln!("OPEN {l}"));
    assert_eq!(arb.outcome(id), Some(&Ok(())), "status {st:?}");
    assert_eq!(wrong(&g, &s), Vec::new());
    assert_eq!(st.restocks, 1, "one trip covers the need");
    assert_eq!(w.stations[&station]["minecraft:stone"], 64, "nothing unneeded taken");
    assert!(w.stations[&station]["minecraft:oak_planks"] >= 200 - 64, "one stack for 16 planks");
}

#[test]
fn resumes_from_a_partly_built_world() {
    let g = Grid::floor(-20, 30);
    let s = house();
    for (p, t) in s.cells().filter(|(p, _)| p[1] == 0) {
        let pos = [ORIGIN[0] + p[0], ORIGIN[1] + p[1], ORIGIN[2] + p[2]];
        let name = t.name.trim_start_matches("minecraft:");
        g.set_state(pos, name, &t.props);
    }
    let mut w = World::new(&g, ITEMS);
    let (proc, status) = builder(&s, ORIGIN, &[]);
    let mut arb = Arbiter::new();
    let id = arb.add(proc, 10);
    run(&mut w, [0.5, 1.0, 0.5], &mut arb, id, 12_000);
    let st = status.lock().unwrap().clone();
    assert_eq!(arb.outcome(id), Some(&Ok(())), "status {st:?}");
    assert_eq!(wrong(&g, &s), Vec::new());
    let upper = s.cells().filter(|(p, t)| p[1] > 0 && !t.is_air()).count() as u32;
    assert_eq!(st.placed, upper, "placed only what was missing");
}

#[test]
fn supports_a_floating_block_and_cleans_up() {
    let g = Grid::floor(-20, 30);
    let mut s = Structure::new([1, 4, 1]);
    s.set([0, 3, 0], b("cobblestone", ""));
    let mut w = World::new(&g, &[("cobblestone", 4), ("dirt", 16)]);
    let (proc, status) = builder(&s, ORIGIN, &[]);
    let mut arb = Arbiter::new();
    let id = arb.add(proc, 10);
    run(&mut w, [0.5, 1.0, 0.5], &mut arb, id, 6000);
    let st = status.lock().unwrap().clone();
    assert_eq!(arb.outcome(id), Some(&Ok(())), "status {st:?}");
    assert_eq!(wrong(&g, &s), Vec::new());
    assert!(w.log.iter().any(|a| matches!(a, Act::Fill { .. })), "built a support");
    assert_eq!(scaffold_left(&g), Vec::<BlockPos>::new(),"support removed");
}

#[test]
fn seals_a_liquid_before_breaking_next_to_it() {
    let g = Grid::floor(-20, 30);
    let water = [ORIGIN[0] - 1, ORIGIN[1], ORIGIN[2]];
    g.set_state(water, "water", "liquid_depth=0");
    g.set_state(ORIGIN, "dirt", "");
    let mut s = Structure::new([1, 1, 1]);
    s.set([0, 0, 0], b("air", ""));
    let mut w = World::new(&g, &[("dirt", 4)]);
    let (proc, status) = builder(&s, ORIGIN, &[]);
    let mut arb = Arbiter::new();
    let id = arb.add(proc, 10);
    run(&mut w, [0.5, 1.0, 0.5], &mut arb, id, 3000);
    let st = status.lock().unwrap().clone();
    assert_eq!(arb.outcome(id), Some(&Ok(())), "status {st:?}");
    let seal = w.log.iter().position(|a| *a == Act::Fill { pos: water, blocks: vec!["minecraft:dirt".into()] });
    let dig = w.log.iter().position(|a| *a == Act::Break { pos: ORIGIN });
    assert!(seal.is_some() && seal < dig, "sealed first: {:?}", w.log);
    assert_eq!(view(&g, ORIGIN).0, "minecraft:air");
    assert_eq!(st.seals, vec![water], "the seal stays");
}
