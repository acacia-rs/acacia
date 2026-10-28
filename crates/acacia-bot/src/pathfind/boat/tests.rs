//! Routes and whole legs on synthetic water: still water at y -61 with stone banks, the live boat simulated by
//! `boat_tick` from the keys the pilot holds.

use std::collections::VecDeque;

use acacia_physics::test_world::{TestBlock, TestWorld};
use acacia_physics::{Aabb, BlockPhysics, BlockPos, BoatState, Liquid, LiquidKind, Vec3, WorldView, boat_tick};

use super::grid::WaterGrid;
use super::{BoatOrder, BoatPilot, plan_leg, route};
use crate::movement::Controls;
use crate::riding::Mount;
use crate::pathfind::profile::MovementProfile;

const WATER: TestBlock = TestBlock::Liquid(Liquid { kind: LiquidKind::Water, depth: 8, falling: false });
const Y: i32 = -61;
const FLOAT_Y: f32 = -60.03334;
/// Blocks from the stop within which touching a bank is not counted (`solve::NUDGE_REACH`, with slack).
const NEAR_STOP: f32 = 2.0;

/// Stone from y -63 to -61 over the box, water two deep (y -62 and -61) in `pools` (inclusive x/z boxes): no exit
/// spot stands in the water.
fn land(min: [i32; 2], max: [i32; 2], pools: &[([i32; 2], [i32; 2])]) -> TestWorld {
    let mut w = TestWorld::new();
    w.fill(Some(TestBlock::Stone), [min[0], Y - 2, min[1]], [max[0], Y, max[1]]);
    for (a, b) in pools {
        w.fill(Some(WATER), [a[0], Y - 1, a[1]], [b[0], Y, b[1]]);
    }
    w
}

/// Nothing is known past `edge` in x.
struct Fog<'a> {
    world: &'a TestWorld,
    edge: i32,
}

impl WorldView for Fog<'_> {
    fn block_collisions(&self, pos: BlockPos, out: &mut Vec<Aabb>) {
        self.world.block_collisions(pos, out);
    }

    fn block(&self, pos: BlockPos) -> BlockPhysics {
        self.world.block(pos)
    }

    fn is_area_loaded(&self, area: &Aabb) -> bool {
        area.max[0] <= self.edge as f32
    }
}

#[test]
fn a_lake_route_goes_round_an_island() {
    let mut w = land([-40, -40], [40, 40], &[([-30, -30], [30, 30])]);
    w.fill(Some(TestBlock::Stone), [-5, Y, -20], [5, Y + 1, 20]);
    let grid = WaterGrid::new(&w, Y);
    let r = route::plan(&grid, [-20.0, 0.5], [20.0, 0.5], 0.0).expect("route");
    assert!(r.complete && r.points.len() >= 3 && r.length() < 2.0 * 45.0, "{:?}", r.points);
    for seg in r.points.windows(2) {
        assert!(grid.line_clear(seg[0], seg[1], 0.0), "{seg:?}");
    }
}

#[test]
fn a_one_wide_neck_is_refused_and_unknown_water_is_blocked() {
    // Two pools joined by a 1-wide channel at z 0.
    let w = land([-30, -10], [30, 10], &[([-25, -8], [-5, 8]), ([5, -8], [25, 8]), ([-5, 0], [5, 0])]);
    let grid = WaterGrid::new(&w, Y);
    let partial = route::plan(&grid, [-15.0, 0.5], [15.0, 0.5], 0.0).expect("partial");
    assert!(!partial.complete && partial.points.last().unwrap()[0] < -4.0, "{partial:?}");
    let w = land([-30, -10], [30, 10], &[([-25, -8], [25, 8])]);
    let fog = Fog { world: &w, edge: 0 };
    let partial = route::plan(&WaterGrid::new(&fog, Y), [-15.0, 0.5], [15.0, 0.5], 0.0).expect("partial");
    assert!(!partial.complete && partial.points.last().unwrap()[0] <= -0.5, "{partial:?}");
}

/// A river: east 60 blocks, a bend north, then east again, `width` wide; the target is on the far end's bank.
fn river(width: i32) -> (TestWorld, Vec3, BlockPos) {
    let w = land([-10, -20], [140, 80], &[([0, 0], [60, width - 1]), ([60 - width + 1, 0], [60, 50]), ([60, 50 - width + 1], [120, 50])]);
    (w, [3.0, FLOAT_Y, width as f32 / 2.0], [122, Y + 1, 50 - width / 2])
}

#[derive(Debug)]
#[allow(dead_code, reason = "fields only read through Debug, in failure messages")]
struct Paddled {
    profile: MovementProfile,
    seed: u64,
    /// The route: where the boat is to rest, and the end of a partial one.
    stop: [f32; 2],
    ticks: u64,
    /// First tick within `NEAR_STOP` of the stop.
    near_at: Option<u64>,
    contacts: u32,
    /// Where the exit search puts the rider from the resting boat, and where the leg planned it.
    exit: Vec3,
    planned: Vec3,
    end: Vec3,
    failed: Option<String>,
    /// The last ticks, or those before the first contact: (x, z), yaw, keys.
    recent: VecDeque<([f32; 2], f32, [f32; 2])>,
}

fn paddle(w: &TestWorld, start: Vec3, target: BlockPos, profile: MovementProfile, seed: u64) -> Paddled {
    let leg = plan_leg(w, start, target, [0.0; 3], profile).expect("leg");
    let exit = leg.exit.expect("a stop by the target");
    let stop = *leg.route.points.last().expect("route points");
    let mut pilot = BoatPilot::new(&leg, [0.0; 3], profile, 20 * 60 * 5, seed);
    let mut st = BoatState::new(start, 0.0);
    let mut c = Controls::default();
    let grid = WaterGrid::new(w, Y);
    let (mut contacts, mut recent, mut near_at) = (0, VecDeque::new(), None);
    for tick in 0.. {
        match pilot.tick(&st, w, &mut c) {
            BoatOrder::Paddle => {}
            BoatOrder::Arrived => {
                // Where BDS's exit search puts the rider from the boat as it really rests.
                let mount = Mount { vehicle: st.pos, yaw: st.yaw, motion: [st.vel[0], st.vel[2]], seat_offset: [0.0; 3] };
                let landed = mount.exit_feet(w).unwrap_or([f32::NAN; 3]);
                return Paddled { profile, seed, stop, ticks: tick, near_at, contacts, exit: landed, planned: exit, end: st.pos, failed: None, recent };
            }
            BoatOrder::Fail(why) => return Paddled { profile, seed, stop, ticks: tick, near_at, contacts, exit, planned: exit, end: st.pos, failed: Some(why), recent },
        }
        if contacts == 0 {
            if recent.len() == 30 {
                recent.pop_front();
            }
            recent.push_back(([st.pos[0], st.pos[2]], st.yaw, [c.strafe, c.forward]));
        }
        boat_tick(&mut st, [c.strafe, c.forward], w);
        // Touching: within 2 mm of a bank (an exact touch is clear to the grid). A stop lies against its bank, so
        // easing onto it may nudge that bank.
        let away = (st.pos[0] - stop[0]).hypot(st.pos[2] - stop[1]) > NEAR_STOP;
        near_at = near_at.or((!away).then_some(tick));
        contacts += u32::from(away && !grid.clear(st.pos[0], st.pos[2], 0.002));
    }
    unreachable!()
}

fn check(p: &Paddled, target: BlockPos, max_ticks: u64) {
    eprintln!("LEG {:?} seed {}: {} ticks, near the stop {:?} at {:?}, exit {:?}", p.profile, p.seed, p.ticks, p.stop, p.near_at, p.exit);
    assert!(p.failed.is_none(), "{p:?}");
    assert_eq!(p.contacts, 0, "{p:?}");
    assert!(p.ticks <= max_ticks, "{p:?}");
    let centre = [target[0] as f32 + 0.5, target[2] as f32 + 0.5];
    assert!((p.exit[0] - centre[0]).hypot(p.exit[2] - centre[1]) < 2.5 && p.exit[1].floor() as i32 == target[1], "{p:?}");
}

#[test]
fn optimal_paddles_a_bending_river_to_the_bank() {
    let (w, start, target) = river(4);
    let p = paddle(&w, start, target, MovementProfile::Optimal, 1);
    // About 170 blocks at 2.64 ticks a block, plus the turns and the stop.
    check(&p, target, 600);
}

#[test]
fn human_paddles_it_for_seeds_one_to_five() {
    let (w, start, target) = river(5);
    for seed in 1..=5 {
        let p = paddle(&w, start, target, MovementProfile::Human, seed);
        check(&p, target, 700);
    }
}

#[test]
fn the_live_drill_river_ends_at_its_bank() {
    // examples/boat.rs at a=30 b=12 c=30 from a bot at the origin: the stop sits off the channel's line, and the
    // boat once hovered 4-5 blocks short of it for 130 ticks (sim and live alike).
    let w = land([-10, -10], [80, 30], &[([1, -3], [30, 2]), ([25, -3], [30, 12]), ([25, 7], [60, 12])]);
    for (profile, max_ticks) in [(MovementProfile::Optimal, 260), (MovementProfile::Human, 380)] {
        let p = paddle(&w, [2.5, FLOAT_Y, 0.5], [61, Y + 1, 9], profile, 2);
        check(&p, [61, Y + 1, 9], max_ticks);
    }
}

#[test]
fn a_two_wide_canal_then_a_square_turn() {
    // 2 wide for 36 blocks, a basin, then 5 wide north: the boat's smallest turn is 1.7 blocks across.
    let w = land([-10, -10], [60, 60], &[([0, 0], [40, 1]), ([36, -2], [42, 4]), ([38, 0], [42, 40])]);
    for profile in [MovementProfile::Optimal, MovementProfile::Human] {
        let p = paddle(&w, [3.0, FLOAT_Y, 1.0], [44, Y + 1, 38], profile, 3);
        check(&p, [44, Y + 1, 38], 900);
    }
}
