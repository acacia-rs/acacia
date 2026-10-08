//! Mouse clicks on an inventory screen, as vanilla's (Java's `AbstractContainerMenu.doClick`)
//! carried out with the item moves on [`Bot`]: the cursor picks up, places, swaps and splits.

use super::plan::stacks_with;
use super::SlotRef;
use crate::{ActionError, Bot};

/// Largest stack the screen builds by hand; the server refuses more for items that stack lower.
const MAX_STACK: u16 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Click {
    /// Pick up the stack, put the cursor's down, or swap the two.
    Left,
    /// Pick up half (rounded up), or put one down.
    Right,
    /// Move the stack to the other part of the screen.
    Shift,
}

impl Bot {
    /// A click on `slot` with the cursor's stack as it is tracked.
    pub async fn click_slot(&mut self, slot: SlotRef, click: Click) -> Result<(), ActionError> {
        if click == Click::Shift {
            return self.quick_move(slot).await.map(|_| ());
        }
        let state = self.state();
        let here = slot.stack(state).cloned().unwrap_or_default();
        let held = SlotRef::Cursor.stack(state).cloned().unwrap_or_default();
        let right = click == Click::Right;
        let (count, from, to) = match (held.is_empty(), here.is_empty()) {
            (true, true) => return Ok(()),
            (true, false) => (if right { here.count.div_ceil(2) } else { here.count }, slot, SlotRef::Cursor),
            (false, true) => (if right { 1 } else { held.count }, SlotRef::Cursor, slot),
            (false, false) if stacks_with(&held, &here) => {
                (if right { 1 } else { held.count }.min(MAX_STACK.saturating_sub(here.count)), SlotRef::Cursor, slot)
            }
            (false, false) => return self.swap_items(SlotRef::Cursor, slot).await,
        };
        if count == 0 {
            return Ok(());
        }
        self.move_item(from, to, u8::try_from(count).unwrap_or(u8::MAX)).await
    }

    /// A click outside the screen: throws the cursor's stack, or one of it with `one`.
    pub async fn drop_cursor(&mut self, one: bool) -> Result<(), ActionError> {
        let held = self.state().inventory.cursor().count;
        if held == 0 {
            return Ok(());
        }
        let count = if one { 1 } else { u8::try_from(held).unwrap_or(u8::MAX) };
        self.drop_item(SlotRef::Cursor, count).await
    }
}
