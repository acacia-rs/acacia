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
