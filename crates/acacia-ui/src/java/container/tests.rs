use serde_json::json;

use super::beacon::{Beacon, BeaconPress};
use super::input::{Button, Click, Input, Key, Pointer, Press, key_pressed, mouse_clicked, mouse_dragged, mouse_released};
use super::item_combiner::Station;
use super::merchant::{Merchant, Offer};
use super::recipe_book::input::{BookPress, RecipePress};
use super::recipe_book::screens::{RecipeScreen, Window};
use super::recipe_book::{Book, Collection, Entry, Kind, Search, Tab};
use super::simple::{Kind as Simple, Simple as Chest};
use super::stonecutter::Stonecutter;
use super::{Env, Screen};
use crate::java::gui::Fixed;
use crate::java::text::Text;
use crate::java::tooltip::game::Game;
use crate::java::tooltip::stack::ItemStack;

fn stack(id: &str, count: i32) -> ItemStack {
    let components = json!({"minecraft:max_stack_size": 64}).as_object().cloned().unwrap_or_default();
    ItemStack { id: id.into(), count, size: 1, components, key: None }
}

fn chest() -> Chest {
    Chest::new(Simple::Chest { rows: 3 }, 400, 300, Text::default(), Text::default(), &Fixed(6.0))
}

/// The middle of slot `i` of `s`, in screen pixels.
fn over<S: Screen>(s: &S, i: usize) -> [f64; 2] {
    let (b, slot) = (s.base(), &s.base().slots[i]);
    [f64::from(b.left + slot.x + 8), f64::from(b.top + slot.y + 8)]
}

#[test]
fn a_press_with_an_empty_hand_clicks_the_slot_and_skips_its_release() {
    let mut s = chest();
    s.base.slots[3].item = Some(stack("minecraft:stone", 5));
    let mut p = Pointer::default();
    let at = over(&s, 3);
    let click = |input| vec![Click::Slot { slot: Some(3), outside: false, button: 0, input }];
    assert_eq!(mouse_clicked(&s, &mut p, at, Button::Left, false, false, false), click(Input::Pickup));
    assert!(mouse_released(&s, &mut p, at, Button::Left, false, false).is_empty());
    assert_eq!(mouse_clicked(&s, &mut p, at, Button::Left, true, false, false), click(Input::QuickMove));
    let outside = mouse_clicked(&s, &mut p, [1.0, 1.0], Button::Right, false, false, false);
    assert_eq!(outside, vec![Click::Slot { slot: None, outside: true, button: 1, input: Input::Throw }]);
}

#[test]
fn a_drag_with_a_stack_held_quick_crafts_over_the_slots_it_crossed() {
    let mut s = chest();
    s.base.carried = Some(stack("minecraft:stone", 10));
    s.base.slots[1].item = Some(stack("minecraft:dirt", 1));
    let mut p = Pointer::default();
    assert!(mouse_clicked(&s, &mut p, over(&s, 0), Button::Left, false, false, false).is_empty());
    for i in [0, 1, 2, 2] {
        mouse_dragged(&s, &mut p, over(&s, i));
    }
    assert_eq!(p.quick().map(|q| q.slots), Some(vec![0, 2]), "a slot holding another item is passed over");
    let done = mouse_released(&s, &mut p, over(&s, 2), Button::Left, false, false);
    assert_eq!(done, vec![Click::QuickCraft { kind: 0, slots: vec![0, 2] }]);
    assert!(p.quick().is_none());
    // Without a drag the release puts the stack down where it is.
    mouse_clicked(&s, &mut p, over(&s, 5), Button::Right, false, false, false);
    let put = mouse_released(&s, &mut p, over(&s, 5), Button::Right, false, false);
    assert_eq!(put, vec![Click::Slot { slot: Some(5), outside: false, button: 1, input: Input::Pickup }]);
    // Another button's release cancels the drag.
    mouse_clicked(&s, &mut p, over(&s, 0), Button::Left, false, false, false);
    mouse_dragged(&s, &mut p, over(&s, 0));
    assert!(mouse_released(&s, &mut p, over(&s, 0), Button::Right, false, false).is_empty());
}

#[test]
fn keys_swap_with_the_hotbar_and_drop_over_the_hovered_slot() {
    let mut s = chest();
    s.base.slots[4].item = Some(stack("minecraft:stone", 5));
    let at = over(&s, 4);
    assert_eq!(key_pressed(&s, at, Key::Hotbar(2)), Some(Click::Slot { slot: Some(4), outside: false, button: 2, input: Input::Swap }));
    assert_eq!(key_pressed(&s, at, Key::Drop { all: true }), Some(Click::Slot { slot: Some(4), outside: false, button: 1, input: Input::Throw }));
    assert_eq!(key_pressed(&s, over(&s, 5), Key::Drop { all: false }), None);
}

#[test]
fn the_stonecutter_picks_a_recipe_and_grabs_its_scroller() {
    let mut s = Stonecutter::new(400, 300, Text::default(), Text::default());
    s.base.slots[0].item = Some(stack("minecraft:stone", 1));
    s.recipes = (0..20).map(|i| (stack("minecraft:stone_slab", 2), 100 + i)).collect();
    s.container_changed();
    let (left, top) = (f64::from(s.base.left), f64::from(s.base.top));
    assert_eq!(s.mouse_clicked([left + 52.0 + 16.0 + 1.0, top + 14.0 + 18.0 + 1.0]), Some(Press::Button(5)));
    assert_eq!(s.mouse_clicked([left + 120.0, top + 20.0]), Some(Press::Scroller));
    s.mouse_dragged([left + 200.0, top + 200.0]);
    assert_eq!(s.scroll_offs(), 1.0);
    assert_eq!(s.mouse_clicked([left + 53.0, top + 15.0]), Some(Press::Button(8)), "two rows down: the list ends at its last full row");
}

#[test]
fn the_beacon_chooses_powers_and_confirms() {
    let station = Station { data: vec![4], ..Station::default() };
    let mut b = Beacon::new(400, 300, Text::default(), Text::default(), &station, None, None, true);
    let (left, top) = (f64::from(b.base.left), f64::from(b.base.top));
    let speed = [left + 76.0 - 23.0 + 5.0, top + 22.0 + 5.0];
    assert_eq!(b.mouse_clicked(speed), Some(BeaconPress::Chose));
    assert_eq!(b.primary.as_deref(), Some("speed"));
    let upgrade = [left + 167.0 + 24.0 - 23.0 + 5.0, top + 47.0 + 5.0];
    assert_eq!(b.mouse_clicked(upgrade), Some(BeaconPress::Chose));
    assert_eq!(b.secondary.as_deref(), Some("speed"));
    assert_eq!(b.mouse_clicked([left + 170.0, top + 110.0]), Some(BeaconPress::Confirm));
}

#[test]
fn merchant_offers_answer_with_their_index_past_the_scroll() {
    let mut m = Merchant::new(400, 300, Text::default(), Text::default());
    m.offers = (0..9).map(|_| Offer { base_cost_a: stack("minecraft:emerald", 1), result: stack("minecraft:bread", 1), max_uses: 1, ..Offer::default() }).collect();
    m.scroll(-5.0);
    assert_eq!(m.scroll_off, 2);
    let (left, top) = (f64::from(m.base.left), f64::from(m.base.top));
    assert_eq!(m.mouse_clicked([left + 10.0, top + 18.0 + 20.0 + 1.0]), Some(Press::Button(3)));
    assert_eq!(m.mouse_clicked([left + 96.0, top + 30.0]), Some(Press::Scroller));
}

fn book(visible: bool) -> Book {
    let entry = |craftable| Entry { selected: true, craftable, display: vec![7], cells: Vec::new() };
    let collections: Vec<Collection> = (0..25).map(|i| Collection { entries: vec![entry(i % 2 == 0), entry(true)] }).collect();
    let tabs = vec![Tab { search: true, icons: vec![1], collections: (0..25).collect(), anim: 0.0 }, Tab { search: false, icons: vec![2], collections: vec![0], anim: 0.0 }];
    Book {
        kind: Kind::Crafting,
        visible,
        narrow: false,
        width: 500,
        height: 300,
        filtering: false,
        time: 0.0,
        tabs,
        selected_tab: 0,
        collections,
        page: 0,
        button_anims: Vec::new(),
        search: Search::default(),
        overlay: None,
        ghost: Vec::new(),
        items: Default::default(),
        hovered: None,
    }
}

#[test]
fn the_recipe_book_turns_pages_switches_tabs_and_places_recipes() {
    let game = Game::default();
    let lang = |_: &str| -> Option<String> { None };
    let env = Env { font: &Fixed(6.0), lang: &lang, advanced: false, game: &game, op: false };
    let window = Window::Inventory { figure: super::entity::Figure { entity: "minecraft:player".into(), bb_height: 1.8, scale: Some(1.0), fall_flying: false }, last_mouse: [0.0; 2], effects: Vec::new() };
    let mut s = RecipeScreen::new(book(true), window, Text::default(), Text::default(), &env);
    let [xo, yo] = s.book.origin().map(f64::from);
    let forward = [xo + 95.0, yo + 140.0];
    assert_eq!(s.mouse_clicked(forward, Button::Left), Some(RecipePress::Book(BookPress::Changed)));
    assert_eq!(s.book.page, 1);
    let first = [xo + 12.0, yo + 32.0];
    assert_eq!(s.mouse_clicked(first, Button::Left), Some(RecipePress::Book(BookPress::Recipe { collection: 20, entry: 0 })));
    assert_eq!(s.mouse_clicked(first, Button::Right), Some(RecipePress::Book(BookPress::Changed)));
    assert_eq!(s.book.overlay, Some(0));
    assert_eq!(s.mouse_clicked([1.0, 1.0], Button::Left), Some(RecipePress::Book(BookPress::Changed)), "a click off the overlay closes it");
    assert_eq!(s.book.overlay, None);
    let tabs = s.book.tab_rects();
    let second = [f64::from(tabs[1].1[0] + 2), f64::from(tabs[1].1[1] + 2)];
    s.mouse_clicked(second, Button::Left);
    assert_eq!((s.book.selected_tab, s.book.page), (1, 0));
    assert!(!s.clicked_outside(first), "on the book is not off the window");
}

#[test]
fn the_book_button_shows_the_book_and_moves_the_window() {
    let game = Game::default();
    let lang = |_: &str| -> Option<String> { None };
    let env = Env { font: &Fixed(6.0), lang: &lang, advanced: false, game: &game, op: false };
    let mut s = RecipeScreen::new(book(false), Window::Crafting, Text::default(), Text::default(), &env);
    let before = s.base.left;
    let [x, y, ..] = s.button().map(f64::from);
    assert_eq!(s.mouse_clicked([x + 1.0, y + 1.0], Button::Left), Some(RecipePress::Toggled));
    assert!(s.book.visible && s.base.left > before);
}
