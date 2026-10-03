use acacia_client::proto::nbt::Value;
use acacia_physics::BlockPos;
use acacia_world::{Mining, Tool};

use super::equip::Destination;
use crate::interact::{break_ticks, BreakConditions};
use crate::interact::breaking::{enchantment_level, EFFICIENCY};
use crate::items::SlotRef;
use crate::state::{Inventory, ItemStack};
use crate::{ActionError, Bot};

/// Default for [`Bot::set_tool_spare_durability`]: skip only tools the next use would break.
pub(crate) const DEFAULT_SPARE_DURABILITY: u16 = 1;

impl Bot {
    /// The main-inventory slot whose item breaks the block at `pos` fastest (see
    /// [`crate::interact::break_ticks`]), if any is faster than the bare hand. Tools with less
    /// durability left than [`Bot::set_tool_spare_durability`] are skipped; among equally fast
    /// tools the least worn wins. Physics bots only.
    pub fn best_tool_for(&self, pos: BlockPos) -> Option<SlotRef> {
        let (_, block) = self.block_at(pos)?;
        let base = BreakConditions { tool: None, efficiency: 0, ..self.break_conditions() };
        let items = &self.state.items;
        let spare = self.survival.tool_spare;
        fastest_slot(&self.state.inventory, |item| items.name(item.network_id), &block.mining, base, spare).map(SlotRef::Main)
    }

    /// Holds the best tool for the block at `pos` ([`Bot::best_tool_for`]); keeps the held item when
    /// nothing beats the hand. Returns the slot of the tool now held.
    pub async fn equip_best_tool(&mut self, pos: BlockPos) -> Result<Option<SlotRef>, ActionError> {
        if self.block_at(pos).is_none() {
            return Err(ActionError::NotPossible(format!("{pos:?} is not loaded (physics bots only)")));
        }
        let Some(slot) = self.best_tool_for(pos) else { return Ok(None) };
        self.equip(slot, Destination::Hand).await?;
        Ok(Some(SlotRef::Main(self.state.inventory.selected_hotbar_slot)))
    }

    /// Tools with less durability left than `spare` (max durability − `Damage`) are not picked by
    /// [`Bot::best_tool_for`]. Default 1: only tools the next use would break are skipped.
    pub fn set_tool_spare_durability(&mut self, spare: u16) {
        self.survival.tool_spare = spare;
    }
}

/// Durability left before `tool` breaks: its maximum minus the stack's `Damage` tag.
pub(crate) fn durability_left(stack: &ItemStack, tool: Tool) -> u16 {
    let damage = match stack.nbt.as_ref().and_then(|n| n.value.get("Damage")) {
        Some(Value::Int(d)) => *d,
        Some(Value::Short(d)) => i32::from(*d),
        Some(Value::Byte(d)) => i32::from(*d),
        _ => 0,
    };
    let left = i32::from(tool.max_durability()) - damage.max(0);
    u16::try_from(left.max(0)).unwrap_or(0)
}

/// The main slot (0-35) with the fewest break ticks, if fewer than by hand, among tools with at
/// least `spare` durability left. Ties go to the most durability left, then the held slot, then
/// the hotbar, then the lower slot.
pub(crate) fn fastest_slot<'a>(
    inv: &Inventory,
    name: impl Fn(&ItemStack) -> Option<&'a str>,
    mining: &Mining,
    base: BreakConditions,
    spare: u16,
) -> Option<u8> {
    let by_hand = break_ticks(mining, &base)?;
    let held = inv.selected_hotbar_slot;
    inv.main
        .iter()
        .enumerate()
        .filter(|(_, s)| !s.is_empty())
        .filter_map(|(i, stack)| {
            let tool = name(stack).and_then(Tool::from_identifier)?;
            let left = durability_left(stack, tool);
            if left < spare {
                return None;
            }
            let c = BreakConditions { tool: Some(tool), efficiency: enchantment_level(stack, EFFICIENCY), ..base };
            let ticks = break_ticks(mining, &c)?;
            let slot = i as u8;
            (ticks < by_hand).then_some((ticks, u16::MAX - left, slot != held, slot >= Inventory::HOTBAR_SLOTS, slot))
        })
        .min()
        .map(|(.., slot)| slot)
}
