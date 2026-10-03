//! Shift-click equivalent. Bedrock has no quick-move action: the client itself picks destination
//! slots and sends plain `Place` actions, so the bot does the same. Like vanilla it tops up partial
//! stacks of the same item first, then uses the first empty slot (2026-10-02 capture: a smelted
//! ingot joined the ingots in slot 11 while slots 1 and 2 were empty).

use super::plan::stacks_with;
use super::slot::open_container;
use super::{Op, SlotRef};
use crate::state::{GameState, Inventory, ItemStack};
use crate::ActionError;

/// Assumed maximum stack size when merging; the item registry does not carry it.
const MAX_STACK: u16 = 64;

/// Ops that move the whole stack in `from` to the other side: container or workstation → player
/// inventory, player → open container, or hotbar ↔ inventory when no container is open.
pub(crate) fn quick_move_ops(state: &GameState, from: SlotRef) -> Result<Vec<Op>, ActionError> {
    let stack = from.stack(state).filter(|s| !s.is_empty()).ok_or_else(|| ActionError::NotPossible(format!("{from:?} is empty")))?;
    let targets: Vec<(SlotRef, ItemStack)> = targets(state, from).into_iter().filter_map(|t| t.stack(state).map(|s| (t, s.clone()))).collect();
    let ops = move_ops(from, stack, &targets);
    if ops.is_empty() {
        return Err(ActionError::NotPossible("no room for the stack".into()));
    }
    Ok(ops)
}

/// Ops that move `stack`, the contents `from` will have (a craft result not made yet), into the
/// player's main inventory.
pub(crate) fn to_inventory_ops(state: &GameState, from: SlotRef, stack: &ItemStack) -> Result<Vec<Op>, ActionError> {
    to_inventory_ops_after(state, from, stack, &[])
}

/// [`to_inventory_ops`] into the inventory as `earlier` ops of the same request leave it: their
/// `Consume`s free room (vanilla put a recipe-book result into the slot its planks came from) and
/// slots they move items into are left out.
pub(crate) fn to_inventory_ops_after(state: &GameState, from: SlotRef, stack: &ItemStack, earlier: &[Op]) -> Result<Vec<Op>, ActionError> {
    let mut targets: Vec<(SlotRef, ItemStack)> = state.inventory.main.iter().enumerate().map(|(i, s)| (SlotRef::Main(i as u8), s.clone())).collect();
    for op in earlier {
        match *op {
            Op::Consume { from, count } => {
                if let Some((_, s)) = targets.iter_mut().find(|(t, _)| *t == from) {
                    s.count = s.count.saturating_sub(u16::from(count));
                    if s.count == 0 {
                        *s = ItemStack::default();
                    }
                }
            }
            Op::Transfer { to, .. } => targets.retain(|(t, _)| *t != to),
            _ => {}
        }
    }
    let ops = move_ops(from, stack, &targets);
    let moved: u16 = ops.iter().map(|op| if let Op::Transfer { count, .. } = op { u16::from(*count) } else { 0 }).sum();
    if moved < stack.count {
        return Err(ActionError::NotPossible("no room in the inventory for the result".into()));
    }
    Ok(ops)
}

/// Partial stacks of the same item in slot order, then the rest into the first empty slot.
fn move_ops(from: SlotRef, stack: &ItemStack, targets: &[(SlotRef, ItemStack)]) -> Vec<Op> {
    // Only a count above one proves the item stacks: two unenchanted swords look alike but never merge.
    let stackable = |dst: &ItemStack| stack.count > 1 || dst.count > 1;
    let mut left = stack.count;
    let mut ops = Vec::new();
    for (to, dst) in targets {
        if left == 0 {
            break;
        }
        if !dst.is_empty() && dst.count < MAX_STACK && stacks_with(dst, stack) && stackable(dst) {
            let count = left.min(MAX_STACK - dst.count);
            ops.push(Op::Transfer { from, to: *to, count: clamp(count) });
            left -= count;
        }
    }
    if left > 0
        && let Some((to, _)) = targets.iter().find(|(_, s)| s.is_empty())
    {
        ops.push(Op::Transfer { from, to: *to, count: clamp(left) });
    }
    ops
}

fn main_slots(range: std::ops::Range<u8>) -> Vec<SlotRef> {
    range.map(SlotRef::Main).collect()
}

fn targets(state: &GameState, from: SlotRef) -> Vec<SlotRef> {
    let hotbar = Inventory::HOTBAR_SLOTS;
    let all = Inventory::MAIN_SLOTS as u8;
    match (from, open_container(state)) {
        (SlotRef::Container(_) | SlotRef::Ui(_), _) => main_slots(0..all),
        (_, Some(open)) => (0..open.slots.len().min(usize::from(u8::MAX)) as u8).map(SlotRef::Container).collect(),
        (SlotRef::Main(i), None) if i < hotbar => main_slots(hotbar..all),
        (SlotRef::Main(_), None) => main_slots(0..hotbar),
        (_, None) => main_slots(hotbar..all).into_iter().chain(main_slots(0..hotbar)).collect(),
    }
}

fn clamp(count: u16) -> u8 {
    u8::try_from(count).unwrap_or(u8::MAX)
}
