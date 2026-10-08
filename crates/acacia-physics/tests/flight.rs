use acacia_physics::test_world::{TestBlock, TestWorld};
use acacia_physics::{Input, PlayerState, tick};

fn world() -> TestWorld {
    let mut w = TestWorld::new();
    w.fill(Some(TestBlock::Stone), [-8, -1, -8], [8, -1, 8]);
    w
}

fn flyer(at: [f32; 3], creative: bool) -> PlayerState {
    let mut st = PlayerState::new(at);
    st.flight.may_fly = true;
    st.flight.creative = creative;
    (st.flight.flying, st.flight.travel) = (true, true);
    st
}

const IDLE: Input = Input {
    move_vector: [0.0, 0.0], yaw: 0.0, pitch: 0.0, jump: false, sneak: false, want_down: false, sprint: false, swim: false,
    glide: false, using_item: false,
};

#[test]
fn double_tap_toggles_flight() {
    let w = world();
    let mut st = PlayerState::new([0.5, 0.0, 0.5]);
    st.flight.may_fly = true;
    let jump = Input { jump: true, ..IDLE };
    for (input, flying) in [(jump, false), (IDLE, false), (jump, true), (IDLE, true), (jump, false)] {
        tick(&mut st, &input, &w);
        assert_eq!(st.flight.flying, flying);
    }
}

#[test]
fn taps_too_far_apart_or_without_rights_do_nothing() {
    let w = world();
    let mut st = PlayerState::new([0.5, 0.0, 0.5]);
    let jump = Input { jump: true, ..IDLE };
    for input in [jump, IDLE, jump] {
        tick(&mut st, &input, &w);
    }
    assert!(!st.flight.flying);
    st.flight.may_fly = true;
    tick(&mut st, &IDLE, &w);
    tick(&mut st, &jump, &w);
    (0..7).for_each(|_| _ = tick(&mut st, &IDLE, &w));
    tick(&mut st, &jump, &w);
    assert!(!st.flight.flying);
}

/// Strict BDS: the toggle tick takes the flight's vertical input with the walking travel's gravity and drag,
/// and the tick a flight stops keeps the flight's drag without its vertical input.
#[test]
fn travel_follows_the_toggle_a_tick_late() {
    let w = world();
    let mut st = PlayerState::new([0.5, 5.0, 0.5]);
    st.flight.may_fly = true;
    st.flight.trigger_ticks = 3;
    st.pressing_jump = false;
    st.vel = [0.0, 0.248136, 0.0];
    let on = tick(&mut st, &Input { jump: true, ..IDLE }, &w);
    assert!(st.flight.flying);
    assert_eq!(on.delta[1], ((0.15 + 0.248136) - 0.08) * 0.98);
    st.flight.trigger_ticks = 3;
    tick(&mut st, &IDLE, &w);
    let v = st.vel[1];
    let off = tick(&mut st, &Input { jump: true, ..IDLE }, &w);
    assert!(!st.flight.flying);
    assert_eq!(off.delta[1], v * 0.6);
}

#[test]
fn rises_without_gravity() {
    let (w, mut st) = (world(), flyer([0.5, 0.0, 0.5], true));
    let input = Input { jump: true, ..IDLE };
    let first = tick(&mut st, &input, &w);
    assert!(!first.jumped, "a flyer's jump key rises instead");
    assert_eq!(first.delta[1], 0.15 * 0.6);
    let second = tick(&mut st, &input, &w);
    assert_eq!(second.delta[1], (0.15 + 0.15 * 0.6) * 0.6);
}

#[test]
fn creative_hover_brakes_vertically() {
    let (w, mut st) = (world(), flyer([0.5, 5.0, 0.5], true));
    st.vel = [0.0, 0.2, 0.0];
    let out = tick(&mut st, &IDLE, &w);
    assert_eq!(out.delta[1], 0.2 * 0.375 * 0.6);
}

#[test]
fn sneak_descends_without_crouching() {
    let (w, mut st) = (world(), flyer([0.5, 5.0, 0.5], true));
    let out = tick(&mut st, &Input { sneak: true, want_down: true, ..IDLE }, &w);
    assert_eq!(out.delta[1], -0.22 * 0.6);
    assert!(!st.sneaking);
}

#[test]
fn sprint_doubles_the_fly_speed() {
    let (w, mut st) = (world(), flyer([0.5, 5.0, 0.5], false));
    // Yaw -90 faces +X.
    let out = tick(&mut st, &Input { sprint: true, move_vector: [0.0, 1.0], yaw: -90.0, ..IDLE }, &w);
    assert!(st.sprinting);
    assert!((out.delta[0] - 0.05 * 2.0 * 0.98 * 0.91).abs() < 1e-6, "{:?}", out.delta);
    assert_eq!(out.delta[1], 0.0);
}
