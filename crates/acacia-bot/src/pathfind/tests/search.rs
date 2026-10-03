use std::time::Duration;

use acacia_physics::{BlockPos, Vec3};

use super::grid::Grid;
use crate::pathfind::{Goal, MoveKind, Path, SearchOpts, search};

pub fn opts(parkour: bool) -> SearchOpts {
    SearchOpts { max_nodes: 50_000, budget: Duration::from_secs(5), parkour, ..SearchOpts::default() }
}

fn find(g: &Grid, from: Vec3, goal: BlockPos, parkour: bool) -> Path {
    search(&g.terrain(), from, &Goal::Block(goal), &opts(parkour))
}

pub fn kinds(p: &Path) -> Vec<MoveKind> {
    p.nodes.iter().map(|n| n.kind).collect()
}

pub const ORIGIN: Vec3 = [0.5, 1.0, 0.5];

#[test]
fn flat_walk_straight_and_diagonal() {
    let g = Grid::floor(-10, 10);
    let p = find(&g, ORIGIN, [5, 1, 0], true);
    assert!(p.complete);
    assert_eq!(p.nodes.len(), 5);
    assert!(kinds(&p).iter().all(|k| *k == MoveKind::Walk));
    let p = find(&g, ORIGIN, [4, 1, 4], true);
    assert_eq!(p.nodes.len(), 4, "diagonals: {:?}", p.nodes);
}

#[test]
fn staircase_up_needs_jumps_but_slabs_do_not() {
    let g = Grid::floor(-10, 10);
    for k in 1..=3 {
        g.fill([k, 1, -1], [k, k, 1], "stone");
    }
    let p = find(&g, ORIGIN, [3, 4, 0], true);
    assert!(p.complete, "{p:?}");
    assert_eq!(kinds(&p).iter().filter(|k| **k == MoveKind::Ascend).count(), 3, "{:?}", p.nodes);

    let g = Grid::floor(-10, 10);
    g.fill_state([1, 1, -1], [1, 1, 1], "smooth_stone_slab", "minecraft:vertical_half=bottom");
    g.fill([2, 1, -1], [2, 1, 1], "stone");
    let p = find(&g, ORIGIN, [2, 2, 0], true);
    assert!(p.complete);
    assert_eq!(p.nodes[0].feet, 1.5);
    assert!(kinds(&p).iter().all(|k| *k == MoveKind::Walk), "{:?}", p.nodes);
}

#[test]
fn drops_at_most_three_blocks_unless_into_water() {
    let g = Grid::floor(-10, 10);
    g.fill([-3, 1, -1], [0, 3, 1], "stone");
    let p = find(&g, [0.5, 4.0, 0.5], [4, 1, 0], true);
    assert!(p.complete);
    assert!(p.nodes.iter().any(|n| n.kind == MoveKind::Descend && n.pos[1] == 1), "{:?}", p.nodes);

    let g = Grid::floor(-10, 10);
    g.fill([-3, 1, -1], [0, 8, 1], "stone");
    let p = find(&g, [0.5, 9.0, 0.5], [4, 1, 0], true);
    assert!(!p.complete, "an 8-block drop is not safe");

    g.fill([1, 0, -1], [1, 0, 1], "air").fill([1, -2, -1], [1, -2, 1], "stone");
    g.fill_state([1, -1, -1], [1, 0, 1], "water", "liquid_depth=0");
    let p = find(&g, [0.5, 9.0, 0.5], [4, 1, 0], true);
    assert!(p.complete, "a water landing breaks the fall");
    assert!(p.nodes.iter().any(|n| n.kind == MoveKind::Descend && n.wet), "{:?}", p.nodes);
}

#[test]
fn detours_around_a_wall() {
    let g = Grid::floor(-10, 10);
    g.fill([2, 1, -3], [2, 3, 3], "stone");
    let p = find(&g, ORIGIN, [4, 1, 0], true);
    assert!(p.complete);
    assert!(p.nodes.iter().all(|n| n.pos[0] != 2 || n.pos[2].abs() > 3), "{:?}", p.nodes);
    assert!(p.nodes.len() > 4);
}

#[test]
fn crosses_water_by_swimming() {
    let g = Grid::floor(-10, 10);
    g.fill([2, -3, -10], [5, -3, 10], "stone").fill([2, 0, -10], [5, 0, 10], "air");
    g.fill_state([2, -2, -10], [5, 0, 10], "water", "liquid_depth=0");
    let p = find(&g, ORIGIN, [7, 1, 0], false);
    assert!(p.complete, "{p:?}");
    assert!(p.nodes.iter().any(|n| n.kind == MoveKind::Swim), "{:?}", p.nodes);
    assert_eq!(p.nodes.last().map(|n| n.pos), Some([7, 1, 0]));
}

#[test]
fn unreachable_goal_gives_partial_path_towards_it() {
    let g = Grid::floor(-10, 10);
    let p = find(&g, ORIGIN, [30, 1, 0], true);
    assert!(!p.complete);
    let end = p.nodes.last().expect("partial path").pos;
    assert_eq!(end[0], 10, "ends at the edge nearest the goal: {end:?}");
}

#[test]
fn avoids_lava() {
    let g = Grid::floor(-10, 10);
    g.fill_state([2, 0, -2], [4, 0, 2], "lava", "liquid_depth=0");
    let p = find(&g, ORIGIN, [6, 1, 0], false);
    assert!(p.complete);
    for n in &p.nodes {
        assert!(!((2..=4).contains(&n.pos[0]) && n.pos[2].abs() <= 3), "in or next to lava: {:?}", n.pos);
    }
}

#[test]
fn avoids_cobweb_and_magma() {
    let g = Grid::floor(-10, 10);
    g.fill([2, 1, -1], [2, 1, 1], "web").fill([2, 0, -3], [2, 0, -2], "magma").fill([2, 0, 2], [2, 0, 3], "magma");
    let p = find(&g, ORIGIN, [4, 1, 0], false);
    assert!(p.complete);
    assert!(p.nodes.iter().all(|n| n.pos[0] != 2 || n.pos[2].abs() > 3), "{:?}", p.nodes);
}

/// A ladder on the west face of a stone column at x = 1 (y 1..=5); floor on top at y = 5.
pub fn ladder_course(g: &Grid) {
    g.fill([1, 1, 0], [1, 5, 0], "stone").fill([1, 5, -1], [4, 5, 1], "stone");
    let ladder = g.registry.states_of("minecraft:ladder").find(|(_, s)| s.boxes.first().is_some_and(|b| b.min[0] > 0.5));
    let (id, _) = ladder.expect("a ladder state hanging on the +x face");
    for y in 1..=5 {
        g.view.set_block(0, y, 0, 0, id);
    }
}

#[test]
fn climbs_ladders() {
    let g = Grid::floor(-10, 10);
    ladder_course(&g);
    let p = find(&g, [-1.5, 1.0, 0.5], [3, 6, 0], false);
    assert!(p.complete, "{p:?}");
    assert!(kinds(&p).iter().filter(|k| **k == MoveKind::ClimbUp).count() >= 3, "{:?}", p.nodes);
}

#[test]
fn jumps_gaps() {
    let g = Grid::new();
    g.fill([-5, 0, -1], [0, 0, 1], "stone").fill([2, 0, -1], [3, 0, 1], "stone").fill([6, 0, -1], [8, 0, 1], "stone");
    let p = find(&g, ORIGIN, [7, 1, 0], true);
    assert!(p.complete, "{p:?}");
    assert_eq!(kinds(&p).iter().filter(|k| **k == MoveKind::Parkour).count(), 2, "{:?}", p.nodes);
    assert!(!find(&g, ORIGIN, [7, 1, 0], false).complete);
}
