//! Headless execution: the follower drives the real movement simulation on a synthetic course.

use acacia_physics::Vec3;

use super::grid::Grid;
use super::search::{ORIGIN, ladder_course, opts};
use crate::movement::Movement;
use crate::pathfind::maneuver;
use crate::pathfind::{FollowStatus, Follower, Goal, MoveKind, PathNode, SearchOpts, Sense, Step, search};
use crate::world::PhysicsWorld;

pub struct Run {
    pub ticks: u32,
    pub replans: u32,
    pub end: Vec3,
}

fn sense(g: &Grid, m: &Movement) -> Sense {
    let pos = m.position().expect("started");
    let feet = [pos[0].floor() as i32, pos[1].floor() as i32, pos[2].floor() as i32];
    Sense { pos, on_ground: m.on_ground(), in_water: g.terrain().cell(feet).is_water() }
}

/// Does a node's work in the grid the way the navigator does it on a server: breaks and doors at
/// once, placing after the same maneuvers.
fn perform(g: &Grid, world: &PhysicsWorld, m: &mut Movement, node: &PathNode) {
    for step in node.work.steps() {
        match step {
            Step::Break(pos) => {
                g.set_state(pos, "air", "");
            }
            Step::Door { pos, .. } => g.toggle(pos),
            Step::Place { against, face } => {
                let target = face.adjacent(against);
                let [dx, _, dz] = face.offset();
                for _ in 0..30 {
                    let s = sense(g, m);
                    let ready = if node.kind == MoveKind::Pillar {
                        maneuver::rise(&s, target, &mut m.controls)
                    } else {
                        maneuver::to_edge(&s, [target[0] - dx, target[1] + 1, target[2] - dz], (dx, dz), &mut m.controls)
                    };
                    if ready {
                        break;
                    }
                    m.tick(world);
                }
                g.set_state(target, "dirt", "");
                m.controls.stop();
                m.controls.sneak = false;
            }
        }
    }
}

fn run(g: &Grid, start: Vec3, goal: Goal, max_ticks: u32, parkour: bool) -> Run {
    run_with(g, start, goal, max_ticks, &opts(parkour))
}

pub fn run_with(g: &Grid, start: Vec3, goal: Goal, max_ticks: u32, search_opts: &SearchOpts) -> Run {
    let world = g.physics();
    let mut m = Movement::new();
    m.start(start, 0.0, 0.0);
    let plan = |pos: Vec3| {
        let path = search(&g.terrain(), pos, &goal, search_opts);
        assert!(!path.nodes.is_empty() || path.complete, "no path from {pos:?}");
        (!path.complete, Follower::new(path.nodes, pos, 0.25))
    };
    let (mut partial, mut follower) = plan(start);
    let mut replans = 0;
    for tick in 0..max_ticks {
        let sense = sense(g, &m);
        let pos = sense.pos;
        match follower.tick(&sense, &mut m.controls) {
            FollowStatus::Moving => {}
            FollowStatus::Work => {
                let node = *follower.pending_work().expect("work due");
                perform(g, &world, &mut m, &node);
                follower.work_done();
            }
            FollowStatus::Arrived if !partial => {
                eprintln!("arrived at {pos:?} after {tick} ticks, {replans} re-plans");
                return Run { ticks: tick, replans, end: pos };
            }
            status => {
                if status != FollowStatus::Arrived {
                    replans += 1;
                    eprintln!("tick {tick}: {status:?} at {pos:?}, target {:?}", follower.ahead().first());
                }
                (partial, follower) = plan(pos);
            }
        }
        m.tick(&world);
    }
    panic!("not arrived after {max_ticks} ticks: at {:?}, next {:?}", m.position(), follower.ahead().first());
}

pub fn assert_at(r: &Run, goal: [f32; 3], tolerance: f32) {
    let d = (r.end[0] - goal[0]).hypot(r.end[2] - goal[2]);
    assert!(d < tolerance && (r.end[1] - goal[1]).abs() < 0.1, "ended at {:?}, wanted {goal:?}", r.end);
    assert_eq!(r.replans, 0, "re-planned");
}

#[test]
fn walks_and_stops_precisely() {
    let g = Grid::floor(-20, 20);
    let r = run(&g, ORIGIN, Goal::Block([15, 1, 0]), 200, true);
    assert_at(&r, [15.5, 1.0, 0.5], 0.3);
    assert!(r.ticks < 90, "sprints the straight: {} ticks", r.ticks);
    let r = run(&g, ORIGIN, Goal::Block([-6, 1, 9]), 200, true);
    assert_at(&r, [-5.5, 1.0, 9.5], 0.3);
}

#[test]
fn walks_around_a_wall() {
    let g = Grid::floor(-20, 20);
    g.fill([3, 1, -4], [3, 3, 4], "stone");
    let r = run(&g, ORIGIN, Goal::Block([6, 1, 0]), 300, true);
    assert_at(&r, [6.5, 1.0, 0.5], 0.3);
}

#[test]
fn climbs_stairs_and_drops() {
    let g = Grid::floor(-20, 20);
    for k in 1..=3 {
        g.fill([k, 1, -1], [k, k, 1], "stone");
    }
    g.fill([4, 1, -1], [5, 3, 1], "stone");
    let r = run(&g, ORIGIN, Goal::Block([5, 4, 0]), 300, true);
    assert_at(&r, [5.5, 4.0, 0.5], 0.3);
    let r = run(&g, r.end, Goal::Block([8, 1, 0]), 300, true);
    assert_at(&r, [8.5, 1.0, 0.5], 0.3);
}

#[test]
fn walks_up_slabs() {
    let g = Grid::floor(-20, 20);
    g.fill_state([2, 1, -1], [2, 1, 1], "smooth_stone_slab", "minecraft:vertical_half=bottom");
    g.fill([3, 1, -1], [5, 1, 1], "stone");
    let r = run(&g, ORIGIN, Goal::Block([5, 2, 0]), 200, true);
    assert_at(&r, [5.5, 2.0, 0.5], 0.3);
}

#[test]
fn jumps_gaps() {
    let g = Grid::new();
    g.fill([-5, 0, -1], [0, 0, 1], "stone").fill([2, 0, -1], [3, 0, 1], "stone").fill([6, 0, -1], [9, 0, 1], "stone");
    let r = run(&g, ORIGIN, Goal::Block([8, 1, 0]), 300, true);
    assert_at(&r, [8.5, 1.0, 0.5], 0.3);
}

#[test]
fn swims_across() {
    let g = Grid::floor(-20, 20);
    g.fill([2, -3, -10], [5, -3, 10], "stone").fill([2, 0, -10], [5, 0, 10], "air");
    g.fill_state([2, -2, -10], [5, 0, 10], "water", "liquid_depth=0");
    let r = run(&g, ORIGIN, Goal::Block([8, 1, 0]), 600, false);
    assert_at(&r, [8.5, 1.0, 0.5], 0.3);
}

#[test]
fn climbs_a_ladder() {
    let g = Grid::floor(-20, 20);
    ladder_course(&g);
    let r = run(&g, [-1.5, 1.0, 0.5], Goal::Block([3, 6, 0]), 400, false);
    assert_at(&r, [3.5, 6.0, 0.5], 0.3);
}
