//! The oneclick tier's blocks built in the headless simulation (see `build_sim.rs`, `place_oneclick.rs`).

use super::build_sim::{builder, explain, run, World};
use super::build_tests::{scaffold_left, wrong, ORIGIN};
use super::grid::Grid;
use crate::build::{Block, Structure};
use crate::pathfind::process::Arbiter;

/// 5×5×3: a plank floor, a cobblestone back row, trapdoors (one high on a pillar), two pillars under
/// a ceiling with a hanging lantern and a chandelier (a chain with a lantern under it), and floor blocks in front.
fn scene() -> Structure {
    let mut s = Structure::new([5, 5, 3]);
    let mut put = |p: [i32; 3], name: &str, props: &str| s.set(p, Some(Block::new(name, props)));
    for x in 0..5 {
        for z in 0..3 {
            put([x, 0, z], "oak_planks", "");
            (1..5).for_each(|y| put([x, y, z], "air", ""));
        }
        put([x, 1, 0], "cobblestone", "");
        put([x, 4, 1], "cobblestone", "");
    }
    (1..4).for_each(|y| {
        put([0, y, 1], "cobblestone", "");
        put([4, y, 1], "cobblestone", "");
    });
    // The chandelier's lantern hangs above eye height off the floor: its chain's underside must be in view.
    put([1, 3, 1], "lantern", "hanging=1");
    put([3, 3, 1], "iron_chain", "pillar_axis=y");
    put([3, 2, 1], "soul_lantern", "hanging=1");
    put([2, 3, 1], "stone_button", "button_pressed_bit=0,facing_direction=0");
    put([0, 2, 2], "trapdoor", "direction=2,open_bit=0,upside_down_bit=1");
    put([1, 1, 2], "trapdoor", "direction=3,open_bit=0,upside_down_bit=0");
    put([2, 1, 2], "fence_gate", "in_wall_bit=0,minecraft:cardinal_direction=south,open_bit=0");
    put([3, 1, 2], "chest", "minecraft:cardinal_direction=south");
    put([4, 1, 2], "chest", "minecraft:cardinal_direction=south");
    put([0, 1, 2], "hopper", "facing_direction=0,toggle_bit=0");
    put([2, 1, 1], "end_rod", "facing_direction=1");
    s
}

const ITEMS: &[(&str, u32)] = &[
    ("oak_planks", 64), ("cobblestone", 64), ("lantern", 4), ("soul_lantern", 4), ("iron_chain", 4), ("trapdoor", 4), ("fence_gate", 2), ("chest", 4),
    ("hopper", 2), ("end_rod", 2), ("stone_button", 2), ("dirt", 64),
];

#[test]
fn builds_the_oneclick_blocks_block_exact() {
    let g = Grid::floor(-20, 30);
    let s = scene();
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
    assert!(scaffold_left(&g).is_empty(), "scaffold removed");
}
