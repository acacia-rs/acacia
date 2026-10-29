use super::*;
use crate::input::Input;
use crate::sim::tick;
use crate::state::PlayerState;
use crate::test_world::{TestBlock, TestWorld};

fn floor() -> TestWorld {
    let mut w = TestWorld::new();
    w.fill(Some(TestBlock::Stone), [-4, 19, -4], [12, 19, 8]);
    w
}

/// A resting boat as strict BDS reported it in drill `entity_drill`: wire (4.7, 20.375, z), box 1.4 × 0.455.
fn boat(z: f32) -> Aabb {
    Aabb::new(4.7 - 0.7, 20.0, z - 0.7, 4.7 + 0.7, 20.455, z + 0.7)
}

fn walk(yaw: f32, boxes: &[Aabb], ticks: usize) -> Vec<[f32; 3]> {
    let w = floor();
    let world = WithEntities { world: &w, boxes };
    let mut st = PlayerState::new([0.5, 20.0, 0.5]);
    st.on_ground = true;
    let input = Input { move_vector: [0.0, 1.0], yaw, ..Input::default() };
    (0..ticks).map(|_| tick(&mut st, &input, &world).position).collect()
}

#[test]
fn walking_into_a_boat_steps_onto_it() {
    let path = walk(-90.0, &[boat(0.7)], 60);
    let on = path.iter().position(|p| p[1] > 20.0).expect("stepped up");
    assert!((path[on][1] - 20.455).abs() < 1e-6 && path[on][0] + 0.3 > 4.0 - 0.12, "{:?}", path[on]);
    assert!(path.iter().all(|p| p[1] <= 20.455 + 1e-6));
}

#[test]
fn no_boxes_walks_through() {
    assert!(walk(-90.0, &[], 60).iter().all(|p| p[1] == 20.0));
}

#[test]
fn offaxis_step_onto_a_boat() {
    let path = walk(-70.0, &[boat(1.7)], 40);
    for (i, p) in path.iter().enumerate() {
        println!("{i} {p:?}");
    }
    let on = path.iter().position(|p| p[1] > 20.0).expect("stepped up");
    // Strict BDS stepped up on the tick whose box first reached x 4.0 (drill: x 3.7043629, z 1.6662927).
    assert_eq!(path[on - 1][0] + 0.3 < 4.0, true, "{:?} then {:?}", path[on - 1], path[on]);
}
