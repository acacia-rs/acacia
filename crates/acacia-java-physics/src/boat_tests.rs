use acacia_registry::Block;

use super::*;

/// Stone at y -1, one layer of still water at y 0 where `|x|, |z| < 20`, air elsewhere.
struct Pool;

impl Blocks for Pool {
    fn state(&self, x: i32, y: i32, z: i32) -> Option<u32> {
        let block = match y {
            -1 => Block::Stone,
            0 if x.abs() < 20 && z.abs() < 20 => Block::Water,
            _ => Block::Air,
        };
        Some(block.default_state().id())
    }

    fn min_y(&self) -> i32 {
        -64
    }
}

const AHEAD: Keys = Keys { forward: true, backward: false, left: false, right: false, jump: false, shift: false, sprint: false };
const LEFT: Keys = Keys { forward: false, backward: false, left: true, right: false, jump: false, shift: false, sprint: false };

fn floating() -> Boat {
    let mut boat = Boat::new(&Pool, [0.5, 0.5, 0.5], 0.0, false);
    for _ in 0..200 {
        boat.tick(&Pool, Keys::default());
    }
    boat
}

#[test]
fn a_boat_settles_on_still_water() {
    let boat = floating();
    assert_eq!(boat.status, BoatStatus::InWater);
    assert_eq!([boat.pos[0], boat.pos[2]], [0.5, 0.5]);
    assert!(boat.delta[1].abs() < 1.0E-4, "still bobbing: {:?}", boat.delta);
    assert!(boat.pos[1] > 0.3 && boat.pos[1] < 8.0 / 9.0, "floats at {}", boat.pos[1]);
}

#[test]
fn paddling_ahead_goes_south_on_both_paddles() {
    let mut boat = floating();
    let start = boat.pos;
    for _ in 0..20 {
        assert_eq!(boat.tick(&Pool, AHEAD), [true, true]);
    }
    assert_eq!(boat.pos[0], start[0]);
    assert!(boat.pos[2] > start[2] + 2.0, "moved {:?}", boat.pos);
}

#[test]
fn left_alone_turns_anticlockwise_on_the_right_paddle() {
    let mut boat = floating();
    assert_eq!(boat.tick(&Pool, LEFT), [false, true]);
    assert_eq!((boat.delta_rotation, boat.yaw), (-1.0, -1.0));
}

#[test]
fn a_boat_on_stone_slides_with_its_friction() {
    let mut boat = Boat::new(&Pool, [30.5, 0.0, 0.5], 0.0, false);
    boat.delta = [0.1, 0.0, 0.0];
    boat.tick(&Pool, Keys::default());
    assert_eq!(boat.status, BoatStatus::OnLand);
    assert!((boat.delta[0] - 0.06).abs() < 1.0E-6, "slid {:?}", boat.delta);
}

#[test]
fn the_rider_turns_with_the_boat_within_105_degrees() {
    let mut boat = floating();
    boat.delta_rotation = 2.0;
    assert_eq!(boat.turn_rider(10.0), 12.0);
    assert_eq!(boat.turn_rider(150.0), 105.0);
    assert_eq!(boat.rider_feet()[1], boat.pos[1] + 0.1875 - 0.6);
}
