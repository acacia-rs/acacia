//! The interact tier in the simulation (`interact_model.rs` as the server): use-clicks, counts, signs.

use super::build_sim::{builder, explain, run, World};
use super::build_tests::{scaffold_left, wrong, ORIGIN};
use super::grid::Grid;
use crate::build::{Block, SignText, Structure};
use crate::pathfind::process::{Act, Arbiter};

fn b(name: &str, props: &str) -> Option<Block> {
    Some(Block::new(name, props))
}

/// A plank floor with a row of use-toggled blocks, a row of counted blocks and signs.
fn board() -> Structure {
    let mut s = Structure::new([7, 2, 5]);
    for x in 0..7 {
        for z in 0..5 {
            s.set([x, 0, z], b("oak_planks", ""));
        }
    }
    s.set([0, 1, 2], b("unpowered_repeater", "minecraft:cardinal_direction=east,repeater_delay=2"));
    s.set([2, 1, 2], b("unpowered_comparator", "minecraft:cardinal_direction=north,output_lit_bit=0,output_subtract_bit=1"));
    s.set([4, 1, 2], b("lever", "lever_direction=up_north_south,open_bit=1"));
    s.set([6, 1, 2], b("daylight_detector_inverted", "redstone_signal=0"));
    s.set([0, 1, 4], b("white_candle", "candles=2,lit=0"));
    s.set([2, 1, 4], b("sea_pickle", "cluster_count=2,dead_bit=1"));
    s.set([4, 1, 4], b("turtle_egg", "cracked_state=no_cracks,turtle_egg_count=three_egg"));
    s.set([6, 1, 4], b("standing_sign", "ground_sign_direction=6"));
    s.set_text([6, 1, 4], Some(SignText { front: "front\nline".into(), back: "back".into() }));
    s.set([3, 1, 0], b("cobblestone", ""));
    s.set([3, 1, 1], b("wall_sign", "facing_direction=3"));
    s.set_text([3, 1, 1], Some(SignText { front: "wall".into(), back: String::new() }));
    s.set([5, 1, 0], b("cobblestone", ""));
    s.set([6, 1, 0], b("oak_hanging_sign", "attached_bit=0,facing_direction=3,ground_sign_direction=0,hanging=0"));
    s
}

const ITEMS: &[(&str, u32)] = &[
    ("oak_planks", 64), ("cobblestone", 8), ("repeater", 2), ("comparator", 2), ("lever", 2), ("daylight_detector", 2), ("white_candle", 8), ("sea_pickle", 8),
    ("turtle_egg", 8), ("oak_sign", 4), ("oak_hanging_sign", 2), ("dirt", 64),
];

#[test]
fn uses_counts_and_signs_build_block_exact() {
    let g = Grid::floor(-20, 30);
    // Already there with the wrong toggles: used, not broken.
    let at = |p: [i32; 3]| [ORIGIN[0] + p[0], ORIGIN[1] + p[1], ORIGIN[2] + p[2]];
    g.set_state(at([4, 0, 2]), "oak_planks", "");
    g.set_state(at([4, 1, 2]), "lever", "lever_direction=up_north_south,open_bit=0");
    g.set_state(at([0, 0, 2]), "oak_planks", "");
    g.set_state(at([0, 1, 2]), "unpowered_repeater", "minecraft:cardinal_direction=east,repeater_delay=3");
    let s = board();
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
    assert_eq!(st.broken, 0, "toggles are used, not broken: {st:?}");
    let uses = |p: [i32; 3]| w.log.iter().filter(|a| matches!(a, Act::Use { pos, .. } if *pos == at(p))).count();
    assert_eq!((uses([0, 1, 2]), uses([4, 1, 2]), uses([2, 1, 4]), uses([4, 1, 4])), (3, 1, 2, 2), "delay 3 -> 2 is three clicks; pickles and eggs by use");
    assert_eq!(w.signs.get(&(at([6, 1, 4]), true)).map(String::as_str), Some("front\nline"));
    assert_eq!(w.signs.get(&(at([6, 1, 4]), false)).map(String::as_str), Some("back"));
    assert_eq!(w.signs.get(&(at([3, 1, 1]), true)).map(String::as_str), Some("wall"));
    assert_eq!(scaffold_left(&g), Vec::<[i32; 3]>::new(), "scaffold removed");
}
