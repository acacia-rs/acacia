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

/// `floor` at y -1; `ahead` at y 0 where x >= 2.
struct Field {
    floor: Block,
    ahead: Block,
}

impl Blocks for Field {
    fn state(&self, x: i32, y: i32, _: i32) -> Option<u32> {
        let block = match y {
            -1 => self.floor,
            0 if x >= 2 => self.ahead,
            _ => Block::Air,
        };
        Some(block.default_state().id())
    }

    fn min_y(&self) -> i32 {
        -64
    }
}

#[test]
fn a_cobweb_holds_the_boat_on_its_next_move() {
    let web = Field { floor: Block::Stone, ahead: Block::Cobweb };
    let mut boat = Boat::new(&web, [0.5, 0.0, 0.5], 0.0, false);
    boat.on_ground = true;
    boat.delta = [2.0, 0.0, 0.0];
    boat.tick(&web, Keys::default());
    assert_eq!(boat.stuck, [0.25, 0.05_f32 as f64, 0.25]);
    let (x, dx) = (boat.pos[0], boat.delta[0]);
    boat.tick(&web, Keys::default());
    assert_eq!(boat.stuck, [0.25, 0.05_f32 as f64, 0.25]);
    assert!((boat.pos[0] - x - dx * 0.6 * 0.25).abs() < 1.0E-6,"moved {} from {x}", boat.pos[0]);
    assert_eq!(boat.delta[0], 0.0);
}

#[test]
fn slime_scales_a_grounded_boat_once_per_effects_pass() {
    let slime = Field { floor: Block::SlimeBlock, ahead: Block::Air };
    let mut boat = Boat::new(&slime, [0.5, 0.0, 0.5], 0.0, false);
    boat.tick(&slime, Keys::default());
    boat.delta = [0.1, 0.0, 0.0];
    boat.tick(&slime, Keys::default());
    let friction = boat.land_friction as f64 * 2.0;
    assert!((boat.delta[0] - 0.1 * friction * 0.4 * 0.4).abs() < 1.0E-9, "slid {:?} at friction {friction}", boat.delta);
}

#[test]
fn a_teleport_moves_the_boat_over_three_ticks() {
    let mut boat = floating();
    let start = boat.pos;
    boat.interpolate_to([start[0] + 3.0, start[1], start[2]], 90.0, 0.0);
    let mut xs = Vec::new();
    for _ in 0..4 {
        boat.tick(&Pool, Keys::default());
        xs.push(boat.pos[0] - start[0]);
    }
    assert_eq!(xs, [1.0, 2.0, 3.0, 3.0]);
    assert_eq!(boat.yaw, 90.0);
}

#[test]
fn the_rider_turns_with_the_boat_within_105_degrees() {
    let mut boat = floating();
    boat.delta_rotation = 2.0;
    assert_eq!(boat.turn_rider(10.0), 12.0);
    assert_eq!(boat.turn_rider(150.0), 105.0);
    assert_eq!(boat.rider_feet()[1], boat.pos[1] + 0.1875 - 0.6);
}
