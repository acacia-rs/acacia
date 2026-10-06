//! The anvil result as BDS 1.26.52 computes it: repair, combine, enchantment merge, rename, level cost
//! and RepairCost. BDS sends no slot update, and an empty result fails the request (status 9), so
//! the bot refuses what the server would not make.

use acacia_client::proto::nbt::Value;
use acacia_client::proto::types::GameMode;

use super::anvil::{nbt_int, set_nbt};
use super::enchants::{anvil_value, enchants, merge, with_enchant_list};
use super::repair;
use crate::state::{GameState, ItemStack};
use crate::ActionError;

const ENCHANTED_BOOK: &str = "minecraft:enchanted_book";
/// Levels at which a non-rename craft is "too expensive"; a rename alone is capped below it.
const TOO_EXPENSIVE: i32 = 40;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AnvilOutcome {
    pub result: ItemStack,
    /// Items the craft uses up from the material slot.
    pub material_used: u16,
    /// Levels the craft costs.
    pub level: i32,
}

/// What the anvil makes of `item` with `material` and an optional new name (`""` clears one).
pub(crate) fn anvil_outcome(state: &GameState, item: &ItemStack, material: Option<&ItemStack>, rename: Option<&str>) -> Result<AnvilOutcome, ActionError> {
    let not_possible = |why: &str| ActionError::NotPossible(why.to_owned());
    let base = nbt_int(item, "RepairCost") + material.map_or(0, |m| nbt_int(m, "RepairCost"));
    let mut result = item.clone();
    let (mut cost, mut material_used) = (0, 0);
    if let Some(added) = material {
        (cost, material_used) = apply_material(state, &mut result, item, added)?;
        if cost < 1 {
            return Err(not_possible("the material changes nothing at the anvil"));
        }
    }
    let mut rename_cost = 0;
    match rename {
        Some("") if item.custom_name.is_some() => {
            result.custom_name = None;
            remove_nbt(&mut result, "display");
            cost += 1;
        }
        Some(name) if !name.is_empty() && item.custom_name.as_deref() != Some(name) => {
            result.custom_name = Some(name.to_owned());
            set_nbt(&mut result, "display", Value::Compound(vec![("Name".into(), Value::String(name.into()))]));
            cost += 1;
            rename_cost = 1;
        }
        _ => {}
    }
    if cost == 0 {
        return Err(not_possible("nothing to do at the anvil"));
    }
    let creative = state.player.game_mode == GameMode::Creative;
    let total = base + cost;
    let rename_only = cost == rename_cost;
    let level = if rename_only { total.min(TOO_EXPENSIVE - 1) } else { total };
    if !creative && !rename_only && total >= TOO_EXPENSIVE {
        return Err(not_possible(&format!("too expensive ({total} levels)")));
    }
    if !creative && level > state.player.xp_level {
        return Err(not_possible(&format!("the anvil craft needs level {level}")));
    }
    let kept = nbt_int(item, "RepairCost").max(material.map_or(0, |m| nbt_int(m, "RepairCost")));
    set_nbt(&mut result, "RepairCost", Value::Int(if rename_only { kept } else { kept * 2 + 1 }));
    Ok(AnvilOutcome { result, material_used, level })
}

/// Repairs or combines `result` (a copy of `item`) with `added`; returns the cost and the
/// material it uses up.
fn apply_material(state: &GameState, result: &mut ItemStack, item: &ItemStack, added: &ItemStack) -> Result<(i32, u16), ActionError> {
    let name_of = |s: &ItemStack| state.items.name(s.network_id).unwrap_or_default();
    let (item_name, added_name) = (name_of(item), name_of(added));
    let max = repair::max_durability(item_name);
    let damage = nbt_int(item, "Damage").max(0) as u16;
    if let Some(max) = max.filter(|_| repair::repairs(item_name, added_name)) {
        let units = repair::repair_units(max, damage, added.count);
        let left = (0..units).fold(damage, |d, _| d - d.min(max / 4));
        set_nbt(result, "Damage", Value::Int(i32::from(left)));
        return Ok((i32::from(units), units));
    }
    let book = added_name == ENCHANTED_BOOK;
    if added_name != item_name && !book {
        return Err(ActionError::NotPossible(format!("{added_name} does not combine with {item_name}")));
    }
    let mut cost = 0;
    if let Some(max) = max.filter(|_| !book) {
        let combined = repair::combined_damage(max, damage, nbt_int(added, "Damage").max(0) as u16);
        if combined < damage {
            set_nbt(result, "Damage", Value::Int(i32::from(combined)));
            cost += 2;
        }
    }
    let before = enchants(item);
    let merged = merge(&before, &enchants(added)).map_err(ActionError::NotPossible)?;
    let delta = anvil_value(&merged, book) - anvil_value(&before, book);
    if delta > 0 {
        *result = with_enchant_list(result, &merged);
        cost += if item.count > 1 { TOO_EXPENSIVE } else { delta };
    }
    Ok((cost, item.count))
}

fn remove_nbt(stack: &mut ItemStack, key: &str) {
    if let Some(Value::Compound(entries)) = stack.nbt.as_mut().map(|n| &mut n.value) {
        entries.retain(|(k, _)| k != key);
    }
}
