//! Boat legs in the search (design step 6): a lake crossing beats walking round or swimming, the heuristic stays
//! under the edge, and no boat is planned without one to board.

use super::drive::macro_opts;
use super::grid::Grid;
use super::search::{ORIGIN, kinds, opts};
use super::sim::{assert_at, run_with};
use crate::pathfind::boat::BoatSource;
use crate::pathfind::{Goal, MoveKind, MovementProfile, Path, SearchOpts, search};

const FAR_BANK: [i32; 3] = [50, 1, 0];

/// Stone to y 0 (feet at 1) and a lake two deep (y -1 and 0) over x 5..=44, z -40..=40: round it is ~140 blocks.
fn lake() -> Grid {
    let g = Grid::sized(3);
    g.fill([-48, -2, -48], [63, 0, 63], "stone");
    g.fill_state([5, -1, -40], [44, 0, 40], "water", "liquid_depth=0");
    g
}

fn with_boats(source: BoatSource) -> SearchOpts {
    SearchOpts { boats: true, boat_source: source, ..opts(true) }
}

fn carried() -> BoatSource {
    BoatSource { carried: true, seen: Vec::new() }
}

fn boat_nodes(p: &Path) -> Vec<[i32; 3]> {
    p.nodes.iter().filter(|n| n.kind == MoveKind::Boat).map(|n| n.pos).collect()
}

#[test]
fn a_lake_crossing_takes_the_boat() {
    let g = lake();
    let goal = Goal::Block(FAR_BANK);
    let p = search(&g.terrain(), ORIGIN, &goal, &with_boats(carried()));
    let legs = boat_nodes(&p);
    assert!(p.complete && legs.len() == 1 && legs[0][0] >= 45, "{legs:?} in {:?}", kinds(&p));
    let without = search(&g.terrain(), ORIGIN, &goal, &opts(true));
    assert!(without.complete && boat_nodes(&without).is_empty());
    assert!(p.cost < without.cost, "boat {} vs {}", p.cost, without.cost);
}

#[test]
fn macro_searches_take_the_boat_too() {
    let g = lake();
    for profile in [MovementProfile::Optimal, MovementProfile::Human] {
        // About 20k nodes either profile (planning the walk round took 42k-73k).
        let o = SearchOpts { boats: true, boat_source: carried(), max_nodes: 50_000, ..macro_opts(profile) };
        let p = search(&g.terrain(), [-11.5, 1.0, 0.5], &Goal::Block(FAR_BANK), &o);
        assert!(p.complete && boat_nodes(&p).len() == 1, "{profile:?}: {:?} after {} nodes", kinds(&p), p.visited);
    }
}

#[test]
fn the_heuristic_stays_under_the_boat_edge() {
    let g = lake();
    let o = with_boats(carried());
    let p = search(&g.terrain(), ORIGIN, &Goal::Block(FAR_BANK), &o);
    let i = p.nodes.iter().position(|n| n.kind == MoveKind::Boat).expect("a boat leg");
    let from = if i == 0 { [0, 1, 0] } else { p.nodes[i - 1].pos };
    let leg = &p.nodes[i];
    let floor = Goal::Block(leg.pos).estimate(from, o.speed(), true);
    assert!(leg.cost >= floor, "edge {} under the heuristic's {floor}", leg.cost);
    assert!(o.speed().per_block < opts(true).speed().per_block);
}

#[test]
fn no_boat_is_planned_without_one_to_board() {
    let g = lake();
    // Nothing carried, and the one boat seen floats out in the lake.
    for source in [BoatSource::default(), BoatSource { carried: false, seen: vec![[30.5, 0.96, 20.5]] }] {
        let o = with_boats(source.clone());
        let p = search(&g.terrain(), ORIGIN, &Goal::Block(FAR_BANK), &o);
        assert!(boat_nodes(&p).is_empty(), "{source:?}: {:?}", kinds(&p));
        if !source.any() {
            assert_eq!(o.speed(), opts(true).speed());
        }
    }
}

#[test]
fn a_boat_floating_by_the_bank_is_boarded() {
    let g = lake();
    let o = with_boats(BoatSource { carried: false, seen: vec![[6.5, 0.96, 0.5]] });
    let p = search(&g.terrain(), ORIGIN, &Goal::Block(FAR_BANK), &o);
    let i = p.nodes.iter().position(|n| n.kind == MoveKind::Boat).unwrap_or_else(|| panic!("{:?}", kinds(&p)));
    let from = if i == 0 { [0, 1, 0] } else { p.nodes[i - 1].pos };
    assert!((from[0] as f32 + 0.5 - 6.5).hypot(from[2] as f32) <= 2.6, "boarded from {from:?}");
}

#[test]
fn a_pond_is_walked_round() {
    let g = Grid::sized(2);
    g.fill([-32, -2, -32], [47, 0, 47], "stone");
    g.fill_state([5, -1, -3], [10, 0, 3], "water", "liquid_depth=0");
    let p = search(&g.terrain(), ORIGIN, &Goal::Block([14, 1, 0]), &with_boats(carried()));
    assert!(p.complete && boat_nodes(&p).is_empty(), "{:?}", kinds(&p));
}

#[test]
fn the_follower_stops_for_the_boat_and_arrives() {
    let g = lake();
    let r = run_with(&g, ORIGIN, Goal::Block(FAR_BANK), 1200, &with_boats(carried()));
    assert_at(&r, [50.5, 1.0, 0.5], 0.5);
}
