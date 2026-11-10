//! Any-angle shortcuts while following (plan step 6b): straight lines instead of the 8-direction staircase.

use super::drive::{PROFILES, Style, assert_arrived, drive, macro_opts, plan, plan_and_follow, seeds, standing};
use super::grid::Grid;
use crate::pathfind::shortcut::over_floor;
use crate::movement::{Rollout, Sim};
use crate::pathfind::{Goal, MovementProfile, SearchOpts};

/// Ticks to [15, 1, 6] under every execution seed.
fn follow(g: &Grid, opts: &SearchOpts, shortcuts: bool) -> Vec<u32> {
    let (_, runs) = plan_and_follow(g, [-15.5, 1.0, -5.5], [15, 1, 6], opts, shortcuts).expect("plan");
    runs.iter().map(|o| o.ticks).collect()
}

/// 30 blocks east and 11 north: the plan staircases, the shortcut runs the line.
#[test]
fn shortcuts_cut_a_long_diagonal() {
    let g = Grid::floor(-31, 46);
    for profile in PROFILES {
        for macros in [false, true] {
            let opts = SearchOpts { macros, ..macro_opts(profile) };
            let (without, with) = (follow(&g, &opts, false), follow(&g, &opts, true));
            eprintln!("{profile:?}, macros {macros}: {with:?} ticks with shortcuts, {without:?} without");
            assert!(with.iter().zip(&without).all(|(w, o)| w < o), "{profile:?}, macros {macros}: {with:?} vs {without:?}");
        }
    }
}

/// Round a pit's corner (the course's sprint40 past the diag pit): a `Human` shortcut keeps its centre over the
/// floor on every grounded tick, as its eased aim drifts off a line that only grazes the edge.
#[test]
fn human_shortcuts_keep_off_pit_edges() {
    let g = Grid::floor(-31, 46);
    g.fill([0, -3, -12], [5, 0, 3], "air");
    let opts = macro_opts(MovementProfile::Human);
    for from in [[8.5, 1.0, 1.5], [9.5, 1.0, 2.5], [10.5, 1.0, 0.5]] {
        let goal = Goal::Block([-6, 1, 4]);
        let path = plan(&g, &standing(from), &goal, &opts);
        for &seed in seeds(opts.profile) {
            let mut m = standing(from);
            let mut over_air = Vec::new();
            let o = drive(&g, &mut m, path.nodes.clone(), &goal, &opts, Style { seed, shortcuts: true }, |t, m| {
                let st = m.state().expect("started");
                if st.on_ground && !over_floor(&g.physics(), &Rollout::new(st.clone()).body()) {
                    over_air.push((t, st.pos));
                }
            });
            assert_arrived(&o, [-6, 1, 4]);
            assert!(over_air.is_empty(), "from {from:?} seed {seed}: centre over the pit {over_air:?}");
        }
    }
}

/// A wall across the line: no shortcut through it, the path around still arrives.
#[test]
fn shortcuts_keep_out_of_walls() {
    let g = Grid::floor(-31, 46);
    g.fill([0, 1, -12], [0, 3, 3], "stone");
    for profile in PROFILES {
        follow(&g, &macro_opts(profile), true);
    }
}
