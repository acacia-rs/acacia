//! Survival helpers: eating and drinking ([`crate::Bot::consume`], auto-eat), equipping armour and
//! tools. Vanilla packet sequences: docs/research/survival-signs-beds.md.

mod auto_eat;
mod consume;
mod equip;
mod fetch;
mod food;
mod item_use;
mod tools;

#[cfg(test)]
mod tests;

pub use auto_eat::AutoEat;
pub use equip::{Destination, EquipMethod};
pub use food::{food, use_ticks, Food, FoodChoice};

use crate::{ActionError, Bot};
use auto_eat::AutoEater;
use fetch::Fetch;
use item_use::ItemUse;

/// Per-bot survival state, advanced every client tick.
#[derive(Debug)]
pub(crate) struct Survival {
    item_use: Option<ItemUse>,
    /// How the last [`Bot::consume`] ended, until it collects it.
    result: Option<Result<(), ActionError>>,
    auto_eat: AutoEater,
    /// Auto-eat moving food from this main-inventory slot into the hotbar.
    fetch: Option<(Fetch, u8)>,
    /// Whether the server accepted the fetch's request, once it answered.
    fetch_response: Option<bool>,
    /// See [`Bot::set_tool_spare_durability`].
    tool_spare: u16,
    /// The bot is being stepped by `Bot::next` rather than by an awaiting action.
    pub idle: bool,
}

impl Survival {
    pub fn new(policy: Option<AutoEat>) -> Self {
        let mut auto_eat = AutoEater::default();
        auto_eat.policy = policy;
        Self {
            item_use: None,
            result: None,
            auto_eat,
            fetch: None,
            fetch_response: None,
            tool_spare: tools::DEFAULT_SPARE_DURABILITY,
            idle: false,
        }
    }
}

impl Bot {
    pub(crate) fn tick_survival(&mut self) {
        self.tick_auto_eat();
        self.tick_fetch();
        self.tick_item_use();
    }
}
