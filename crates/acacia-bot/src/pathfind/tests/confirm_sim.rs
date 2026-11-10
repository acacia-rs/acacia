//! C3 in the headless simulation: the pilot holds still after a server setback and re-plans from where the server
//! put the bot; path work the server keeps undoing ends in a loop error instead of digging forever.

use acacia_client::proto::packets::{CorrectPlayerMovePrediction, CorrectPlayerMovePredictionPredictionType as Kind};
use acacia_client::proto::types::{Vec2f, Vec3f};
use acacia_physics::{BlockPos, Vec3};

use super::alter::{tool, with};
use super::grid::Grid;
use super::sim::perform;
use crate::movement::{Movement, Rollout, EYE_HEIGHT};
use crate::pathfind::{Goal, GotoOpts, NavStatus, Pilot, SearchJob, Step, answered};
use crate::state::queries::test_support::raw;
use crate::state::Me;

const ME: Me = Me { runtime_entity_id: 1, unique_entity_id: -1 };

/// An immediate (tick 0) correction putting the bot's feet at `feet`.
fn correct(m: &mut Movement, feet: Vec3) {
    let packet = raw(&CorrectPlayerMovePrediction {
        prediction_type: Kind::Player,
        position: Vec3f { x: feet[0], y: feet[1] + EYE_HEIGHT, z: feet[2] },
        delta: Vec3f { x: 0.0, y: 0.0, z: 0.0 },
        rotation: Vec2f { x: 0.0, z: 0.0 },
        angular_velocity: None,
        on_ground: true,
        tick: 0,
    });
    m.apply(&packet, &ME).expect("correction applies");
}

struct Drive {
    result: Result<(), String>,
    track: Vec<Vec3>,
    pilot: Pilot,
}

/// Drives a pilot from `start`; `server(tick, movement)` may correct the bot, `undo(dug)` may put back what work broke.
fn drive(g: &Grid, start: Vec3, pilot: Pilot, max_ticks: u32, mut server: impl FnMut(u32, &mut Movement), undo: impl Fn(&Grid, &[BlockPos])) -> Drive {
    let world = g.physics();
    let mut m = Movement::new();
    m.start(start, 0.0, 0.0);
    let mut d = Drive { result: Err("out of ticks".into()), track: Vec::new(), pilot };
    for tick in 0..max_ticks {
        server(tick, &mut m);
        d.track.push(m.position().expect("started"));
        let opts = d.pilot.search_opts(None, 20.0);
        match d.pilot.drive(&g.terrain(), &mut m, opts, &mut |job: SearchJob<Rollout>| answered(job.run(&g.terrain()))) {
            Ok(NavStatus::Arrived) => {
                d.result = Ok(());
                break;
            }
            Ok(NavStatus::Working) => {
                let node = d.pilot.pending_work().expect("work due");
                perform(g, &world, &mut m, &node);
                d.pilot.work_done();
                let dug: Vec<_> = node.work.steps().filter_map(|s| if let Step::Break(p) = s { Some(p) } else { None }).collect();
                undo(g, &dug);
            }
            Ok(_) => {}
            Err(e) => {
                d.result = Err(e.to_string());
                break;
            }
        }
        m.tick(&world);
    }
    d
}

fn moved(track: &[Vec3], from: usize, to: usize) -> f32 {
    (track[to][0] - track[from][0]).hypot(track[to][2] - track[from][2])
}

#[test]
fn a_setback_holds_the_bot_still_then_it_replans_from_there() {
    let g = Grid::floor(-10, 40);
    let pilot = Pilot::new(Goal::Block([30, 1, 0]), GotoOpts { seed: Some(1), ..GotoOpts::default() });
    let d = drive(&g, [0.5, 1.0, 0.5], pilot, 400, |tick, m| {
        let at = m.position().expect("started");
        match tick {
            // The server puts the bot half a block back and aside.
            20 => correct(m, [at[0] - 0.5, at[1], at[2] + 0.5]),
            // An ulp-sized correction is no setback.
            60 => correct(m, [at[0] + 2e-6, at[1], at[2]]),
            _ => {}
        }
    }, |_, _| {});
    assert_eq!(d.result, Ok(()), "arrived");
    assert_eq!(d.pilot.settles(), 1, "held once: the ulp correction is not a setback");
    assert_eq!(d.pilot.replans(), 0, "the re-plan after a setback is free");
    assert!(moved(&d.track, 24, 29) < 0.05, "stood still while settling: moved {:.3}", moved(&d.track, 24, 29));
    assert!(moved(&d.track, 32, 40) > 1.0, "on its way again after {} ticks", crate::governor::SETTLE_TICKS);
    let end = d.track.last().expect("ticked");
    assert!((end[0] - 30.5).hypot(end[2] - 0.5) < 0.3, "ended at {end:?}");
}

#[test]
fn work_the_server_keeps_undoing_ends_in_a_loop_error() {
    let g = Grid::floor(-10, 30);
    g.fill([3, 1, -10], [8, 10, 30], "stone");
    let opts = GotoOpts { search: with(true, false, tool("minecraft:iron_pickaxe")), ..GotoOpts::default() };
    let pilot = Pilot::new(Goal::Block([12, 1, 0]), opts);
    let d = drive(&g, [0.5, 1.0, 0.5], pilot, 2000, |_, _| {}, |g, dug| dug.iter().for_each(|p| {
        g.set_state(*p, "stone", "");
    }));
    let err = d.result.expect_err("cannot get through");
    assert!(err.contains("loop"), "ended with: {err}");
    assert!(d.track.len() < 600, "bailed after {} ticks", d.track.len());
    assert!(d.pilot.work_repeats() >= 3);
}
