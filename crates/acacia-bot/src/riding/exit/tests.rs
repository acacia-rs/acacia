use acacia_physics::test_world::{TestBlock, TestWorld};

use super::*;

/// Grass floor at y 63 around the origin, open air above.
fn flat() -> TestWorld {
    let mut w = TestWorld::new();
    w.fill(Some(TestBlock::Stone), [-5, 63, -5], [5, 63, 5]);
    w
}

fn mount(vehicle: Vec3) -> Mount {
    Mount { vehicle, yaw: 0.0, motion: [0.0; 2], seat_offset: [0.0, 0.63, 0.0] }
}

fn close(a: Vec3, b: Vec3) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-4)
}

#[test]
fn standing_vehicle_tries_minus_z_first() {
    let feet = mount([1.5, 64.0, 1.5]).exit_feet(&flat()).unwrap();
    assert!(close(feet, [1.5, 64.001, 0.5]), "{feet:?}");
}

#[test]
fn search_order_follows_the_vehicle_s_motion() {
    assert_eq!(horizontal_offsets([0.0, 0.0])[..2], [[0.0, -1.0], [0.0, 1.0]]);
    // Moving +Z: forward (0,1), side (-1,0); then s-f, -s-f, s+f, f-s, -f, f.
    let order = horizontal_offsets([0.02, 0.3]);
    assert_eq!(order, [[-1.0, 0.0], [1.0, 0.0], [-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0], [1.0, 1.0], [0.0, -1.0], [0.0, 1.0]]);
    assert_eq!(horizontal_offsets([-0.5, 0.1])[0], [0.0, -1.0], "moving -X: side (0, -1)");
}

#[test]
fn blocked_spots_are_skipped_and_slabs_raise_the_feet() {
    let mut w = flat();
    w.fill(Some(TestBlock::Stone), [1, 64, 0], [1, 65, 0]);
    w.set([1, 64, 2], TestBlock::Slab);
    let feet = mount([1.5, 64.0, 1.5]).exit_feet(&w).unwrap();
    assert!(close(feet, [1.5, 64.501, 2.5]), "{feet:?}");
}

#[test]
fn no_floor_or_no_headroom_is_not_a_spot() {
    let mut w = TestWorld::new();
    w.set([1, 63, 1], TestBlock::Stone);
    // Only the vehicle's own block has a floor, and no offset is (0, 0).
    assert_eq!(mount([1.5, 64.0, 1.5]).exit_feet(&w), None);

    let mut w = flat();
    w.fill(Some(TestBlock::Stone), [-5, 65, -5], [5, 65, 5]);
    // One block of headroom: no spot at dy 0, and dy +1 / -1 are inside the ceiling / floor.
    assert_eq!(mount([1.5, 64.0, 1.5]).exit_feet(&w), None);
}

#[test]
fn seat_offset_turns_with_the_vehicle_and_x_z_start_from_the_seat() {
    let m = Mount { vehicle: [0.5, 64.0, 0.5], yaw: 90.0, motion: [0.0; 2], seat_offset: [0.0, 0.2, 0.6] };
    let seat = m.seat();
    // Yaw 90 faces -X: the offset's +Z (forward) becomes -X.
    assert!(close(seat, [-0.1, 64.2, 0.5]), "{seat:?}");
    let feet = m.exit_feet(&flat()).unwrap();
    assert!(close(feet, [-0.1, 64.001, -0.5]), "{feet:?}");
}
