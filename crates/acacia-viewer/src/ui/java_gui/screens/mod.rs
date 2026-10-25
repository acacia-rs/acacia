//! The Java look's inventory and container screens: the viewer's `control::Inventory` (and the
//! creative items) mapped each frame onto `acacia_ui::java::container`'s screens with what they
//! keep between frames ([`State`]), drawn through `JavaGui::paint`, and asked where input goes
//! (`input.rs`).

mod benches;
mod build;
mod commands;
mod containers;
mod creative;
mod input;
mod player;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::time::Instant;

use acacia_bot::items::SlotRef;
use acacia_ui::DrawList;
use acacia_ui::java::container::anvil::Anvil;
use acacia_ui::java::container::beacon::Beacon;
use acacia_ui::java::container::brewing::Brewing;
use acacia_ui::java::container::cartography::Cartography;
use acacia_ui::java::container::crafter::Crafter;
use acacia_ui::java::container::creative::Creative;
use acacia_ui::java::container::enchanting::Enchanting;
use acacia_ui::java::container::grindstone::Grindstone;
use acacia_ui::java::container::input::{Button, Pointer};
use acacia_ui::java::container::loom::Loom;
use acacia_ui::java::container::merchant::Merchant;
use acacia_ui::java::container::recipe_book::screens::RecipeScreen;
use acacia_ui::java::container::simple::Simple;
use acacia_ui::java::container::smithing::Smithing;
use acacia_ui::java::container::stonecutter::Stonecutter;
use acacia_ui::java::container::{Screen, frame};
use acacia_ui::java::gui::Gui;
use acacia_ui::widget::TextEdit;

pub use build::View;
pub use input::{Event, Outcome};

use super::{JavaGui, PictureRequest};
use crate::control::Stack;
use crate::creative_items::CreativeEntry;
use crate::ui::Skin;

/// What the app keeps for the Java screens beside [`State`]: the creative items, the creative
/// tab showing, and the anvil's name or the creative search being typed.
#[derive(Clone, Copy)]
pub struct JavaScreen<'a> {
    pub entries: &'a [CreativeEntry],
    pub creative: Option<usize>,
    pub name: &'a TextEdit,
}

/// What the Java screens keep between frames and events.
#[derive(Default)]
pub struct State {
    pub pointer: Pointer,
    /// The mouse button held, for drags.
    pub held: Option<Button>,
    /// A list's scroller is held until the release.
    pub scrolling: bool,
    /// `scrollOffs` of the stonecutter's, loom's or creative list.
    pub scroll: f32,
    /// The merchant's `scrollOff` and `shopItem`.
    pub scroll_off: usize,
    pub shop_item: usize,
    /// The stonecutter's recipe or the loom's pattern picked.
    pub pick: Option<usize>,
    /// The beacon's chosen effects (`speed`).
    pub beacon: (Option<String>, Option<String>),
    pub book: Book,
    /// The creative tab a left press fell on: it is selected on a release over it.
    pub tab_press: Option<usize>,
    /// The release after a press the screen's own widgets took is not the slots'.
    pub swallow_release: bool,
    /// When the screen opened: animations count from it.
    pub opened: Option<Instant>,
    /// The last press, for double-clicks.
    pub last_press: Option<(Instant, Button)>,
}

/// The recipe book's settings and page (`ClientRecipeBook` and the component's state).
pub struct Book {
    pub visible: bool,
    pub filtering: bool,
    pub page: usize,
    pub overlay: Option<usize>,
    pub focused: bool,
    /// The search box: `setMaxLength(50)`.
    pub search: TextEdit,
}

impl Default for Book {
    fn default() -> Book {
        Book { visible: false, filtering: false, page: 0, overlay: None, focused: false, search: TextEdit::new("", 50) }
    }
}

impl State {
    /// The screen opened (or switched to another): lists and choices start over; the book keeps
    /// whether it is open and filtering, as the game's recipe book settings do.
    pub fn open(&mut self, now: Instant) {
        let book = std::mem::take(&mut self.book);
        *self = State { opened: Some(now), book: Book { visible: book.visible, filtering: book.filtering, ..Book::default() }, ..State::default() };
    }

    /// What a list shows changed: its choice and scroll start over.
    pub fn list_changed(&mut self) {
        (self.pick, self.scroll, self.scroll_off, self.shop_item) = (None, 0.0, 0, 0);
    }

    /// Whether typed text goes to the recipe book's search box.
    pub fn book_typing(&self) -> bool {
        self.book.visible && self.book.focused
    }

    /// A press of `button` now, and whether it is a double-click (`doubleClickTime`: 250 ms).
    pub fn press(&mut self, button: Button, now: Instant) -> bool {
        let double = self.last_press.is_some_and(|(at, b)| b == button && now.saturating_duration_since(at).as_millis() < 250);
        self.last_press = Some((now, button));
        double
    }

    fn ticks(&self, now: Instant) -> i32 {
        self.opened.map_or(0, |t| (now.saturating_duration_since(t).as_millis() / 50) as i32)
    }
}

/// Where a slot's clicks go.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Slot(SlotRef),
    /// What the workstation's slots make (`TakeCrafted`).
    Crafted,
    /// The stonecutter's result: the picked recipe's id.
    Cut(Option<u32>),
    /// The loom's result: the picked pattern's code.
    Loom(Option<&'static str>),
    /// A creative item: its entry id.
    Creative(u32),
    /// The creative inventory's destroy slot.
    Destroy,
    /// Shown, not taken here (a trade's result, the crafter's, a hidden slot).
    None,
}

/// The screen showing.
pub enum Open {
    Simple(Simple),
    Recipe(RecipeScreen),
    Brewing(Brewing),
    Crafter(Crafter),
    Anvil(Anvil),
    Grindstone(Grindstone),
    Smithing(Smithing),
    Cartography(Cartography),
    Enchanting(Enchanting),
    Beacon(Beacon),
    Stonecutter(Stonecutter),
    Loom(Loom),
    Merchant(Merchant),
    Creative(Creative),
}

impl Open {
    pub fn screen(&mut self) -> &mut dyn Screen {
        match self {
            Open::Simple(s) => s,
            Open::Recipe(s) => s,
            Open::Brewing(s) => s,
            Open::Crafter(s) => s,
            Open::Anvil(s) => s,
            Open::Grindstone(s) => s,
            Open::Smithing(s) => s,
            Open::Cartography(s) => s,
            Open::Enchanting(s) => s,
            Open::Beacon(s) => s,
            Open::Stonecutter(s) => s,
            Open::Loom(s) => s,
            Open::Merchant(s) => s,
            Open::Creative(s) => s,
        }
    }
}

/// A built screen: the screen, the stack behind each draw key, and each slot's target.
pub struct Built {
    pub open: Open,
    pub keys: HashMap<i32, Stack>,
    pub targets: Vec<Target>,
}

/// An open inventory screen into `list`: through the Java screens in the Java look, else the
/// Bedrock layout with the player's figure.
#[allow(clippy::too_many_arguments)]
pub(in crate::ui) fn draw(
    java_gui: &mut JavaGui,
    java: bool,
    skin: &mut Skin,
    lang: &acacia_ui::lang::Lang,
    screen: &crate::ui::Screen,
    me: Option<&crate::me::Me>,
    (size, scale): ([u32; 2], u32),
    (mouse, now): ([f32; 2], Instant),
    list: &mut DrawList,
    pictures: &mut Vec<PictureRequest>,
) {
    let gui = [(size[0] / scale) as f32, (size[1] / scale) as f32];
    if !java {
        crate::ui::screen::draw(list, skin, lang, screen, mouse, gui);
        if let Some((request, quad)) = (screen.layout == acacia_ui::inventory::Layout::Player).then(|| crate::ui::figure::inventory_figure(gui, mouse, scale as f32)).flatten() {
            pictures.push(request);
            list.quads.push(quad);
        }
        return;
    }
    let size = super::gui_size(size, scale);
    let (java, state) = screen.java;
    let view = View::new(screen.inventory, &java, me, size, mouse.map(f64::from), now);
    let mut built = java_gui.build(skin.java_items.as_deref(), lang, &view, state);
    let mut calls = Gui::new(size[0], size[1]);
    let at = view.mouse.map(|m| m.floor() as i32);
    java_gui.with_env(|env| frame(built.open.screen(), &mut calls, env, at, 0.0));
    let keys = std::mem::take(&mut built.keys);
    java_gui.paint(skin, &|key| keys.get(&key).cloned(), &calls, list, pictures);
}
