//! Workstations on Java: after the inputs go in by clicks and the station's own packet (rename, button) is sent, the
//! server fills the result slot itself, and that is shift-clicked out unpredicted. Nothing is predicted: Java's
//! anvil and XP rules are the server's, and it offering nothing is the refusal.

use super::java_craft::RESULT_TIMEOUT;
use super::StationStep;
use crate::bot::Backend;
use crate::state::java_inventory::result_slot;
use crate::{ActionError, Bot};

impl Bot {
    /// Does `step` at the open station and takes its result.
    pub(super) async fn java_station_step(&mut self, step: StationStep<'_>) -> Result<(), ActionError> {
        let Backend::Java(java) = &mut self.backend else { return Err(ActionError::Unsupported("Java stations")) };
        let menu = java.client.inventory.window.as_ref().map(|w| w.menu);
        let slot = menu.and_then(result_slot).ok_or_else(|| ActionError::NotPossible("no station with a result slot is open".into()))?;
        let sent = match step {
            StationStep::Take => Ok(()),
            StationStep::Rename(name) => java.client.rename_item(name),
            StationStep::Unsupported(what) => return Err(ActionError::Unsupported(what)),
        };
        sent.map_err(|_| ActionError::Disconnected)?;
        let offered = |bot: &Bot, _: &_| result_count(bot, slot).filter(|&n| n > 0).map(|_| ());
        match self.wait_until(RESULT_TIMEOUT, offered).await {
            Err(ActionError::Timeout) => return Err(ActionError::NotPossible("the station offers nothing for these inputs".into())),
            other => other?,
        }
        self.click_pause().await?;
        self.java_take_result(slot).await?;
        let taken = |bot: &Bot, _: &_| (result_count(bot, slot) == Some(0)).then_some(());
        self.wait_until(RESULT_TIMEOUT, taken).await
    }

    /// Enchanting option `option` once the table offers it (its level cost, menu data 0-2, is above 0); done when the
    /// input slot holds something else (enchanted, or a book turned enchanted book).
    pub(super) async fn java_enchant(&mut self, option: usize) -> Result<(), ActionError> {
        let cost = |bot: &Bot, _: &_| menu(bot)?.data.iter().find(|&&(id, _)| usize::try_from(id) == Ok(option)).map(|&(_, v)| v).filter(|&v| v > 0);
        match self.wait_until(RESULT_TIMEOUT, cost).await {
            Err(ActionError::Timeout) => return Err(ActionError::NotPossible(format!("the table offers no option {option}"))),
            other => other?,
        };
        let input = |bot: &Bot| menu(bot).and_then(|w| w.slots.first().cloned().flatten());
        let before = input(self);
        self.click_pause().await?;
        let Backend::Java(java) = &mut self.backend else { return Err(ActionError::Disconnected) };
        java.client.menu_button(i32::try_from(option).unwrap_or(0)).map_err(|_| ActionError::Disconnected)?;
        let changed = |bot: &Bot, _: &_| (input(bot) != before).then_some(());
        match self.wait_until(RESULT_TIMEOUT, changed).await {
            Err(ActionError::Timeout) => Err(ActionError::NotPossible(format!("option {option} was refused (levels or lapis?)"))),
            other => other,
        }
    }
}

fn menu(bot: &Bot) -> Option<&acacia_java_client::window::Window> {
    bot.java()?.client().inventory.window.as_ref()
}

/// How many items the open menu shows in `slot`; `None` once no menu is open.
fn result_count(bot: &Bot, slot: usize) -> Option<i32> {
    let window = menu(bot)?;
    Some(window.slots.get(slot)?.as_ref().map_or(0, |s| s.count))
}
