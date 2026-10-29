//! Processes arbitrated in the headless simulation: the grid stands in for the server (digs, fills), a scripted
//! position for a tracked player.

use acacia_physics::{BlockPos, Vec3};
use acacia_world::BlockAccess;

use super::alter::{tool, with};
use super::grid::Grid;
use super::sim::perform;
use crate::interact::{block_distance, Face, BLOCK_REACH};
use crate::movement::Movement;
use crate::pathfind::process::{Act, Arbiter, Decision, Look, ProcId};
use crate::pathfind::{Backfill, Follow, Goal, Goto, GotoOpts, NavStatus, SearchJob, Step, Target, answered};
use crate::ActionError;

pub(super) struct SimLook<'a> {
    pub g: &'a Grid,
    pub target: Option<Vec3>,
}

impl Look for SimLook<'_> {
    fn solid(&self, p: BlockPos) -> Option<bool> {
        let s = self.g.registry.get(self.g.view.block(p[0], p[1], p[2]))?;
        Some(!s.is_air() && !s.is_liquid())
    }

    fn find(&self, _: &Target) -> Option<Vec3> {
        self.target
    }
}

fn name(g: &Grid, p: BlockPos) -> String {
    g.registry.get(g.view.block(p[0], p[1], p[2])).map_or_else(String::new, |s| s.name.to_string())
}

pub struct Run {
    pub ticks: u32,
    pub track: Vec<Vec3>,
    /// The process in control at each tick.
    pub control: Vec<Option<ProcId>>,
    pub dug: Vec<BlockPos>,
}

/// Places the first of the act's blocks that `held` has, as the server would: from within reach, against a
/// solid neighbour, not into the player.
fn fill(g: &Grid, m: &Movement, act: &Act, held: &[&str]) -> Result<(), ActionError> {
    let Act::Fill { pos, blocks } = act else { return Err(ActionError::NotPossible(format!("{act:?} is not simulated"))) };
    let feet = m.position().expect("started");
    let eye = [feet[0], feet[1] + 1.62, feet[2]];
    let look = SimLook { g, target: None };
    let reach = Face::ALL.into_iter().map(|f| f.offset()).any(|[dx, dy, dz]| {
        let against = [pos[0] - dx, pos[1] - dy, pos[2] - dz];
        look.solid(against) == Some(true) && block_distance(eye, against) <= BLOCK_REACH
    });
    let inside = (feet[0] - 0.3 < (pos[0] + 1) as f32 && feet[0] + 0.3 > pos[0] as f32) && (feet[2] - 0.3 < (pos[2] + 1) as f32 && feet[2] + 0.3 > pos[2] as f32) && (feet[1] < (pos[1] + 1) as f32 && feet[1] + 1.8 > pos[1] as f32);
    let block = blocks.iter().find(|b| held.contains(&b.as_str()));
    match block {
        Some(b) if reach && !inside => {
            g.set_state(*pos, b.trim_start_matches("minecraft:"), "");
            Ok(())
        }
        _ => Err(ActionError::NotPossible(format!("cannot fill {pos:?} from {feet:?} (reach {reach}, inside {inside})"))),
    }
}

/// Runs `arb` from `start` until `until` holds or `max_ticks` pass; `target(tick, arb)` places the tracked player
/// (and may pause or resume processes).
pub fn run(g: &Grid, start: Vec3, arb: &mut Arbiter, max_ticks: u32, held: &[&str], mut target: impl FnMut(u32, &mut Arbiter) -> Option<Vec3>, until: impl Fn(&Arbiter, &Grid, &[BlockPos]) -> bool) -> Run {
    let world = g.physics();
    let mut m = Movement::new();
    m.start(start, 0.0, 0.0);
    let mut run = Run { ticks: 0, track: Vec::new(), control: Vec::new(), dug: Vec::new() };
    for tick in 0..max_ticks {
        let pos = m.position().expect("started");
        let look = SimLook { g, target: target(tick, arb) };
        let decision = arb.decide(pos, m.on_ground(), &look);
        (run.ticks, run.track, run.control) = (tick, [run.track, vec![pos]].concat(), [run.control, vec![arb.active()]].concat());
        match decision {
            Decision::Go => {
                let pilot = arb.pilot_mut().expect("routed");
                let opts = pilot.search_opts(None, 20.0);
                let result = pilot.drive(&g.terrain(), &mut m, opts, &mut |job: SearchJob| answered(job.run(&g.terrain())));
                let mut dug = Vec::new();
                if result == Ok(NavStatus::Working) {
                    let node = pilot.pending_work().expect("work due");
                    dug = node.work.steps().filter_map(|s| if let Step::Break(p) = s { Some((p, name(g, p))) } else { None }).filter(|(p, _)| look.solid(*p) == Some(true)).collect();
                    perform(g, &world, &mut m, &node);
                    pilot.work_done();
                }
                if !matches!(result, Ok(NavStatus::Searching | NavStatus::Moving | NavStatus::Working)) {
                    m.controls.stop();
                }
                run.dug.extend(dug.iter().map(|(p, _)| *p));
                arb.drove(result, dug);
            }
            Decision::Act(id, act) => {
                m.controls.stop();
                let result = fill(g, &m, &act, held);
                arb.acted(id, &act, &result);
            }
            Decision::Hold | Decision::Idle | Decision::Empty => m.controls.stop(),
        }
        if until(arb, g, &run.dug) {
            break;
        }
        m.tick(&world);
    }
    eprintln!("RUN {} ticks, at {:?}, dug {:?}", run.ticks, run.track.last(), run.dug);
    run
}

fn horizontal(a: Vec3, b: Vec3) -> f32 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

const PLAYER: Target = Target::Entity(7);

#[test]
fn follow_preempts_goto_which_resumes_when_the_target_leaves() {
    let g = Grid::floor(-30, 46);
    let mut arb = Arbiter::new();
    let goto = arb.add(Goto::new(Goal::Block([40, 1, 0])), 10);
    let follow = arb.add(Follow::new(PLAYER, 3.0), 50);
    let player = |t: u32| [-10.5 + t as f32 * 0.02, 1.0, 20.5];
    let seen = 30..250;
    let r = run(&g, [0.5, 1.0, 0.5], &mut arb, 1200, &[], |t, _| seen.contains(&t).then(|| player(t)), |a, _, _| a.outcome(goto).is_some());
    assert_eq!(arb.outcome(goto), Some(&Ok(())), "goto finished");
    assert!(horizontal(r.track[249], player(249)) < 4.5, "followed the player: at {:?}", r.track[249]);
    assert!(r.control[100..249].iter().all(|c| *c == Some(follow)), "follow kept control while the player was seen");
    assert_eq!(r.control[260], Some(goto), "goto took over again");
    assert!(horizontal(*r.track.last().unwrap(), [40.5, 1.0, 0.5]) < 0.3);
}

#[test]
fn goto_resumes_while_follow_is_paused() {
    let g = Grid::floor(-30, 46);
    let mut arb = Arbiter::new();
    let goto = arb.add(Goto::new(Goal::Block([40, 1, 0])), 10);
    let follow = arb.add(Follow::new(PLAYER, 3.0), 50);
    let r = run(&g, [0.5, 1.0, 0.5], &mut arb, 1200, &[], |t, a| {
        if t == 200 {
            a.pause(follow);
        }
        Some([-10.5, 1.0, 20.5])
    }, |a, _, _| a.outcome(goto).is_some());
    assert_eq!(arb.outcome(goto), Some(&Ok(())));
    assert!(horizontal(r.track[199], [-10.5, 1.0, 20.5]) < 4.5, "followed first: at {:?}", r.track[199]);
    assert!(arb.outcome(follow).is_none() && arb.get(follow).is_some(), "follow is kept, paused");
}

#[test]
fn backfill_restores_every_dug_cell() {
    let g = Grid::floor(-10, 30);
    // Too high to climb over: tunnels.
    g.fill([3, 1, -10], [8, 10, 30], "stone");
    let mut arb = Arbiter::new();
    let opts = GotoOpts { search: with(true, false, tool("minecraft:iron_pickaxe")), ..GotoOpts::default() };
    let goto = arb.add(Goto::with(Goal::Block([12, 1, 0]), opts), 10);
    arb.add(Backfill::new(), 20);
    let solid = |g: &Grid, p: &BlockPos| SimLook { g, target: None }.solid(*p) == Some(true);
    let r = run(&g, [0.5, 1.0, 0.5], &mut arb, 2000, &["minecraft:cobblestone"], |_, _| None, |a, g, dug| a.outcome(goto).is_some() && dug.iter().all(|p| solid(g, p)));
    assert_eq!(arb.outcome(goto), Some(&Ok(())));
    assert!(r.dug.len() >= 12, "tunnelled through 6 blocks: dug {:?}", r.dug);
    let open: Vec<_> = r.dug.iter().filter(|p| !solid(&g, p)).collect();
    assert!(open.is_empty(), "left open: {open:?}");
    assert!(r.dug.iter().all(|p| name(&g, *p) == "minecraft:cobblestone"), "filled with the stone's substitute");
}
