use acacia_physics::test_world::{TestBlock, TestWorld};
use acacia_physics::{Input, Outcome, PlayerState, tick};

fn flat() -> TestWorld {
    let mut w = TestWorld::new();
    w.fill(Some(TestBlock::Stone), [-4, -1, -4], [4, -1, 4]);
    w
}

#[test]
fn idle_on_ground_reports_vanilla_delta() {
    let w = flat();
    let mut st = PlayerState::new([0.5, 0.0, 0.5]);
    st.on_ground = true;
    // Tick 1 starts with zero velocity, so only tick 2 collides vertically.
    tick(&mut st, &Input::default(), &w);
    let out = tick(&mut st, &Input::default(), &w);
    assert_eq!(out.delta, [0.0, -0.0784, 0.0]);
    assert!(out.on_ground && out.vertical_collision && !out.horizontal_collision);
    assert_eq!(out.eye_position, [0.5, 1.62, 0.5]);
}

#[test]
fn jump_fires_once_and_reports_it() {
    let w = flat();
    let mut st = PlayerState::new([0.5, 0.0, 0.5]);
    st.on_ground = true;
    let jump = Input { jump: true, ..Input::default() };
    assert!(tick(&mut st, &jump, &w).jumped);
    assert!(!tick(&mut st, &jump, &w).jumped);
}

#[test]
fn teleport_is_consumed_next_tick() {
    let w = flat();
    let mut st = PlayerState::new([0.5, 0.0, 0.5]);
    st.queue_teleport([2.5, 0.0, 2.5]);
    let out = tick(&mut st, &Input::default(), &w);
    assert_eq!(out.outcome, Outcome::Teleport);
    assert!(out.teleported);
    assert_eq!((out.position, out.delta), ([2.5, 0.0, 2.5], [0.0; 3]));
}

#[test]
fn correction_rewinds_state() {
    let w = flat();
    let mut st = PlayerState::new([0.5, 3.0, 0.5]);
    st.apply_correction([1.5, 0.0, 1.5], [0.0, -0.0784, 0.0], true);
    let out = tick(&mut st, &Input::default(), &w);
    assert_eq!(out.position, [1.5, 0.0, 1.5]);
    assert!(out.on_ground);
}
