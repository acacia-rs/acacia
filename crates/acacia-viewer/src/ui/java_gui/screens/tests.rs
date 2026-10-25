use std::path::Path;
use std::time::Instant;

use acacia_bot::items::{Click as BotClick, SlotRef};
use acacia_ui::inventory::{Bench, SEARCH_TAB, Station};
use acacia_ui::java::container::input::{Button, Key};
use acacia_ui::lang::Lang;

use super::super::items::{item_stack, plain};
use super::super::items::JavaLang;
use super::{Event, JavaGui, Open, Outcome, State, Target, View};
use crate::control::{Command, Inventory, Rows, Stack};
use crate::creative_items::CreativeEntry;

/// `Command` has no `PartialEq`: compared by its debug form.
macro_rules! same {
    ($a:expr, $b:expr $(, $m:expr)?) => {
        assert_eq!(format!("{:?}", $a), format!("{:?}", $b) $(, $m)?)
    };
}

fn gui() -> JavaGui {
    JavaGui::new(Path::new("no-look"))
}

fn stack(name: &str, count: u16) -> Stack {
    Stack { count, ..plain(name) }
}

fn inventory() -> Inventory {
    Inventory { main: vec![None; 36], ..Inventory::default() }
}

fn chest(slots: usize) -> Inventory {
    Inventory { container: Some(Rows { title: String::new(), slots: vec![None; slots] }), ..inventory() }
}

fn view(inventory: &Inventory) -> View<'_> {
    View { inventory, creative: None, entries: &[], name: (String::new(), 0), me: None, size: [427, 240], mouse: [0.0; 2], now: Instant::now() }
}

/// The middle of slot `i` of the screen `view` shows.
fn slot(gui: &JavaGui, view: &View, state: &State, i: usize) -> [f64; 2] {
    let mut built = gui.build(None, &Lang::default(), view, state);
    let base = built.open.screen().base();
    [f64::from(base.left + base.slots[i].x + 8), f64::from(base.top + base.slots[i].y + 8)]
}

fn at(gui: &JavaGui, view: &mut View, state: &mut State, mouse: [f64; 2], event: Event) -> Outcome {
    view.mouse = mouse;
    gui.event(None, &Lang::default(), view, state, event)
}

fn press(button: Button, shift: bool) -> Event {
    Event::Press { button, shift, double: false }
}

#[test]
fn a_chest_lists_its_slots_then_the_players() {
    let (gui, mut inv, state) = (gui(), chest(27), State::default());
    inv.block = Some("minecraft:barrel");
    inv.container.as_mut().unwrap().slots[4] = Some(stack("minecraft:apple", 3));
    let built = gui.build(None, &Lang::default(), &view(&inv), &state);
    assert!(matches!(built.open, Open::Simple(_)));
    assert_eq!((&built.targets[0], &built.targets[27], &built.targets[54]), (&Target::Slot(SlotRef::Container(0)), &Target::Slot(SlotRef::Main(9)), &Target::Slot(SlotRef::Main(0))));
    assert_eq!(built.keys.get(&4).map(|s| s.count), Some(3));
}

#[test]
fn presses_click_and_shift_click_the_slot() {
    let (gui, inv, mut state) = (gui(), chest(27), State::default());
    let mut v = view(&inv);
    let at_slot = slot(&gui, &v, &state, 2);
    let left = at(&gui, &mut v, &mut state, at_slot, press(Button::Left, false));
    same!(left.commands, vec![Command::Click(SlotRef::Container(2), BotClick::Left)]);
    same!(at(&gui, &mut v, &mut state, at_slot, Event::Release { button: Button::Left, shift: false }).commands, Vec::<Command>::new(), "the release after a pickup is skipped");
    let shift = at(&gui, &mut v, &mut state, at_slot, press(Button::Right, true));
    same!(shift.commands, vec![Command::Click(SlotRef::Container(2), BotClick::Shift)]);
}

#[test]
fn a_drag_splits_the_carried_stack_with_right_clicks() {
    let (gui, mut inv, mut state) = (gui(), chest(27), State::default());
    inv.cursor = Some(stack("minecraft:apple", 7));
    let mut v = view(&inv);
    let (a, b) = (slot(&gui, &v, &state, 0), slot(&gui, &v, &state, 1));
    assert!(at(&gui, &mut v, &mut state, a, press(Button::Left, false)).commands.is_empty());
    at(&gui, &mut v, &mut state, a, Event::Move);
    at(&gui, &mut v, &mut state, b, Event::Move);
    let done = at(&gui, &mut v, &mut state, b, Event::Release { button: Button::Left, shift: false });
    let right = |i| Command::Click(SlotRef::Container(i), BotClick::Right);
    same!(done.commands, vec![right(0), right(0), right(0), right(1), right(1), right(1)]);
}

#[test]
fn keys_and_outside_clicks_are_made_of_clicks() {
    let (gui, mut inv, mut state) = (gui(), chest(27), State::default());
    inv.container.as_mut().unwrap().slots[5] = Some(stack("minecraft:apple", 2));
    let mut v = view(&inv);
    let s = slot(&gui, &v, &state, 5);
    let swap = at(&gui, &mut v, &mut state, s, Event::Key(Key::Hotbar(3)));
    let left = |r| Command::Click(r, BotClick::Left);
    same!(swap.commands, vec![left(SlotRef::Container(5)), left(SlotRef::Main(3)), left(SlotRef::Container(5))]);
    let drop = at(&gui, &mut v, &mut state, s, Event::Key(Key::Drop { all: false }));
    same!(drop.commands[1], Command::DropCursor { one: true });
    inv.cursor = Some(stack("minecraft:apple", 2));
    let mut v = view(&inv);
    let out = at(&gui, &mut v, &mut state, [1.0, 1.0], press(Button::Right, false));
    assert!(out.commands.is_empty(), "a press while carrying starts a drag");
    let out = at(&gui, &mut v, &mut state, [1.0, 1.0], Event::Release { button: Button::Right, shift: false });
    same!(out.commands, vec![Command::DropCursor { one: true }]);
}

#[test]
fn brewing_and_bench_slots_take_javas_order() {
    let (gui, mut inv, state) = (gui(), chest(5), State::default());
    inv.station = Some(Station::Brewing);
    let built = gui.build(None, &Lang::default(), &view(&inv), &state);
    let first: Vec<_> = built.targets[..5].to_vec();
    assert_eq!(first, [1, 2, 3, 0, 4].map(|i| Target::Slot(SlotRef::Container(i))).to_vec());
    let mut inv = inventory();
    inv.bench = Some(Bench::Smithing);
    let built = gui.build(None, &Lang::default(), &view(&inv), &state);
    assert_eq!(built.targets[..4].to_vec(), vec![Target::Slot(SlotRef::Ui(53)), Target::Slot(SlotRef::Ui(51)), Target::Slot(SlotRef::Ui(52)), Target::Crafted]);
}

#[test]
fn the_stonecutters_buttons_pick_what_its_result_cuts() {
    let (gui, mut inv, mut state) = (gui(), inventory(), State::default());
    inv.bench = Some(Bench::Stonecutter);
    inv.ui = vec![None; 60];
    inv.ui[3] = Some(stack("minecraft:stone", 4));
    inv.picks = vec![(7, stack("minecraft:stone_slab", 2)), (9, stack("minecraft:stone_stairs", 1))];
    let mut v = view(&inv);
    let mut built = gui.build(None, &Lang::default(), &v, &state);
    let b = built.open.screen().base();
    let second = [f64::from(b.left + 52 + 16 + 4), f64::from(b.top + 14 + 4)];
    at(&gui, &mut v, &mut state, second, press(Button::Left, false));
    assert_eq!(state.pick, Some(1));
    let result = slot(&gui, &v, &state, 1);
    let take = at(&gui, &mut v, &mut state, result, press(Button::Left, true));
    same!(take.commands, vec![Command::TakeCut { id: 9, all: true }]);
}

#[test]
fn creative_items_come_out_of_the_entries_and_tabs_select_on_release() {
    let (gui, inv, mut state) = (gui(), inventory(), State::default());
    let entries = [CreativeEntry { id: 41, stack: stack("minecraft:stone", 1), tab: 0 }];
    let mut v = View { creative: Some(0), entries: &entries, ..view(&inv) };
    let item = slot(&gui, &v, &state, 0);
    let out = at(&gui, &mut v, &mut state, item, press(Button::Left, false));
    same!(out.commands, vec![Command::Creative { id: 41, count: 1 }]);
    at(&gui, &mut v, &mut state, item, Event::Release { button: Button::Left, shift: false });
    let mut built = gui.build(None, &Lang::default(), &v, &state);
    let b = built.open.screen().base();
    // The search tab: top row, the last column on the right.
    let tab = [f64::from(b.left + b.image_width - 27 + 1 + 10), f64::from(b.top - 20)];
    at(&gui, &mut v, &mut state, tab, press(Button::Left, false));
    let out = at(&gui, &mut v, &mut state, tab, Event::Release { button: Button::Left, shift: false });
    assert_eq!(out.tab, Some(SEARCH_TAB));
}

#[test]
fn stacks_carry_their_java_item_name_and_enchantments() {
    let lang = JavaLang::from([("block.minecraft.stone".to_owned(), "Stone".to_owned())]);
    let s = item_stack(&stack("minecraft:stone", 2), None, &lang, 5);
    assert_eq!((s.count, s.key, s.get("item_name").cloned()), (2, Some(5), Some(serde_json::json!({"translate": "block.minecraft.stone"}))));
    let book = Stack { enchantments: vec![(9, 3)], ..stack("minecraft:enchanted_book", 1) };
    let s = item_stack(&book, None, &lang, 0);
    assert_eq!(s.get("stored_enchantments").cloned(), Some(serde_json::json!({"minecraft:sharpness": 3})));
}
