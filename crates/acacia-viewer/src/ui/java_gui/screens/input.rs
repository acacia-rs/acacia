//! Input on the Java screens: each event is asked of the screen as the frame drew it (its own
//! widgets first, as the game's `mouseClicked` overrides do, then the slots through
//! `container::input`), and what it asks of the menu goes out as the bot's commands.

use std::time::Instant;

use acacia_ui::java::container::beacon::BeaconPress;
use acacia_ui::java::container::input::{self, Button, Key, Press};
use acacia_ui::java::container::recipe_book::input::{BookPress, RecipePress};

use super::commands::{beacon_effect, commands};
use super::{Built, JavaGui, JavaScreen, Open, State, View};
use crate::control::{Command, Inventory};
use crate::me::Me;
use crate::settings::LookChoice;
use crate::ui::Ui;
use acacia_render::item::JavaItems;

/// A mouse or key event at `mouse` (GUI pixels).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Press { button: Button, shift: bool, double: bool },
    Release { button: Button, shift: bool },
    Move,
    /// The wheel, up positive.
    Scroll(f64),
    Key(Key),
}

/// What an event did beyond `State`.
#[derive(Debug, Default)]
pub struct Outcome {
    pub commands: Vec<Command>,
    /// The screen closes (the beacon's confirm and cancel).
    pub close: bool,
    /// A creative tab was selected.
    pub tab: Option<usize>,
}

impl Ui {
    /// `event` on the open screen in the Java look; `None` in the Bedrock look.
    #[allow(clippy::too_many_arguments)]
    pub fn java_screen_event(
        &mut self,
        look: LookChoice,
        (inventory, screen, me): (&Inventory, JavaScreen, Option<&Me>),
        (size, scale, mouse): ([u32; 2], u32, [f64; 2]),
        state: &mut State,
        event: Event,
    ) -> Option<Outcome> {
        if look != LookChoice::Java || !self.java_gui.usable(&mut self.java) {
            return None;
        }
        let mouse = mouse.map(|m| m / f64::from(scale));
        let view = View::new(inventory, &screen, me, super::super::gui_size(size, scale), mouse, Instant::now());
        Some(self.java_gui.event(self.java.java_items.as_deref(), &self.lang, &view, state, event))
    }
}

impl JavaGui {
    pub(super) fn event(&self, items: Option<&JavaItems>, lang: &acacia_ui::lang::Lang, view: &View, state: &mut State, event: Event) -> Outcome {
        let mut built = self.build(items, lang, view, state);
        let mut out = Outcome::default();
        let mouse = view.mouse;
        match event {
            Event::Press { button, shift, double } => {
                state.held = Some(button);
                if widgets(&mut built, view, state, mouse, button, &mut out) {
                    state.swallow_release = true;
                    return out;
                }
                let clicks = input::mouse_clicked(built.open.screen(), &mut state.pointer, mouse, button, shift, double, false);
                out.commands = clicks.iter().flat_map(|c| commands(c, &built.targets, view)).collect();
            }
            Event::Release { button, shift } => {
                state.held = None;
                state.scrolling = false;
                if let (Open::Creative(s), Some(tab)) = (&built.open, state.tab_press.take()) {
                    if button == Button::Left && s.tab_at(mouse) == Some(tab) {
                        out.tab = Some(tab);
                        state.scroll = 0.0;
                    }
                }
                if std::mem::take(&mut state.swallow_release) {
                    return out;
                }
                let clicks = input::mouse_released(built.open.screen(), &mut state.pointer, mouse, button, shift, false);
                out.commands = clicks.iter().flat_map(|c| commands(c, &built.targets, view)).collect();
            }
            Event::Move if state.scrolling => match &mut built.open {
                Open::Stonecutter(s) => {
                    s.mouse_dragged(mouse);
                    state.scroll = s.scroll_offs();
                }
                Open::Loom(s) => {
                    s.mouse_dragged(mouse);
                    state.scroll = s.scroll_offs();
                }
                Open::Merchant(s) => {
                    s.mouse_dragged(mouse);
                    state.scroll_off = s.scroll_off;
                }
                Open::Creative(s) => state.scroll = s.dragged_scroll(mouse),
                _ => {}
            },
            Event::Move if state.held.is_some() => input::mouse_dragged(built.open.screen(), &mut state.pointer, mouse),
            Event::Move => {}
            Event::Scroll(dy) => match &mut built.open {
                Open::Stonecutter(s) => {
                    s.scroll(dy);
                    state.scroll = s.scroll_offs();
                }
                Open::Loom(s) => {
                    s.scroll(dy);
                    state.scroll = s.scroll_offs();
                }
                Open::Merchant(s) => {
                    s.scroll(dy);
                    state.scroll_off = s.scroll_off;
                }
                Open::Creative(s) => state.scroll = s.scrolled(dy).unwrap_or(state.scroll),
                _ => {}
            },
            Event::Key(key) => {
                let click = input::key_pressed(built.open.screen(), mouse, key);
                out.commands = click.iter().flat_map(|c| commands(c, &built.targets, view)).collect();
            }
        }
        out
    }
}

/// The screen's own widgets under a press; true when one took it.
fn widgets(built: &mut Built, view: &View, state: &mut State, mouse: [f64; 2], button: Button, out: &mut Outcome) -> bool {
    let inv = view.inventory;
    let list = |press: Option<Press>, len: usize, state: &mut State| match press {
        Some(Press::Button(i)) => {
            if i < len {
                state.pick = Some(i);
            }
            true
        }
        Some(Press::Scroller) => {
            state.scrolling = true;
            true
        }
        None => false,
    };
    let left = button == Button::Left;
    match &mut built.open {
        Open::Recipe(s) => {
            let Some(press) = s.mouse_clicked(mouse, button) else { return false };
            let b = &s.book;
            (state.book.visible, state.book.filtering, state.book.page, state.book.overlay) = (b.visible, b.filtering, b.page, b.overlay);
            state.book.focused = b.search.focused;
            if let RecipePress::Book(BookPress::Recipe { collection, .. }) = press {
                if let Some(stack) = inv.craftable.get(collection) {
                    out.commands.push(Command::Craft { name: stack.name.clone(), table: inv.workbench });
                }
            }
            true
        }
        Open::Stonecutter(s) => list(s.mouse_clicked(mouse), inv.picks.len(), state),
        Open::Loom(s) => list(s.mouse_clicked(mouse), inv.patterns.len(), state),
        Open::Enchanting(s) => {
            let Some(i) = s.mouse_clicked(mouse) else { return false };
            out.commands.push(Command::Enchant(i));
            true
        }
        Open::Beacon(s) if left => {
            let Some(press) = s.mouse_clicked(mouse) else { return false };
            state.beacon = (s.primary.clone(), s.secondary.clone());
            match press {
                BeaconPress::Chose => {}
                BeaconPress::Confirm => {
                    let primary = s.primary.as_deref().and_then(beacon_effect);
                    if let Some(primary) = primary {
                        out.commands.push(Command::Beacon(primary, s.secondary.as_deref().and_then(beacon_effect)));
                    }
                    out.close = true;
                }
                BeaconPress::Cancel => out.close = true,
            }
            true
        }
        Open::Merchant(s) => match s.mouse_clicked(mouse) {
            Some(Press::Button(i)) => {
                state.shop_item = i;
                // TODO: Java's selection only fills the payment slots (`ServerboundSelectTradePacket`); the bot trades once.
                if inv.trade.as_ref().and_then(|t| t.offers.get(i)).is_some_and(|o| o.open) {
                    out.commands.push(Command::Trade(i));
                }
                true
            }
            Some(Press::Scroller) => {
                state.scrolling = true;
                true
            }
            None => false,
        },
        Open::Creative(s) if left => match s.mouse_clicked(mouse) {
            Some(Press::Button(tab)) => {
                state.tab_press = Some(tab);
                true
            }
            Some(Press::Scroller) => {
                state.scrolling = s.scrollable();
                true
            }
            None => false,
        },
        _ => false,
    }
}
