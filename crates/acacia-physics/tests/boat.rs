//! The boat against BDS 1.26.52: positions from a strict server's corrections of a boat paddled
//! straight from rest on still water (2026-10-09, docs/research/riding-fishing-elytra.md "Boat").

use acacia_physics::test_world::{TestBlock, TestWorld};
use acacia_physics::{BoatState, Liquid, LiquidKind, boat_tick, wave_phases};

fn pool() -> TestWorld {
    let mut world = TestWorld::new();
    world.fill(Some(TestBlock::Liquid(Liquid::source(LiquidKind::Water))), [200, -61, 210], [240, -61, 220]);
    world
}

/// The server's boat after input tick 183, floating at rest.
fn at_rest() -> BoatState {
    let mut boat = BoatState::new([215.5, -60.03334, 215.5], 0.0);
    boat.vel = [0.0, -0.002_384_632_6, 0.0];
    boat.wave = wave_phases(&boat).expect("floating")[0];
    boat
}

#[test]
fn paddling_straight_follows_the_servers_positions() {
    let world = pool();
    let mut boat = at_rest();
    // (ticks paddled, x, y) as the server reported them; the second stroke starts on the 11th tick.
    let server = [(1, 215.574_25, -60.035_725), (2, 215.706_16, -60.038_567), (10, 217.874_42, -60.047_165), (11, 218.217_76, -60.041_09), (14, 219.282_29, -60.032_394)];
    let mut ticks = 0;
    for (at, x, y) in server {
        while ticks < at {
            boat_tick(&mut boat, [0.0, 1.0], &world);
            ticks += 1;
        }
        assert!((boat.pos[0] - x).abs() < 2e-5 && (boat.pos[1] - y).abs() < 2e-5, "tick {at}: {:?}, server ({x}, {y})", boat.pos);
    }
    assert_eq!(boat.pos[2], 215.5);
}

#[test]
fn the_wave_reads_back_from_the_motion() {
    let world = pool();
    let mut boat = at_rest();
    for _ in 0..5 {
        boat_tick(&mut boat, [0.0, 0.0], &world);
    }
    let phases = wave_phases(&boat).expect("floating");
    let turn = std::f64::consts::TAU;
    assert!(phases.iter().any(|p| ((p - boat.wave).rem_euclid(turn)).min((boat.wave - p).rem_euclid(turn)) < 1e-2), "{phases:?} {}", boat.wave);
}

#[test]
fn a_single_paddle_turns_the_boat() {
    let world = pool();
    let (mut left, mut right) = (at_rest(), at_rest());
    for _ in 0..20 {
        boat_tick(&mut left, [1.0, 0.0], &world);
        boat_tick(&mut right, [-1.0, 0.0], &world);
    }
    // Left (A) turns the yaw down, as on the server; the two keys mirror each other.
    assert!(left.yaw < -60.0, "{}", left.yaw);
    assert_eq!(left.yaw, -right.yaw);
}
