//! The boat model's shape for planning: banks, turns, stops and channels (still water, `boat.rs` as fitted).

use acacia_physics::test_world::{TestBlock, TestWorld};
use acacia_physics::{BoatState, Liquid, LiquidKind, boat_tick};

const WATER: TestBlock = TestBlock::Liquid(Liquid { kind: LiquidKind::Water, depth: 8, falling: false });
const SURFACE_Y: f32 = -60.03334;

/// Still water at y -61 over x and z in `-r..=r`.
fn lake(r: i32) -> TestWorld {
    let mut world = TestWorld::new();
    world.fill(Some(WATER), [-r, -61, -r], [r, -61, r]);
    world
}

/// The bow points along (sin a, cos a), a = 90° − yaw: yaw 0 is +x.
fn boat(x: f32, z: f32, yaw: f32) -> BoatState {
    BoatState::new([x, SURFACE_Y, z], yaw)
}

fn run(st: &mut BoatState, keys: [f32; 2], ticks: u32, world: &TestWorld) {
    for _ in 0..ticks {
        boat_tick(st, keys, world);
    }
}

fn speed(st: &BoatState) -> f32 {
    st.vel[0].hypot(st.vel[2])
}

#[test]
fn a_bank_stops_the_slide() {
    let mut world = lake(40);
    world.fill(Some(TestBlock::Stone), [10, -61, -40], [10, -60, 40]);
    // yaw 0: a = 90°, the bow points +x.
    let mut st = boat(0.5, 0.5, 0.0);
    run(&mut st, [0.0, 1.0], 80, &world);
    assert!((st.pos[0] - (10.0 - 0.7)).abs() < 1e-3, "{:?}", st.pos);
    assert_eq!(st.vel[0], 0.0);
}

#[test]
fn cruise_speed_and_stop_distance() {
    let world = lake(200);
    let mut st = boat(-150.5, 0.5, 0.0);
    run(&mut st, [0.0, 1.0], 100, &world);
    let v = speed(&st);
    assert!((0.35..0.40).contains(&v), "cruise {v}");
    let x = st.pos[0];
    let mut ticks = 0;
    // S pushes back: it brakes until the boat stops and then reverses it.
    while st.vel[0] > 0.0 {
        boat_tick(&mut st, [0.0, -1.0], &world);
        ticks += 1;
        assert!(ticks < 100, "S never stops it");
    }
    let braked = st.pos[0] - x;
    let mut coast = boat(-150.5, 10.5, 0.0);
    run(&mut coast, [0.0, 1.0], 100, &world);
    let x = coast.pos[0];
    run(&mut coast, [0.0, 0.0], 60, &world);
    let coasted = coast.pos[0] - x;
    // S from cruise: about 1.1 blocks in 11 ticks; a coast drifts 1.6.
    assert!((braked - 1.11).abs() < 0.05 && ticks == 11, "braked {braked} in {ticks}");
    assert!((coasted - 1.61).abs() < 0.05, "coasted {coasted}");
}

#[test]
fn turn_radius_table() {
    let world = lake(200);
    // (keys, steady degrees per tick, radius): W+A and A alone.
    for (keys, want_rate, want_radius) in [([1.0, 1.0], -5.08, 3.09), ([1.0, 0.0], -5.69, 1.72)] {
        let mut st = boat(0.5, 0.5, 0.0);
        run(&mut st, keys, 60, &world);
        let yaw0 = st.yaw;
        run(&mut st, keys, 20, &world);
        let rate = (st.yaw - yaw0) / 20.0;
        let radius = speed(&st) / rate.abs().to_radians();
        assert!((rate - want_rate).abs() < 0.01 && (radius - want_radius).abs() < 0.01, "{keys:?}: {rate}°/t, radius {radius}");
    }
}

#[test]
fn the_box_fits_a_two_wide_channel() {
    let mut world = lake(60);
    world.fill(Some(TestBlock::Stone), [-60, -61, -60], [60, -60, -1]);
    world.fill(Some(TestBlock::Stone), [-60, -61, 2], [60, -60, 60]);
    // Channel z in [0, 2); the bow along +x.
    let mut st = boat(-50.0, 1.0, 0.0);
    run(&mut st, [0.0, 1.0], 120, &world);
    assert!(st.pos[0] > -10.0, "{:?}", st.pos);
    assert_eq!(st.pos[2], 1.0);
    let mut world = lake(60);
    world.fill(Some(TestBlock::Stone), [-60, -61, -60], [60, -60, -1]);
    world.fill(Some(TestBlock::Stone), [-60, -61, 1], [60, -60, 60]);
    let mut st = boat(-50.0, 0.5, 0.0);
    run(&mut st, [0.0, 1.0], 20, &world);
    // A one-wide channel holds a box only by overlapping its walls: the slide cannot start.
    assert!(st.pos[0] < -49.0, "{:?}", st.pos);
}
