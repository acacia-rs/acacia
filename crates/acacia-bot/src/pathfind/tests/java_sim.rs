//! The shared planner and follower driving the Java player on a world of Java ids.

use acacia_physics::Vec3;
use acacia_world::BlockRegistry;

use super::grid::Grid;
use super::search::{ORIGIN, opts};
use crate::movement::{Controls, JavaSim, JavaView, Sim};
use crate::pathfind::{FollowStatus, Follower, Goal, Sense, search};

fn java_floor(lo: i32, hi: i32) -> Grid {
    let g = Grid::with_registry(BlockRegistry::java_arc(), 2);
    g.fill([lo, 0, lo], [hi, 0, hi], "stone");
    g
}

/// Ticks until the Java player arrives at `goal`, and where it ended; panics on a re-plan.
fn run(g: &Grid, start: Vec3, goal: Goal, max_ticks: u32) -> (u32, Vec3) {
    let terrain = g.terrain();
    let world = JavaView(&g.view);
    let path = search::<JavaSim, _>(&terrain, start, &goal, &opts(true));
    assert!(path.complete, "no path to {goal:?}");
    let mut follower = Follower::new(path.nodes, start, 0.25);
    let mut sim = JavaSim::standing(start);
    let mut controls = Controls::default();
    for tick in 0..max_ticks {
        let body = sim.body();
        let feet = body.pos.map(|v| v.floor() as i32);
        let sense = Sense { pos: body.pos, on_ground: body.on_ground, in_water: terrain.cell(feet).is_water() };
        match follower.tick(&sense, Some(&sim), &world, &mut controls) {
            FollowStatus::Moving => {}
            FollowStatus::Arrived => return (tick, body.pos),
            status => panic!("tick {tick}: {status:?} at {:?}", body.pos),
        }
        sim.step_in(&controls, &world);
    }
    panic!("not arrived after {max_ticks} ticks: at {:?}", sim.body().pos);
}

fn assert_near(end: Vec3, want: Vec3) {
    let d = (end[0] - want[0]).hypot(end[2] - want[2]);
    assert!(d < 0.3 && (end[1] - want[1]).abs() < 0.1, "ended at {end:?}, wanted {want:?}");
}

#[test]
fn java_player_sprints_a_straight_and_turns() {
    let g = java_floor(-20, 20);
    let (ticks, end) = run(&g, ORIGIN, Goal::Block([15, 1, 0]), 200);
    assert_near(end, [15.5, 1.0, 0.5]);
    assert!(ticks < 90, "sprints the straight: {ticks} ticks");
    let (_, end) = run(&g, end, Goal::Block([-6, 1, 9]), 300);
    assert_near(end, [-5.5, 1.0, 9.5]);
}

#[test]
fn java_player_goes_around_a_wall_and_up_stairs() {
    let g = java_floor(-20, 20);
    g.fill([3, 1, -4], [3, 3, 4], "stone");
    let (_, end) = run(&g, ORIGIN, Goal::Block([6, 1, 0]), 300);
    assert_near(end, [6.5, 1.0, 0.5]);
    for k in 1..=3 {
        g.fill([8 + k, 1, -1], [8 + k, k, 1], "stone");
    }
    let (_, end) = run(&g, end, Goal::Block([11, 4, 0]), 300);
    assert_near(end, [11.5, 4.0, 0.5]);
}

#[test]
fn java_player_jumps_a_gap() {
    let g = java_floor(-20, 20);
    g.fill([3, 0, -20], [4, 0, 20], "air");
    let (_, end) = run(&g, ORIGIN, Goal::Block([8, 1, 0]), 300);
    assert_near(end, [8.5, 1.0, 0.5]);
}
