use acacia_client::proto::types::{Vec2f, Vec3f};

use super::*;
use crate::state::queries::test_support::raw;

const ME: Me = Me { runtime_entity_id: 1, unique_entity_id: -1 };

fn correction(prediction_type: CorrectPlayerMovePredictionPredictionType, y: f32) -> RawPacket {
    raw(&CorrectPlayerMovePrediction {
        prediction_type,
        position: Vec3f { x: 0.5, y, z: 0.5 },
        delta: Vec3f { x: 0.0, y: 0.0, z: 0.0 },
        rotation: Vec2f { x: 0.0, z: 0.0 },
        angular_velocity: None,
        on_ground: true,
        tick: 0,
    })
}

#[test]
fn vehicle_corrections_do_not_move_the_player() {
    let mut m = Movement::new();
    m.start([0.5, -60.0, 0.5], 0.0, 0.0);
    // BDS corrects a driven boat with its own position; as a player position it is 1.62 too low.
    m.apply(&correction(CorrectPlayerMovePredictionPredictionType::Vehicle, -59.625), &ME).unwrap();
    assert_eq!(m.position(), Some([0.5, -60.0, 0.5]));
    m.apply(&correction(CorrectPlayerMovePredictionPredictionType::Player, -57.0), &ME).unwrap();
    assert_eq!(m.position().map(|p| p[1]), Some(-57.0 - EYE_HEIGHT));
}

#[test]
fn a_dimension_change_stops_the_simulation_until_it_starts_again_standing() {
    let world = acacia_physics::test_world::TestWorld::new();
    let mut m = Movement::new();
    m.start([0.5, -60.0, 0.5], 0.0, 0.0);
    let before = m.tick(&world).unwrap().tick;
    let change = ChangeDimension { dimension: 1, position: Vec3f { x: 30.5, y: 70.0, z: 30.5 }, respawn: false, loading_screen_id: Some(0) };
    m.apply(&raw(&change), &ME).unwrap();
    assert!(!m.is_started() && m.tick(&world).is_none(), "no input from the dimension left");
    // A correction for the old dimension's last inputs must not move the next simulation.
    m.apply(&correction(CorrectPlayerMovePredictionPredictionType::Player, -57.0), &ME).unwrap();
    m.start([30.5, 70.0, 30.5], 0.0, 0.0);
    assert!(m.on_ground(), "BDS carries the standing state over");
    let input = m.tick(&world).unwrap();
    assert_eq!(input.tick, before + 1, "the input tick count goes on");
    // In mid-air the first input falls one tick of gravity, as the server's does.
    let (y, fall) = (input.position.y - EYE_HEIGHT, input.delta.y);
    assert!((y - 69.9216).abs() < 1e-4 && (fall + 0.155232).abs() < 1e-6, "feet {y}, delta {fall}");
}
