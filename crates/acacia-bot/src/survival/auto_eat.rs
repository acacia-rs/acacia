//! Opt-in auto-eat, driven from the client tick so it never blocks [`crate::Bot::next`]: when the
//! hunger bar drops below the threshold it picks food from the hotbar (first moving some up from
//! the main inventory if the hotbar has none, see `fetch.rs`), selects it, eats, and switches
//! back. It only starts while the caller is polling `next` (no action is awaiting), no window is
//! open and nothing else is being used.

use acacia_client::proto::types::GameMode;

use super::food::{choose_food, FoodChoice};
use crate::human::{HOTBAR_SWITCH, HUNGER_NOTICE};
use crate::state::Inventory;
use crate::Bot;

/// Ticks to wait after finding no food before looking again.
const NO_FOOD_TICKS: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutoEat {
    /// Eat when hunger (0-20) is below this.
    pub below: f32,
    pub prefer: FoodChoice,
    /// Move food up from the main inventory when the hotbar has none (default on).
    pub from_inventory: bool,
}

impl Default for AutoEat {
    fn default() -> Self {
        Self { below: 14.0, prefer: FoodChoice::default(), from_inventory: true }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum State {
    #[default]
    Watching,
    Noticing { left: u32 },
    Fetching,
    Switching { left: u32, back_to: u8 },
    Eating { back_to: Option<u8> },
    Restoring { left: u32, slot: u8 },
    Cooldown { left: u32 },
}

/// What auto-eat sees this tick.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ctx {
    /// Below the threshold, alive, in a game mode with hunger.
    pub hungry: bool,
    /// No action awaiting, no window open, nothing in use.
    pub free: bool,
    pub using: bool,
    pub selected: u8,
    /// The hotbar slot of the food to eat.
    pub food: Option<u8>,
    /// Without hotbar food: the main-inventory slot of the food to move up.
    pub stored: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Act {
    Select(u8),
    Eat,
    /// Move the food in this main-inventory slot into the hotbar.
    Fetch(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Delay {
    Notice,
    Switch,
}

#[derive(Debug, Default)]
pub(crate) struct AutoEater {
    pub policy: Option<AutoEat>,
    state: State,
}

impl AutoEater {
    pub fn tick(&mut self, ctx: &Ctx, mut delay: impl FnMut(Delay) -> u32) -> Option<Act> {
        use State as S;
        let (state, act) = match self.state {
            S::Watching if ctx.hungry && ctx.free => (S::Noticing { left: delay(Delay::Notice) }, None),
            S::Watching => (S::Watching, None),
            S::Noticing { .. } if !ctx.hungry || !ctx.free => (S::Watching, None),
            S::Noticing { left: 0 } => match (ctx.food, ctx.stored) {
                (None, None) => (S::Cooldown { left: NO_FOOD_TICKS }, None),
                (None, Some(from)) => (S::Fetching, Some(Act::Fetch(from))),
                (Some(slot), _) if slot == ctx.selected => (S::Eating { back_to: None }, Some(Act::Eat)),
                (Some(slot), _) => (S::Switching { left: delay(Delay::Switch), back_to: ctx.selected }, Some(Act::Select(slot))),
            },
            S::Noticing { left } => (S::Noticing { left: left - 1 }, None),
            S::Fetching => (S::Fetching, None),
            S::Switching { back_to, .. } if !ctx.free => (S::Restoring { left: delay(Delay::Switch), slot: back_to }, None),
            S::Switching { left: 0, back_to } => (S::Eating { back_to: Some(back_to) }, Some(Act::Eat)),
            S::Switching { left, back_to } => (S::Switching { left: left - 1, back_to }, None),
            S::Eating { .. } if ctx.using => (self.state, None),
            S::Eating { back_to: Some(slot) } => (S::Restoring { left: delay(Delay::Switch), slot }, None),
            S::Eating { back_to: None } => (S::Watching, None),
            S::Restoring { .. } if !ctx.free => (S::Watching, None),
            S::Restoring { left: 0, slot } => (S::Watching, Some(Act::Select(slot))),
            S::Restoring { left, slot } => (S::Restoring { left: left - 1, slot }, None),
            S::Cooldown { left: 0 } => (S::Watching, None),
            S::Cooldown { left } => (S::Cooldown { left: left - 1 }, None),
        };
        self.state = state;
        act
    }

    /// Eating could not start: back off as if no food was found.
    pub fn eat_failed(&mut self) {
        self.state = match self.state {
            State::Eating { back_to: Some(slot) } => State::Restoring { left: 0, slot },
            _ => State::Cooldown { left: NO_FOOD_TICKS },
        };
    }

    /// The food move ended: on success look at the hotbar again after `after` ticks.
    pub fn fetched(&mut self, ok: bool, after: u32) {
        if self.state == State::Fetching {
            self.state = if ok { State::Noticing { left: after } } else { State::Cooldown { left: NO_FOOD_TICKS } };
        }
    }
}

impl Bot {
    /// Turns auto-eat on (`Some`) or off.
    pub fn set_auto_eat(&mut self, policy: Option<AutoEat>) {
        self.survival.auto_eat.policy = policy;
    }

    pub(crate) fn tick_auto_eat(&mut self) {
        let Some(policy) = self.survival.auto_eat.policy else { return };
        let p = &self.state.player;
        let hungry = p.alive && matches!(p.game_mode, GameMode::Survival | GameMode::Adventure) && p.hunger < policy.below;
        let using = self.survival.item_use.is_some();
        let inv = &self.state.inventory;
        let choose = |slots: std::ops::Range<u8>| {
            let names = slots.filter_map(|i| Some((i, self.state.items.name(inv.main[usize::from(i)].network_id)?)));
            choose_food(names, 20.0 - p.hunger, policy.prefer)
        };
        let food = hungry.then(|| choose(0..Inventory::HOTBAR_SLOTS)).flatten();
        let stored = (hungry && food.is_none() && policy.from_inventory)
            .then(|| choose(Inventory::HOTBAR_SLOTS..Inventory::MAIN_SLOTS as u8))
            .flatten();
        let ctx = Ctx {
            hungry,
            // Switching away from the rod removes the own hook on BDS.
            free: self.survival.idle && !using && self.state.containers.open.is_none() && self.state.fishing.hook.is_none(),
            using,
            selected: inv.selected_hotbar_slot,
            food,
            stored,
        };
        let human = &mut self.human;
        let act = self.survival.auto_eat.tick(&ctx, |d| {
            human.ticks_between(match d {
                Delay::Notice => HUNGER_NOTICE,
                Delay::Switch => HOTBAR_SWITCH,
            })
        });
        match act {
            Some(Act::Select(slot)) => {
                let _ = self.select_hotbar(slot);
            }
            Some(Act::Eat) => {
                if let Err(e) = self.start_item_use(true) {
                    tracing::debug!(error = %e, "auto-eat could not start");
                    self.survival.auto_eat.eat_failed();
                }
            }
            Some(Act::Fetch(from)) => self.start_fetch(from),
            None => {}
        }
    }
}
