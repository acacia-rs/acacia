//! `Evade` over `Goto` in the arbiter among simulated mobs: a lit creeper is run from, then the goto resumes.

use acacia_physics::constants::DEFAULT_PLAYER_HEIGHT_OFFSET;

use super::drive::standing;
use super::entity_scenarios::goto_opts;
use super::entity_sim::{Brain, Crowd, Harm, Mob};
use super::grid::Grid;
use super::process_sim::SimLook;
use crate::pathfind::hazard::{self, View};
use crate::pathfind::process::{Arbiter, Decision};
use crate::pathfind::{Evade, Goal, Goto, MovementProfile, NavStatus, SearchJob, SearchOpts, answered};

pub struct Outcome {
    pub arrived: bool,
    pub harm: Harm,
    pub evading: Vec<bool>,
}

/// Drives a goto to `goal` with an evade above it, among `mobs`.
pub fn run(g: &Grid, mobs: Vec<Mob>, goal: Goal, profile: MovementProfile, seed: u64, max_ticks: u32) -> Outcome {
    let world = g.physics();
    let mut m = standing([0.5, 1.0, 0.5]);
    let mut crowd = Crowd::new(g, mobs);
    let mut arb = Arbiter::new();
    let goto = arb.add(Goto::with(goal, goto_opts(profile, seed)), 10);
    let evade = arb.add(Evade::new(goto_opts(profile, seed)), 90);
    let mut evading = Vec::new();
    // EVADE_TRACE=1: per-tick position and decision.
    let trace = std::env::var_os("EVADE_TRACE").is_some();
    for tick in 0..max_ticks {
        let pos = m.position().expect("started");
        let (threats, health) = crowd.step(tick, pos);
        m.set_solids(hazard::solid_boxes(&threats));
        let sees = |a, b| hazard::clear_line(&g.terrain(), a, b);
        let eye = [pos[0], pos[1] + DEFAULT_PLAYER_HEIGHT_OFFSET, pos[2]];
        let view = View { eye, yaw: m.state().map_or(0.0, |s| s.yaw), health, sees: &sees };
        // As `Processes::tick`: the field is refreshed before deciding, driven or not.
        if let Some(p) = arb.pilot_mut() {
            p.perceive(threats.clone(), &view);
        }
        let decision = arb.decide(pos, m.on_ground(), &SimLook { g, target: None });
        evading.push(arb.active() == Some(evade));
        if trace {
            eprintln!("TRACE {tick} {pos:.2?} {decision:?} evading {} yaw {:?}", evading[tick as usize], m.state().map(|s| s.yaw));
        }
        match decision {
            Decision::Go => {
                let pilot = arb.pilot_mut().expect("routed");
                let hazards = pilot.perceive(threats.clone(), &view);
                let terrain = g.terrain().with_hazards(hazards.clone());
                let opts = SearchOpts { hazards, ..pilot.search_opts(None, 20.0) };
                let spawn = &mut |job: SearchJob| answered(job.run(&g.terrain().with_hazards(job.opts.hazards.clone())));
                let result = pilot.drive(&terrain, &mut m, opts, spawn);
                if !matches!(result, Ok(NavStatus::Searching | NavStatus::Moving)) {
                    m.controls.stop();
                }
                arb.drove(result, Vec::new());
            }
            _ => m.controls.stop(),
        }
        if arb.outcome(goto).is_some() {
            break;
        }
        m.tick(&world);
    }
    let arrived = arb.outcome(goto) == Some(&Ok(()));
    eprintln!("EVADE {profile:?} seed {seed}: arrived {arrived} after {} ticks, harm {:?}, evading {} ticks", evading.len(), crowd.harm, evading.iter().filter(|e| **e).count());
    Outcome { arrived, harm: crowd.harm, evading }
}

const PROFILES: [MovementProfile; 2] = [MovementProfile::Optimal, MovementProfile::Human];

#[test]
fn runs_from_a_creeper_lit_behind_it() {
    let g = Grid::floor(-30, 46);
    for profile in PROFILES {
        let o = run(&g, vec![Mob::new("creeper", [-1.5, 1.0, 1.5], Brain::Chase)], Goal::Block([30, 1, 0]), profile, 1, 1500);
        assert!(o.arrived, "{profile:?}");
        assert_eq!(o.harm.blast, 0, "{profile:?}");
    }
}

/// Lit beside the way ahead (`Human` notices it late: one lit 2.5 blocks dead ahead of a bot sprinting off at it
/// blows up in its face, as it would a person's).
#[test]
fn backs_off_a_creeper_lit_beside_the_way_then_goes_round() {
    let g = Grid::floor(-30, 46);
    for profile in PROFILES {
        let o = run(&g, vec![Mob::new("creeper", [3.5, 1.0, 2.5], Brain::Chase).lit()], Goal::Block([30, 1, 0]), profile, 2, 2000);
        assert!(o.evading.iter().any(|e| *e), "{profile:?}: evaded");
        assert!(o.arrived, "{profile:?}");
        assert_eq!(o.harm.blast, 0, "{profile:?}");
    }
}

/// A creeper lit where it stands (live: `minecraft:start_exploding_forced`) that blows and is gone: the evade must
/// hand back once it has, not hold where it ran to.
#[test]
fn goes_on_once_a_creeper_lit_where_it_stands_has_blown() {
    let g = Grid::floor(-30, 46);
    for profile in PROFILES {
        let o = run(&g, vec![Mob::new("creeper", [4.5, 1.0, 2.5], Brain::Still).lit()], Goal::Block([30, 1, 0]), profile, 3, 1000);
        assert!(o.evading.iter().any(|e| *e), "{profile:?}: evaded");
        assert!(o.arrived && o.harm.blast == 0, "{profile:?}: {:?}", o.harm);
    }
}

#[test]
fn optimal_escapes_a_creeper_lit_dead_ahead() {
    let g = Grid::floor(-30, 46);
    let o = run(&g, vec![Mob::new("creeper", [3.0, 1.0, 0.5], Brain::Chase)], Goal::Block([30, 1, 0]), MovementProfile::Optimal, 2, 2000);
    assert!(o.arrived && o.harm.blast == 0, "{:?}", o.harm);
}
