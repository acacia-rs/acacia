//! Dead ends and failures (milestone C1): partial paths stop short of pits, a pit with no way out fails at once,
//! a bot at the edge of what it can see backs off instead of re-searching every tick, and re-plans favour the
//! path they replace.

use super::drive::{macro_opts, standing};
use super::grid::{Grid, search};
use super::pilot_sim::{Sky, fly};
use super::search::opts;
use crate::movement::Rollout;
use crate::pathfind::exit::Exits;
use crate::pathfind::{Favour, Goal, GotoOpts, Kit, MovementProfile, SearchOpts};

/// A lane (z -1..1) from x 0 to the pit's rim at 44, then a 3-deep pit (x 45..47) at the edge of the loaded grid.
fn lane_to_a_pit() -> Grid {
    let g = Grid::new();
    g.fill([0, 0, -1], [44, 0, 1], "stone").fill([45, -3, -1], [47, -3, 1], "stone");
    g
}

#[test]
fn partial_path_stops_at_the_rim_not_in_the_pit() {
    let g = lane_to_a_pit();
    for o in [opts(true), macro_opts(MovementProfile::Optimal)] {
        let p = search(&g.terrain(), [0.5, 1.0, 0.5], &Goal::XZ(60, 0), &o);
        let last = p.nodes.last().expect("a partial path");
        assert!(!p.complete && !p.unreachable, "unloaded terrain beyond: not proven unreachable");
        assert!(p.nodes.iter().all(|n| n.pos[1] == 1), "into the pit: {:?}", p.nodes);
        assert_eq!(last.pos[0], 44, "macros {}: stops on the rim", o.macros);
    }
}

/// At the rim with nothing visible beyond: the pilot backs off between searches and gives up when no closer.
#[test]
fn edge_of_the_world_backs_off_then_gives_up() {
    let g = lane_to_a_pit();
    let opts = GotoOpts { search: opts(true), give_up_ticks: Some(200), seed: Some(1), ..GotoOpts::default() };
    let f = fly(&g, &mut standing([0.5, 1.0, 0.5]), Goal::XZ(60, 0), opts, Sky { horizon: None, delay: 0, max_ticks: 1000 });
    let err = f.result.as_ref().expect_err("cannot get there");
    assert!(err.contains("no closer"), "{err}");
    assert!(f.track.iter().all(|p| p[1] > 0.99), "fell into the pit");
    assert!((f.end()[0] - 44.5).abs() < 0.5, "waits on the rim: {:?}", f.end());
    // ~200 ticks stalled on the rim; one search a tick before the backoff.
    assert!(f.searches <= 8, "{} searches", f.searches);
}

/// Leaps down are only queued into cells with a way on: a pit floor is closed (all its cells at once), the lane open,
/// and a pit holding the goal open.
#[test]
fn leap_landings_into_a_closed_pit_are_skipped() {
    let g = lane_to_a_pit();
    let (t, o) = (g.terrain(), macro_opts(MovementProfile::Optimal));
    let spot = |p| t.spot(p).expect("a floor");
    let mut exits = Exits::default();
    assert!(!exits.open::<Rollout, _>(&t, [46, -2, 0], spot([46, -2, 0]), &o, &Goal::XZ(60, 0)));
    assert!(!exits.open::<Rollout, _>(&t, [45, -2, 1], spot([45, -2, 1]), &o, &Goal::XZ(60, 0)), "the basin is closed as a whole");
    assert!(exits.open::<Rollout, _>(&t, [20, 1, 0], spot([20, 1, 0]), &o, &Goal::XZ(60, 0)));
    assert!(Exits::default().open::<Rollout, _>(&t, [46, -2, 0], spot([46, -2, 0]), &o, &Goal::Block([47, -2, 1])));
}

/// A closed 3-deep pit around the bot: every reachable cell searched.
fn pit() -> Grid {
    let g = Grid::floor(-20, 20);
    g.fill([-4, -4, -4], [4, -1, 4], "stone").fill([-1, -2, -1], [1, 0, 1], "air");
    g
}

#[test]
fn closed_pit_fails_at_once() {
    let g = pit();
    let opts = GotoOpts { search: opts(true), seed: Some(1), ..GotoOpts::default() };
    let f = fly(&g, &mut standing([0.5, -2.0, 0.5]), Goal::Block([10, 1, 0]), opts, Sky { horizon: None, delay: 0, max_ticks: 200 });
    let err = f.result.as_ref().expect_err("no way out");
    assert!(err.contains("every reachable cell"), "{err}");
    assert!(f.ticks <= 3 && f.searches == 1, "{} ticks, {} searches", f.ticks, f.searches);
}

#[test]
fn closed_pit_with_blocks_pillars_out() {
    let g = pit();
    let search = SearchOpts { allow_bridge: true, kit: Kit { blocks: 16, ..Kit::default() }, ..opts(true) };
    let opts = GotoOpts { search, seed: Some(1), ..GotoOpts::default() };
    let f = fly(&g, &mut standing([0.5, -2.0, 0.5]), Goal::Block([10, 1, 0]), opts, Sky { horizon: None, delay: 0, max_ticks: 600 });
    assert_eq!(f.result, Ok(()));
}

/// Around a wall two routes cost the same: a favoured one wins, and the path's cost is not discounted.
#[test]
fn replan_favours_the_old_path() {
    let g = Grid::floor(-20, 20);
    g.fill([5, 1, -2], [5, 2, 2], "stone");
    let side = |z: i32| Favour::new((0..=10).map(|x| [x, 1, z]));
    let mut costs = Vec::new();
    for z in [3, -3] {
        let o = SearchOpts { favour: Some(side(z).into()), ..opts(true) };
        let p = search(&g.terrain(), [0.5, 1.0, 0.5], &Goal::Block([10, 1, 0]), &o);
        assert!(p.complete);
        assert!(p.nodes.iter().filter(|n| n.pos[0] == 5).all(|n| n.pos[2] == z), "past the wall on z {z}: {:?}", p.nodes);
        costs.push(p.cost);
    }
    let plain = search(&g.terrain(), [0.5, 1.0, 0.5], &Goal::Block([10, 1, 0]), &opts(true));
    assert!(costs.iter().all(|c| (c - plain.cost).abs() < 0.01), "{costs:?} vs {}", plain.cost);
}
