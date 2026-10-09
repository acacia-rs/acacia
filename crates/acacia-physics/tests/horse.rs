//! The ridden horse against strict BDS 1.26.52: numbers from live rides of natural horses the server
//! did not correct (2026-10-09, docs/research/riding-fishing-elytra.md "Horse").

use acacia_physics::test_world::{TestBlock, TestWorld};
use acacia_physics::{HorseJump, PlayerState, RiderInput, horse, horse_tick};

fn flat() -> TestWorld {
    let mut w = TestWorld::new();
    w.fill(Some(TestBlock::Stone), [-4, -1, -4], [4, -1, 40]);
    w
}

/// A horse standing on the ground at the origin, facing +z.
fn standing(speed: f32, world: &TestWorld) -> PlayerState {
    let mut st = horse([0.5, 0.0, 0.5], 0.0, speed);
    let mut jump = HorseJump::new(0.0);
    for _ in 0..3 {
        horse_tick(&mut st, &mut jump, &RiderInput::default(), world);
    }
    assert!(st.on_ground);
    st
}

#[test]
fn the_first_step_forward_is_the_movement_attribute() {
    let world = flat();
    let speed = 0.202_146_2;
    let mut st = standing(speed, &world);
    let out = horse_tick(&mut st, &mut HorseJump::new(0.0), &RiderInput { move_vector: [0.0, 1.0], ..RiderInput::default() }, &world);
    // The server's horse went from z 152.58868 to 152.79083: the keys are not scaled by 0.98.
    assert!((out.position[2] - 0.5 - speed).abs() < 1e-5, "{:?}", out.position);
}

#[test]
fn two_keys_share_the_vector_before_strafing_is_halved() {
    let world = flat();
    let speed = 0.2;
    let mut st = standing(speed, &world);
    let out = horse_tick(&mut st, &mut HorseJump::new(0.0), &RiderInput { move_vector: [-1.0, 1.0], ..RiderInput::default() }, &world);
    let step = (out.position[0] - 0.5).hypot(out.position[2] - 0.5);
    // (-0.7071 * 0.5, 0.7071): strict BDS moved its horse 0.7905 of the speed (W+D).
    assert!((step - speed * 0.625f32.sqrt()).abs() < 1e-5, "{step}");
}

#[test]
fn a_held_jump_charges_and_its_release_leaps() {
    let world = flat();
    let mut st = standing(0.2, &world);
    let mut jump = HorseJump::new(0.570_957_2);
    let held = RiderInput { jump: true, ..RiderInput::default() };
    for _ in 0..20 {
        let out = horse_tick(&mut st, &mut jump, &held, &world);
        assert_eq!(out.position[1], 0.0, "charging keeps the horse down");
    }
    // 19 charged ticks give 0.82, so 0.4 + 0.4 * 82 / 90 of the strength: the server's horse rose 0.436466.
    let out = horse_tick(&mut st, &mut jump, &RiderInput::default(), &world);
    assert!((out.position[1] - 0.436_466).abs() < 1e-5, "{:?}", out.position);
    assert_eq!(out.position[2], 0.5, "no push forward without the forward key");
}

#[test]
fn a_tap_hops_and_a_running_jump_is_pushed_forward() {
    let world = flat();
    let mut st = standing(0.2, &world);
    let mut jump = HorseJump::new(0.6);
    let forward = RiderInput { move_vector: [0.0, 1.0], ..RiderInput::default() };
    horse_tick(&mut st, &mut jump, &RiderInput { jump: true, ..forward }, &world);
    let before = st.pos[2];
    let out = horse_tick(&mut st, &mut jump, &forward, &world);
    assert!((out.position[1] - 0.6 * 0.4).abs() < 1e-6, "a tap jumps at 0.4 of the strength: {:?}", out.position);
    assert!(out.position[2] - before > 0.2 + 0.4 * 0.4 - 1e-3, "pushed forward by 0.4 of the power: {}", out.position[2] - before);
}
