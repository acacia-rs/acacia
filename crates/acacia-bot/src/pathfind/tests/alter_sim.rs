//! Headless execution of moves with work: the follower stops for it, the grid stands in for the
//! server, and the real movement simulation walks the result.

use super::alter::{blocks, door_in_wall, tool, walled, with};
use super::grid::Grid;
use super::search::ORIGIN;
use super::sim::{assert_at, run_with};
use crate::pathfind::{Goal, Kit};

#[test]
fn tunnels_through_a_wall() {
    let g = walled("stone");
    let r = run_with(&g, ORIGIN, Goal::Block([4, 1, 0]), 300, &with(true, false, tool("minecraft:iron_pickaxe")));
    assert_at(&r, [4.5, 1.0, 0.5], 0.3);
}

#[test]
fn bridges_a_gap() {
    let g = Grid::new();
    g.fill([-5, 0, -1], [0, 0, 1], "stone").fill([4, 0, -1], [8, 0, 1], "stone");
    let r = run_with(&g, ORIGIN, Goal::Block([6, 1, 0]), 600, &with(false, true, blocks(16)));
    assert_at(&r, [6.5, 1.0, 0.5], 0.3);
}

#[test]
fn pillars_up_a_cliff() {
    let g = Grid::floor(-10, 10);
    g.fill([1, 1, -10], [5, 4, 10], "stone");
    let r = run_with(&g, ORIGIN, Goal::Block([3, 5, 0]), 400, &with(false, true, blocks(16)));
    assert_at(&r, [3.5, 5.0, 0.5], 0.3);
}

#[test]
fn walks_through_a_door() {
    let g = door_in_wall("wooden_door");
    let r = run_with(&g, ORIGIN, Goal::Block([4, 1, 0]), 300, &with(false, false, Kit::default()));
    assert_at(&r, [4.5, 1.0, 0.5], 0.3);
}
