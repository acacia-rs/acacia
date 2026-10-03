//! Moves that dig, build or open doors: which are offered and what they cost.

use acacia_physics::{BlockPos, Vec3};
use acacia_world::Tool;

use super::grid::Grid;
use super::search::{ORIGIN, kinds, opts};
use crate::pathfind::{Goal, Kit, MoveKind, Path, SearchOpts, Step, search};

pub fn with(dig: bool, bridge: bool, kit: Kit) -> SearchOpts {
    SearchOpts { allow_dig: dig, allow_bridge: bridge, kit, ..opts(false) }
}

pub fn tool(name: &str) -> Kit {
    Kit { tools: vec![(Tool::from_identifier(name).expect("a tool"), 0)], blocks: 0 }
}

pub fn blocks(n: u32) -> Kit {
    Kit { tools: Vec::new(), blocks: n }
}

fn find(g: &Grid, from: Vec3, goal: BlockPos, o: &SearchOpts) -> Path {
    search(&g.terrain(), from, &Goal::Block(goal), o)
}

fn steps(p: &Path) -> Vec<Step> {
    p.nodes.iter().flat_map(|n| n.work.steps().collect::<Vec<_>>()).collect()
}

fn broken(p: &Path) -> Vec<BlockPos> {
    steps(p).into_iter().filter_map(|s| if let Step::Break(b) = s { Some(b) } else { None }).collect()
}

fn placed(p: &Path) -> usize {
    steps(p).iter().filter(|s| matches!(s, Step::Place { .. })).count()
}

/// Stone floor with a wall at x = 2 across it, of `block`, 3 high.
pub fn walled(block: &str) -> Grid {
    let g = Grid::floor(-10, 10);
    g.fill([2, 1, -10], [2, 3, 10], block);
    g
}

#[test]
fn digs_through_a_wall_only_when_allowed() {
    let g = walled("stone");
    assert!(!find(&g, ORIGIN, [4, 1, 0], &with(false, false, Kit::default())).complete);
    let hand = find(&g, ORIGIN, [4, 1, 0], &with(true, false, Kit::default()));
    assert!(hand.complete, "{hand:?}");
    let dug = broken(&hand);
    assert!(dug.len() == 2 && dug.iter().all(|b| b[0] == 2), "{dug:?}");
    let pick = find(&g, ORIGIN, [4, 1, 0], &with(true, false, tool("minecraft:diamond_pickaxe")));
    assert!(pick.complete);
    assert!(hand.cost > 300.0 && pick.cost < 60.0, "by hand {} ticks, diamond pickaxe {}", hand.cost, pick.cost);
}

#[test]
fn digs_down_and_up_a_step() {
    let g = Grid::floor(-10, 10);
    g.fill([-10, -3, -10], [10, -1, 10], "dirt");
    let p = find(&g, ORIGIN, [0, -1, 0], &with(true, false, Kit::default()));
    assert!(p.complete, "{p:?}");
    assert_eq!(kinds(&p), vec![MoveKind::Down, MoveKind::Down], "{:?}", p.nodes);

    let g = Grid::floor(-10, 10);
    g.fill([1, 1, -10], [10, 3, 10], "dirt");
    let p = find(&g, ORIGIN, [1, 2, 0], &with(true, false, Kit::default()));
    assert!(p.complete, "{p:?}");
    assert_eq!(p.nodes.last().map(|n| (n.kind, n.work.breaks())), Some((MoveKind::Ascend, 2)), "{:?}", p.nodes);
}

#[test]
fn never_digs_beside_liquid() {
    for liquid in ["water", "lava"] {
        let g = walled("stone");
        g.fill_state([3, 1, -10], [3, 3, 10], liquid, "liquid_depth=0");
        let p = find(&g, ORIGIN, [5, 1, 0], &with(true, false, Kit::default()));
        assert!(!p.complete, "{liquid}: {:?}", p.nodes);
        assert!(broken(&p).iter().all(|b| b[0] != 2), "{liquid}: {:?}", broken(&p));
    }
}

#[test]
fn never_digs_unbreakable_or_under_falling_blocks() {
    let g = walled("bedrock");
    assert!(!find(&g, ORIGIN, [4, 1, 0], &with(true, false, tool("minecraft:netherite_pickaxe"))).complete);
    let g = Grid::floor(-10, 10);
    g.fill([2, 1, -10], [2, 2, 10], "stone").fill([2, 3, -10], [2, 3, 10], "gravel");
    assert!(!find(&g, ORIGIN, [4, 1, 0], &with(true, false, Kit::default())).complete);
}

#[test]
fn bridges_gaps_with_enough_blocks() {
    let g = Grid::new();
    g.fill([-5, 0, -1], [0, 0, 1], "stone").fill([4, 0, -1], [8, 0, 1], "stone");
    let goal = [6, 1, 0];
    assert!(!find(&g, ORIGIN, goal, &with(false, false, blocks(64))).complete);
    let p = find(&g, ORIGIN, goal, &with(false, true, blocks(64)));
    assert!(p.complete, "{p:?}");
    assert_eq!(placed(&p), 3, "{:?}", p.nodes);
    assert!(!find(&g, ORIGIN, goal, &with(false, true, blocks(2))).complete, "three blocks are needed");
}

#[test]
fn pillars_up_and_scarce_blocks_cost_more() {
    let g = Grid::floor(-10, 10);
    g.fill([1, 1, -10], [5, 4, 10], "stone");
    let p = find(&g, ORIGIN, [3, 5, 0], &with(false, true, blocks(64)));
    assert!(p.complete, "{p:?}");
    assert_eq!(kinds(&p).iter().filter(|k| **k == MoveKind::Pillar).count(), 3, "{:?}", p.nodes);
    let scarce = find(&g, ORIGIN, [3, 5, 0], &with(false, true, blocks(3)));
    assert!(scarce.complete && scarce.cost > p.cost, "{} vs {}", scarce.cost, p.cost);
}

/// [`walled`] stone with a closed `name` (door or gate) at (2, 1, 0) facing along x.
pub fn door_in_wall(name: &str) -> Grid {
    let g = walled("stone");
    if name.ends_with("door") {
        let props = "minecraft:cardinal_direction=south,open_bit=0,door_hinge_bit=0";
        g.set_state([2, 1, 0], name, &format!("{props},upper_block_bit=0"));
        g.set_state([2, 2, 0], name, &format!("{props},upper_block_bit=1"));
    } else {
        g.set_state([2, 1, 0], name, "minecraft:cardinal_direction=west,open_bit=0").set_state([2, 2, 0], "air", "");
    }
    g
}

#[test]
fn opens_wooden_doors_and_gates_but_not_iron_doors() {
    for name in ["wooden_door", "fence_gate"] {
        let g = door_in_wall(name);
        let p = find(&g, ORIGIN, [4, 1, 0], &with(false, false, Kit::default()));
        assert!(p.complete, "{name}: {p:?}");
        assert_eq!(steps(&p), vec![Step::Door { pos: [2, 1, 0], open: true }], "{name}");
        let closed = SearchOpts { open_doors: false, ..with(false, false, Kit::default()) };
        assert!(!find(&g, ORIGIN, [4, 1, 0], &closed).complete, "{name} with doors off");
    }
    let g = door_in_wall("iron_door");
    assert!(!find(&g, ORIGIN, [4, 1, 0], &with(false, false, Kit::default())).complete);
}
