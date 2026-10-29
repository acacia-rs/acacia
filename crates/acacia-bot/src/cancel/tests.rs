use super::*;

#[test]
fn a_cancel_ends_only_the_action_in_flight() {
    let mut state = CancelState::default();
    let handle = CancelHandle(state.shared.clone());
    handle.cancel();
    state.begin();
    assert!(!state.requested(), "a cancel while idle is ignored");
    handle.cancel();
    assert!(state.requested());
    state.begin();
    assert!(state.requested(), "every later wait of the cancelled action fails too");
    assert!(state.end());
    assert!(!state.end(), "ending twice is a no-op");
    state.begin();
    assert!(!state.requested(), "the next action runs");
    handle.clone().cancel();
    assert!(state.requested(), "clones share the bot");
}
