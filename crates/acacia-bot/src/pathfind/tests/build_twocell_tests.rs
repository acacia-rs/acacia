//! Doors, beds and tall plants built in the headless simulation (`place_twocell.rs` as the server).

use acacia_physics::BlockPos;

use super::build_sim::{builder, explain, run, view, World};
use super::build_tests::{scaffold_left, wrong, ORIGIN};
use super::grid::Grid;
use crate::build::{Block, Structure};
use crate::pathfind::process::{Act, Arbiter};

fn door(facing: &str, hinge: u8, upper: u8) -> Option<Block> {
    Some(Block::new("wooden_door", &format!("door_hinge_bit={hinge},minecraft:cardinal_direction={facing},open_bit=0,upper_block_bit={upper}")))
}

/// 5×3×5: a plank floor (grass under the sunflower) with a wall along z 0 holding a double door, a bed
/// facing east and a sunflower inside.
fn room() -> Structure {
    let mut s = Structure::new([5, 3, 5]);
    for x in 0..5 {
        for z in 0..5 {
            s.set([x, 0, z], Some(Block::new("oak_planks", "")));
            for y in 1..3 {
                s.set([x, y, z], Some(Block::new(if z == 0 { "cobblestone" } else { "air" }, "")));
            }
        }
    }
    s.set([3, 0, 3], Some(Block::new("grass_block", "")));
    // Placed looking north from inside: facing east, west is left. The right door has a door on its left.
    s.set([1, 1, 0], door("east", 0, 0));
    s.set([1, 2, 0], door("east", 0, 1));
    s.set([2, 1, 0], door("east", 0, 0));
    s.set([2, 2, 0], door("east", 1, 1));
    s.set([1, 1, 2], Some(Block::new("bed", "direction=3,head_piece_bit=0,occupied_bit=0")));
    s.set([2, 1, 2], Some(Block::new("bed", "direction=3,head_piece_bit=1,occupied_bit=0")));
    s.set([3, 1, 3], Some(Block::new("sunflower", "upper_block_bit=0")));
    s.set([3, 2, 3], Some(Block::new("sunflower", "upper_block_bit=1")));
    s
}

const ITEMS: &[(&str, u32)] = &[("oak_planks", 64), ("cobblestone", 64), ("grass_block", 4), ("wooden_door", 4), ("bed", 2), ("sunflower", 2), ("dirt", 64)];

#[test]
fn builds_doors_a_bed_and_a_sunflower_block_exact() {
    let g = Grid::floor(-20, 30);
    let at = |p: BlockPos| [ORIGIN[0] + p[0], ORIGIN[1] + p[1], ORIGIN[2] + p[2]];
    // Junk where the left door's upper half goes, and a door with the wrong hinge where the right one goes.
    g.set_state(at([1, 2, 0]), "stone", "");
    for (y, hinge, upper) in [(1, 0, 0), (2, 0, 1)] {
        g.set_state(at([2, y, 0]), "wooden_door", &format!("door_hinge_bit={hinge},minecraft:cardinal_direction=east,open_bit=0,upper_block_bit={upper}"));
    }
    let s = room();
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
    assert_eq!(scaffold_left(&g), Vec::<BlockPos>::new(), "scaffold removed");
    let places = |item: &str| w.log.iter().filter(|a| matches!(a, Act::Place { item: i, .. } if i.ends_with(item))).count();
    assert_eq!((places("wooden_door"), places(":bed"), places("sunflower")), (2, 1, 1), "one placement per pair");
    assert_eq!(view(&g, at([2, 2, 0])).1, "door_hinge_bit=1,minecraft:cardinal_direction=east,open_bit=0,upper_block_bit=1");
}

/// 8×3×2 with a wall row at z 0: door A at x1 (hinge 0) with cobblestone right of it at x2-3, which built first
/// would flip it to 1; door B at x5 (hinge 1) that needs the cobblestone right of it at x6 first.
fn hinge_wall() -> Structure {
    let mut s = Structure::new([8, 3, 2]);
    for x in 0..8 {
        for z in 0..2 {
            s.set([x, 0, z], Some(Block::new("oak_planks", "")));
            for y in 1..3 {
                let wall = z == 0 && matches!(x, 2 | 3 | 6);
                s.set([x, y, z], Some(Block::new(if wall { "cobblestone" } else { "air" }, "")));
            }
        }
    }
    for (x, hinge) in [(1, 0), (5, 1)] {
        s.set([x, 1, 0], door("east", 0, 0));
        s.set([x, 2, 0], door("east", hinge, 1));
    }
    s
}

#[test]
fn a_door_goes_in_before_the_wall_that_would_flip_its_hinge() {
    let g = Grid::floor(-20, 30);
    let at = |p: BlockPos| [ORIGIN[0] + p[0], ORIGIN[1] + p[1], ORIGIN[2] + p[2]];
    let s = hinge_wall();
    let mut w = World::new(&g, &[("oak_planks", 64), ("cobblestone", 64), ("wooden_door", 4), ("dirt", 64)]);
    let (proc, status) = builder(&s, ORIGIN, &[]);
    let mut arb = Arbiter::new();
    let id = arb.add(proc, 10);
    let ticks = run(&mut w, [0.5, 1.0, 3.5], &mut arb, id, 12_000);
    let st = status.lock().unwrap().clone();
    eprintln!("{ticks} ticks, {st:?}");
    explain(&w, &s, ORIGIN).iter().for_each(|l| eprintln!("OPEN {l}"));
    assert_eq!(arb.outcome(id), Some(&Ok(())), "status {st:?}");
    assert_eq!(wrong(&g, &s), Vec::new());
    let placed = |p: BlockPos| w.log.iter().position(|a| matches!(a, Act::Place { against, face, .. } if face.adjacent(*against) == at(p)));
    let door_a = placed([1, 1, 0]).expect("door A placed");
    assert!(placed([2, 1, 0]).expect("wall placed") > door_a, "door A before the wall right of it");
    let door_b = placed([5, 1, 0]).expect("door B placed");
    assert!(placed([6, 1, 0]).expect("wall placed") < door_b, "door B after the wall it needs");
    assert_eq!(w.log.iter().filter(|a| matches!(a, Act::Break { .. })).count(), 0, "nothing broken and redone");
}

#[test]
fn a_bed_takes_the_item_of_its_colour() {
    let g = Grid::floor(-20, 30);
    let station = [0, 1, 6];
    g.set_state(station, "barrel", "");
    let mut s = Structure::new([2, 2, 1]);
    s.set([0, 0, 0], Some(Block::new("oak_planks", "")));
    s.set([1, 0, 0], Some(Block::new("oak_planks", "")));
    s.set([0, 1, 0], Some(Block::new("bed", "bed_color=14,direction=3,head_piece_bit=0,occupied_bit=0")));
    s.set([1, 1, 0], Some(Block::new("bed", "bed_color=14,direction=3,head_piece_bit=1,occupied_bit=0")));
    let mut w = World::new(&g, &[("oak_planks", 4), ("bed@3", 1), ("dirt", 16)]);
    w.stations.insert(station, [("minecraft:bed@3".to_string(), 1), ("minecraft:bed@14".to_string(), 1)].into());
    let (proc, status) = builder(&s, ORIGIN, &[station]);
    let mut arb = Arbiter::new();
    let id = arb.add(proc, 10);
    run(&mut w, [0.5, 1.0, 3.5], &mut arb, id, 6000);
    let st = status.lock().unwrap().clone();
    assert_eq!(arb.outcome(id), Some(&Ok(())), "status {st:?}");
    assert_eq!(wrong(&g, &s), Vec::new());
    assert_eq!((w.inv["minecraft:bed@3"], w.stations[&station]["minecraft:bed@3"]), (1, 1), "the white bed held is not used or fetched");
    assert_eq!(w.stations[&station]["minecraft:bed@14"], 0, "the red bed fetched");
    assert!(w.log.iter().any(|a| matches!(a, Act::Place { item, .. } if item == "minecraft:bed@14")));
}
