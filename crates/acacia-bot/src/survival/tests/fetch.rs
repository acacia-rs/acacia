use super::ctx;
use crate::items::{Op, SlotRef};
use crate::state::{GameState, ItemStack};
use crate::survival::auto_eat::{Act, AutoEater, Ctx};
use crate::survival::fetch::{fetch_op, Fetch, FetchCtx, FetchStep};

fn stored(slot: u8) -> Ctx {
    Ctx { stored: Some(slot), ..ctx(true, true, false, None) }
}

#[test]
fn auto_eat_fetches_food_when_the_hotbar_has_none() {
    let mut eater = AutoEater::default();
    let delay = |_| 0;
    assert_eq!(eater.tick(&stored(20), delay), None, "notices");
    assert_eq!(eater.tick(&stored(20), delay), Some(Act::Fetch(20)));
    assert_eq!(eater.tick(&ctx(true, false, false, None), delay), None, "waits for the move, busy or not");
    eater.fetched(true, 0);
    assert_eq!(eater.tick(&ctx(true, true, false, Some(3)), delay), Some(Act::Select(3)), "eats what it moved up");

    let mut eater = AutoEater::default();
    eater.tick(&stored(20), delay);
    eater.tick(&stored(20), delay);
    eater.fetched(false, 0);
    for _ in 0..100 {
        assert_eq!(eater.tick(&stored(20), delay), None, "cooling down after a failed move");
    }
}

fn run(fetch: &mut Fetch, ctx: FetchCtx, ticks: usize) -> Vec<FetchStep> {
    (0..ticks).map(|_| fetch.tick(&ctx, || 2)).collect()
}

#[test]
fn fetch_opens_clicks_waits_and_closes() {
    use FetchStep::{Close, Nothing, Send};
    let opened = FetchCtx { opened: true, ..FetchCtx::default() };
    let mut fetch = Fetch::new();
    assert_eq!(run(&mut fetch, opened, 4), [Nothing, Nothing, Nothing, Send], "open, then a click's pause");
    assert_eq!(run(&mut fetch, opened, 2), [Nothing, Nothing], "waiting for the response");
    assert_eq!(run(&mut fetch, FetchCtx { response: Some(true), ..opened }, 1), [Nothing]);
    assert_eq!(run(&mut fetch, opened, 3), [Nothing, Nothing, Close(true)]);

    let mut fetch = Fetch::new();
    assert_eq!(run(&mut fetch, FetchCtx::default(), 21).last(), Some(&Nothing), "BDS may never confirm the open");
    assert_eq!(run(&mut fetch, FetchCtx::default(), 3).last(), Some(&Send));
    assert_eq!(run(&mut fetch, FetchCtx::default(), 62).last(), Some(&Close(false)), "no response");

    let mut fetch = Fetch::new();
    assert_eq!(run(&mut fetch, FetchCtx { interrupted: true, ..FetchCtx::default() }, 1), [Close(false)], "an action started");
}

#[test]
fn food_goes_to_an_empty_hotbar_slot_else_swaps_with_the_held_one() {
    let item = |id| ItemStack { network_id: id, count: 5, ..ItemStack::default() };
    let mut state = GameState::default();
    state.inventory.main[0] = item(1);
    state.inventory.main[20] = item(2);
    assert_eq!(fetch_op(&state, 20), Op::Transfer { from: SlotRef::Main(20), to: SlotRef::Main(1), count: 5 });
    for slot in 0..9 {
        state.inventory.main[slot] = item(1);
    }
    state.inventory.selected_hotbar_slot = 6;
    assert_eq!(fetch_op(&state, 20), Op::Swap { a: SlotRef::Main(20), b: SlotRef::Main(6) });
}
