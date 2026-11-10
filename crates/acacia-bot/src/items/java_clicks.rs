//! Item moves on Java: each [`Op`] becomes the clicks a player would make in the open menu (the own inventory, or
//! the open container with the player's slots below it), planned against a
//! copy of the client's inventory so every click's outcome is known before any is sent. Partial counts are moved one
//! right click at a time; swaps with a hotbar or offhand slot use the number keys, as players do.
//!
//! Vanilla releases the movement keys when a screen opens, so the bot stands for a tick with no input, sends the
//! clicks and, in the own inventory, closes it (Grim treats clicks in window 0 as an open inventory until the close).
//! A container screen stays open for [`Bot::close_container`].

use acacia_java_client::{Client, Error as ClientError, Inventory};
use acacia_java_client::clicks::{Click, OFFHAND_BUTTON, OUTSIDE};
use acacia_java_client::inventory::{ARMOUR_HEAD, HOTBAR_START, OFFHAND};

/// The player's slots below a container's: 27 main, then 9 hotbar.
const MAIN_ROWS: u8 = 27;

use super::{Op, SlotRef, ui};
use crate::state::java_inventory::{station_slots, CRAFTING_MENU, GRID_START};
use crate::bot::Backend;
use crate::{ActionError, Bot};

/// Clicks that carry out `ops` in order, starting from `inventory`.
pub(crate) fn plan(inventory: &Inventory, ops: &[Op]) -> Result<Vec<Click>, ActionError> {
    let mut plan = Planner { inv: inventory.clone(), clicks: Vec::new() };
    for &op in ops {
        plan.op(op)?;
    }
    Ok(plan.clicks)
}

/// `slot`'s index in the open menu: window 0, or a container of `own` slots followed by the player's.
fn window(inv: &Inventory, slot: SlotRef) -> Result<i16, ActionError> {
    let station = inv.window.as_ref().and_then(|w| station_slots(w.menu).iter().position(|&s| SlotRef::Ui(s) == slot));
    if let Some(i) = station {
        return i16::try_from(i).map_err(|_| not_possible("slot out of range"));
    }
    let i = match (inv.window.as_ref().map(|w| w.slots.len()), slot) {
        (None, SlotRef::Main(i @ 0..9)) => HOTBAR_START + usize::from(i),
        (None, SlotRef::Main(i @ 9..36)) => usize::from(i),
        (None, SlotRef::Armor(i @ 0..4)) => ARMOUR_HEAD + usize::from(i),
        (None, SlotRef::Offhand) => OFFHAND,
        (Some(own), SlotRef::Main(i @ 0..9)) => own + usize::from(MAIN_ROWS + i),
        (Some(own), SlotRef::Main(i @ 9..36)) => own + usize::from(i - 9),
        (Some(own), SlotRef::Container(i)) if usize::from(i) < own => usize::from(i),
        (None, SlotRef::Ui(i @ ui::CRAFTING_2X2..ui::CRAFTING_3X3)) => GRID_START + usize::from(i - ui::CRAFTING_2X2),
        (Some(_), SlotRef::Ui(i @ ui::CRAFTING_3X3..=ui::CRAFTING_3X3_LAST)) if inv.window.as_ref().is_some_and(|w| w.menu == CRAFTING_MENU) => {
            GRID_START + usize::from(i - ui::CRAFTING_3X3)
        }
        (_, other) => return Err(not_possible(&format!("{other:?} is not in the open Java menu"))),
    };
    i16::try_from(i).map_err(|_| not_possible("slot out of range"))
}

/// The number key that swaps with `slot`; the offhand key only in the own inventory.
fn button(inv: &Inventory, slot: SlotRef) -> Option<i8> {
    match slot {
        SlotRef::Main(i @ 0..9) => Some(i as i8),
        SlotRef::Offhand if inv.window.is_none() => Some(OFFHAND_BUTTON),
        _ => None,
    }
}

fn not_possible(reason: &str) -> ActionError {
    ActionError::NotPossible(reason.into())
}

struct Planner {
    inv: Inventory,
    clicks: Vec<Click>,
}

impl Planner {
    fn push(&mut self, click: Click) {
        self.inv.click(click);
        self.clicks.push(click);
    }

    fn repeat(&mut self, click: Click, times: i32) {
        (0..times).for_each(|_| self.push(click));
    }

    fn window(&self, slot: SlotRef) -> Result<i16, ActionError> {
        window(&self.inv, slot)
    }

    fn count(&self, slot: i16) -> i32 {
        self.inv.menu_slot(slot as usize).map_or(0, |s| s.count)
    }

    fn cursor(&self) -> i32 {
        self.inv.cursor.as_ref().map_or(0, |s| s.count)
    }

    fn need_empty_cursor(&self) -> Result<(), ActionError> {
        if self.cursor() == 0 { Ok(()) } else { Err(not_possible("the cursor holds a stack")) }
    }

    /// `to` must be empty or hold the cursor's item.
    fn need_fits(&self, to: i16) -> Result<(), ActionError> {
        match (self.inv.menu_slot(to as usize), &self.inv.cursor) {
            (Some(here), Some(held)) if (here.item, &here.components) != (held.item, &held.components) => Err(not_possible("the target holds another item")),
            _ => Ok(()),
        }
    }

    fn op(&mut self, op: Op) -> Result<(), ActionError> {
        match op {
            Op::Transfer { from, to, count } => self.transfer(from, to, i32::from(count)),
            Op::Swap { a, b } => self.swap(a, b),
            Op::Drop { from, count } => self.drop(from, i32::from(count)),
            Op::Destroy { .. } | Op::Consume { .. } | Op::Create { .. } => Err(ActionError::Unsupported("destroying and crafting on Java")),
        }
    }

    fn transfer(&mut self, from: SlotRef, to: SlotRef, n: i32) -> Result<(), ActionError> {
        match (from, to) {
            (SlotRef::Cursor, SlotRef::Cursor) => Err(not_possible("moving the cursor onto itself")),
            (SlotRef::Cursor, to) => {
                let (t, held) = (self.window(to)?, self.cursor());
                if n > held {
                    return Err(not_possible("the cursor holds fewer items"));
                }
                self.need_fits(t)?;
                if n == held { self.push(Click::left(t)) } else { self.repeat(Click::right(t), n) }
                if self.cursor() != held - n { Err(not_possible("no room in the target")) } else { Ok(()) }
            }
            (from, SlotRef::Cursor) => {
                let f = self.window(from)?;
                let has = self.count(f);
                self.need_empty_cursor()?;
                if n > has {
                    return Err(not_possible("the slot holds fewer items"));
                }
                if n == (has + 1) / 2 {
                    self.push(Click::right(f));
                } else {
                    self.push(Click::left(f));
                    self.repeat(Click::right(f), has - n);
                }
                Ok(())
            }
            (from, to) => {
                let (f, t) = (self.window(from)?, self.window(to)?);
                let (has, before) = (self.count(f), self.count(t));
                self.need_empty_cursor()?;
                if n > has {
                    return Err(not_possible("the slot holds fewer items"));
                }
                self.push(Click::left(f));
                self.need_fits(t)?;
                if n == has { self.push(Click::left(t)) } else { self.repeat(Click::right(t), n) }
                if self.cursor() > 0 {
                    self.push(Click::left(f));
                }
                if self.count(t) != before + n { Err(not_possible("no room in the target")) } else { Ok(()) }
            }
        }
    }

    fn swap(&mut self, a: SlotRef, b: SlotRef) -> Result<(), ActionError> {
        let armour = |s| matches!(s, SlotRef::Armor(_));
        match (a, b) {
            (SlotRef::Cursor, other) | (other, SlotRef::Cursor) => {
                self.push(Click::left(self.window(other)?));
                Ok(())
            }
            (a, b) if button(&self.inv, b).is_some() && !armour(a) => {
                self.push(Click::swap(self.window(a)?, button(&self.inv, b).unwrap_or_default()));
                Ok(())
            }
            (a, b) if button(&self.inv, a).is_some() && !armour(b) => self.swap(b, a),
            (a, b) => {
                self.need_empty_cursor()?;
                let (a, b) = (self.window(a)?, self.window(b)?);
                [a, b, a].into_iter().for_each(|s| self.push(Click::left(s)));
                Ok(())
            }
        }
    }

    fn drop(&mut self, from: SlotRef, n: i32) -> Result<(), ActionError> {
        if from == SlotRef::Cursor {
            if n > self.cursor() {
                return Err(not_possible("the cursor holds fewer items"));
            }
            let all = Click::left(OUTSIDE);
            if n == self.cursor() { self.push(all) } else { self.repeat(Click::right(OUTSIDE), n) }
            return Ok(());
        }
        let f = self.window(from)?;
        self.need_empty_cursor()?;
        if n > self.count(f) {
            return Err(not_possible("the slot holds fewer items"));
        }
        if n == self.count(f) { self.push(Click::throw(f, true)) } else { self.repeat(Click::throw(f, false), n) }
        Ok(())
    }
}

impl Bot {
    /// [`Bot::item_stack_request`] on Java. Clicks are predicted, not answered: the server only writes back slots
    /// it disagrees with.
    pub(crate) async fn java_item_ops(&mut self, ops: &[Op]) -> Result<(), ActionError> {
        let Backend::Java(java) = &self.backend else { return Err(ActionError::Unsupported("Java item moves")) };
        let clicks = plan(&java.client.inventory, ops)?;
        self.java_clicks(|client| clicks.into_iter().try_for_each(|c| client.click(c))).await
    }

    /// Shift-clicks the open menu's result `slot` (crafting, stations). Only the server knows what that makes, so the click
    /// predicts nothing and the server sends back every slot it changed.
    pub(crate) async fn java_take_result(&mut self, slot: usize) -> Result<(), ActionError> {
        let slot = i16::try_from(slot).map_err(|_| not_possible("slot out of range"))?;
        self.java_clicks(|client| client.click_unpredicted(Click::quick_move(slot))).await
    }

    /// Stands for a tick with no input, then sends what `send` queues and, in the own inventory unless crafting holds
    /// it open ([`crate::bot::Java::inventory_held`]), closes it.
    async fn java_clicks(&mut self, send: impl FnOnce(&mut Client) -> Result<(), ClientError>) -> Result<(), ActionError> {
        let held = self.backend.controls_mut().map(std::mem::take).unwrap_or_default();
        self.next_tick(|_, _| false).await?;
        let Backend::Java(java) = &mut self.backend else { return Err(ActionError::Disconnected) };
        let close = java.client.inventory.window.is_none() && !java.inventory_held;
        let sent = send(&mut java.client).and_then(|()| if close { java.client.close_screen() } else { Ok(()) });
        crate::state::java_inventory::mirror(&mut self.state.inventory, &java.client.inventory);
        crate::state::java_inventory::mirror_window(&mut self.state.containers, &java.client.inventory);
        sent.map_err(|_| ActionError::Disconnected)?;
        let ticked = self.next_tick(|_, _| false).await;
        if let Some(controls) = self.backend.controls_mut() {
            *controls = held;
        }
        ticked.map(|_| ())
    }
}

#[cfg(test)]
#[path = "java_clicks_tests.rs"]
mod tests;
