//! Grindstone (`CraftGrindstone`): BDS 1.26.52's rules.
//! The result loses every enchantment but its curses, gets `RepairCost` from the curse count, and
//! an enchanted book without curses becomes a plain book. BDS sends no slot update.

use acacia_client::proto::nbt::Value;
use acacia_client::proto::types::WindowType;
use acacia_physics::BlockPos;

use super::anvil::{nbt_int, set_nbt};
use super::enchants::{enchants, is_curse, with_enchant_list};
use super::{named, occupied, repair};
use crate::items::craft::{Craft, CraftAction};
use crate::items::{to_inventory_ops, ui, Op, SlotRef};
use crate::state::{GameState, ItemStack};
use crate::{ActionError, Bot};

const ENCHANTED_BOOK: &str = "minecraft:enchanted_book";
const BOOK: &str = "minecraft:book";
/// The result's `RepairCost` (and the request's `cost`) by its number of curses.
const REPAIR_COST_BY_CURSES: [i32; 4] = [0, 1, 3, 0];

impl Bot {
    /// Disenchants `input`, or merges it with `additional` (the same damageable item), at the
    /// grindstone at `pos`.
    pub async fn grindstone(&mut self, pos: BlockPos, input: SlotRef, additional: Option<SlotRef>) -> Result<(), ActionError> {
        let slots = [SlotRef::Ui(ui::GRINDSTONE_INPUT), SlotRef::Ui(ui::GRINDSTONE_ADDITIONAL)];
        self.workstation_craft(pos, WindowType::Grindstone, &[Some(input), additional], &slots, grindstone_plan).await
    }
}

pub(crate) fn grindstone_plan(state: &GameState) -> Result<(Craft, Vec<Op>), ActionError> {
    let (input, additional) = (SlotRef::Ui(ui::GRINDSTONE_INPUT), SlotRef::Ui(ui::GRINDSTONE_ADDITIONAL));
    let item = occupied(state, input)?;
    let other = occupied(state, additional).ok();
    let result = grindstone_result(state, item, other)?;
    let cost = nbt_int(&result, "RepairCost");
    // The "recipe" id is an ItemStackNetIdVariant: the input stack's id (BDS: status 9 with 0). The server works out the XP.
    let action = CraftAction::Grindstone { network_id: item.stack_network_id.unwrap_or(0), times: 1, cost };
    let craft = Craft::new(action, vec![named(state, &result)?]).with_results_action();
    let mut ops = vec![Op::Consume { from: input, count: 1 }];
    if other.is_some() {
        ops.push(Op::Consume { from: additional, count: 1 });
    }
    ops.extend(to_inventory_ops(state, SlotRef::CREATED_OUTPUT, &result)?);
    Ok((craft, ops))
}

/// What the grindstone makes of `item` (and `other`).
pub(crate) fn grindstone_result(state: &GameState, item: &ItemStack, other: Option<&ItemStack>) -> Result<ItemStack, ActionError> {
    let name = state.items.name(item.network_id).unwrap_or_default();
    let mut result = item.clone();
    if let Some(other) = other {
        let max = repair::max_durability(name).filter(|_| other.network_id == item.network_id);
        let Some(max) = max else {
            return Err(ActionError::NotPossible("the grindstone merges only two of the same damageable item".into()));
        };
        let damage = |s: &ItemStack| nbt_int(s, "Damage").max(0) as u16;
        set_nbt(&mut result, "Damage", Value::Int(i32::from(repair::grindstone_damage(max, damage(item), damage(other)))));
    }
    let mut curses: Vec<(i16, i16)> = Vec::new();
    for (id, lvl) in std::iter::once(item).chain(other).flat_map(enchants).filter(|(id, _)| is_curse(*id)) {
        if !curses.iter().any(|(c, _)| *c == id) {
            curses.push((id, lvl));
        }
    }
    if name == ENCHANTED_BOOK && curses.is_empty() {
        let book = state.items.id(BOOK).ok_or_else(|| ActionError::NotPossible(format!("{BOOK} is not in the registry")))?;
        return Ok(ItemStack { network_id: book, count: item.count, ..ItemStack::default() });
    }
    let mut result = with_enchant_list(&result, &curses);
    set_nbt(&mut result, "RepairCost", Value::Int(REPAIR_COST_BY_CURSES[curses.len().min(3)]));
    Ok(result)
}
