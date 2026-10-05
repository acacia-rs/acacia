use acacia_client::proto::nbt::{Nbt, Value};
use acacia_client::proto::types::{
    GameMode, ItemStackRequestActionsItemContent as Content, ItemStackRequestActionsItemTypeId as TypeId, ItemStackRequestCause, WindowType,
};

use super::*;
use crate::items::{ui, SlotRef};
use crate::workstation::anvil::anvil_plan;
use crate::workstation::anvil_cost::anvil_outcome;
use crate::workstation::enchants::{enchants, with_enchant_list};
use crate::workstation::grindstone::{grindstone_plan, grindstone_result};
use crate::ActionError;

/// A stack with `(key, int)` NBT entries.
fn tagged(name: &str, stack_id: i32, ints: &[(&str, i32)]) -> ItemStack {
    let entries = ints.iter().map(|&(k, v)| (k.into(), Value::Int(v))).collect();
    ItemStack { nbt: Some(Nbt { name: String::new(), value: Value::Compound(entries) }), ..stack(name, 1, stack_id) }
}

fn enchanted(stack: ItemStack, list: &[(i16, i16)]) -> ItemStack {
    with_enchant_list(&stack, list)
}

fn int(stack: &ItemStack, key: &str) -> Option<i32> {
    match stack.nbt.as_ref()?.value.get(key) {
        Some(Value::Int(v)) => Some(*v),
        _ => None,
    }
}

fn anvil_state(level: i32) -> GameState {
    let mut state = state(&[]);
    open(&mut state, WindowType::Anvil);
    state.player.xp_level = level;
    state
}

fn refused<T: std::fmt::Debug>(result: Result<T, ActionError>) -> bool {
    matches!(result, Err(ActionError::NotPossible(_)))
}

#[test]
fn anvil_rename_carries_the_name_as_filter_string() {
    let mut state = anvil_state(30);
    state.inventory.ui[usize::from(ui::ANVIL_INPUT)] = stack("minecraft:diamond_sword", 1, 41);
    let request = encode(&state, &anvil_plan(&state, Some("Excalibur")).unwrap());
    // No CraftResultsDeprecated: BDS crashes on it in an anvil request.
    assert_eq!(kinds(&request), [(TypeId::Optional, 15), (TypeId::Consume, 5), (TypeId::Place, 1)]);
    let Content::Optional(optional) = &request.actions[0].content else { panic!() };
    assert_eq!(optional.filtered_string_index, 0);
    assert_eq!((request.custom_names.clone(), request.cause), (vec!["Excalibur".to_owned()], ItemStackRequestCause::AnvilText));
}

#[test]
fn anvil_repair_consumes_the_units_the_server_uses() {
    let mut state = anvil_state(30);
    state.inventory.ui[usize::from(ui::ANVIL_INPUT)] = tagged("minecraft:diamond_sword", 41, &[("Damage", 1000)]);
    state.inventory.ui[usize::from(ui::ANVIL_MATERIAL)] = stack("minecraft:diamond", 5, 42);
    let (craft, ops) = anvil_plan(&state, None).unwrap();
    // A quarter of 1561 (390) per diamond: three for 1000 damage. Material first, like vanilla.
    assert_eq!(ops[..2], [Op::Consume { from: SlotRef::Ui(ui::ANVIL_MATERIAL), count: 3 }, Op::Consume { from: SlotRef::Ui(ui::ANVIL_INPUT), count: 1 }]);
    let result = &craft.results[0].1;
    assert_eq!((int(result, "Damage"), int(result, "RepairCost")), (Some(0), Some(1)));

    state.inventory.ui[usize::from(ui::ANVIL_MATERIAL)] = tagged("minecraft:diamond_sword", 42, &[("Damage", 1000)]);
    let (craft, ops) = anvil_plan(&state, None).unwrap();
    assert_eq!(ops[0], Op::Consume { from: SlotRef::Ui(ui::ANVIL_MATERIAL), count: 1 });
    assert_eq!(int(&craft.results[0].1, "Damage"), Some(252));
}

#[test]
fn anvil_cost_and_repair_cost_follow_bds() {
    let state = anvil_state(30);
    let sword = |rc| tagged("minecraft:diamond_sword", 41, &[("Damage", 1000), ("RepairCost", rc)]);
    let diamonds = stack("minecraft:diamond", 5, 42);
    // 3 diamonds on RepairCost 3: level 6, RepairCost 3*2+1.
    let out = anvil_outcome(&state, &sword(3), Some(&diamonds), None).unwrap();
    assert_eq!((out.level, out.material_used, int(&out.result, "RepairCost")), (6, 3, Some(7)));
    // The material's RepairCost counts in the cost and the max of both is doubled.
    let other = tagged("minecraft:diamond_sword", 42, &[("Damage", 1000), ("RepairCost", 7)]);
    let out = anvil_outcome(&state, &sword(1), Some(&other), None).unwrap();
    assert_eq!((out.level, int(&out.result, "RepairCost")), (1 + 7 + 2, Some(15)));
    // A rename alone keeps RepairCost and is capped at 39 levels.
    let out = anvil_outcome(&anvil_state(39), &sword(63), None, Some("x")).unwrap();
    assert_eq!((out.level, out.material_used, int(&out.result, "RepairCost")), (39, 0, Some(63)));
    assert_eq!(out.result.custom_name.as_deref(), Some("x"));
    // Too expensive, above the bot's level, or nothing to do: the server makes no result.
    assert!(refused(anvil_outcome(&state, &sword(37), Some(&diamonds), None)), "40 levels");
    assert!(refused(anvil_outcome(&anvil_state(5), &sword(3), Some(&diamonds), None)), "needs 6");
    let whole = stack("minecraft:diamond_sword", 1, 41);
    assert!(refused(anvil_outcome(&state, &whole, Some(&diamonds), Some("x"))), "an undamaged item with its material");
    assert!(refused(anvil_outcome(&state, &whole, Some(&stack("minecraft:stone", 1, 43)), None)), "unrelated material");
    let mut creative = anvil_state(0);
    creative.player.game_mode = GameMode::Creative;
    assert!(anvil_outcome(&creative, &sword(37), Some(&diamonds), None).is_ok(), "creative skips the level checks");
}

#[test]
fn anvil_merges_enchantments() {
    let state = anvil_state(30);
    let sharp = |lvl, id| enchanted(stack("minecraft:diamond_sword", 1, id), &[(9, lvl), (17, 1)]);
    // Equal levels go up one; the same item without damage costs only the enchantment value.
    let material = enchanted(stack("minecraft:diamond_sword", 1, 42), &[(9, 2)]);
    let out = anvil_outcome(&state, &sharp(2, 41), Some(&material), None).unwrap();
    assert_eq!(enchants(&out.result), [(9, 3), (17, 1)]);
    assert_eq!((out.level, out.material_used, int(&out.result, "RepairCost")), (1, 1, Some(1)));
    // A book adds its enchantment at book weights: mending (rare) 2 per level.
    let book = enchanted(stack("minecraft:enchanted_book", 1, 43), &[(26, 1)]);
    let out = anvil_outcome(&state, &sharp(5, 41), Some(&book), None).unwrap();
    assert_eq!((enchants(&out.result), out.level), (vec![(9, 5), (17, 1), (26, 1)], 2));
    // Smite next to sharpness is exclusive.
    let smite = enchanted(stack("minecraft:enchanted_book", 1, 43), &[(10, 1)]);
    assert!(refused(anvil_outcome(&state, &sharp(1, 41), Some(&smite), None)));
}

#[test]
fn grindstone_keeps_curses_and_prices_them() {
    let mut state = state(&[]);
    open(&mut state, WindowType::Grindstone);
    state.inventory.ui[usize::from(ui::GRINDSTONE_INPUT)] = enchanted(stack("minecraft:diamond_sword", 1, 51), &[(9, 2)]);
    let request = encode(&state, &grindstone_plan(&state).unwrap());
    assert_eq!(kinds(&request), [(TypeId::CraftGrindstoneRequest, 16), (TypeId::ResultsDeprecated, 19), (TypeId::Consume, 5), (TypeId::Place, 1)]);
    let Content::CraftGrindstoneRequest(grind) = &request.actions[0].content else { panic!() };
    assert_eq!((grind.recipe_network_id, grind.times_crafted, grind.cost), (51, 1, 0), "names the input stack");
    let (craft, _) = grindstone_plan(&state).unwrap();
    assert_eq!((enchants(&craft.results[0].1), int(&craft.results[0].1, "RepairCost")), (vec![], Some(0)), "as BDS makes it");

    let cursed = enchanted(stack("minecraft:diamond_sword", 1, 51), &[(9, 2), (28, 1), (27, 1)]);
    let result = grindstone_result(&state, &cursed, None).unwrap();
    assert_eq!((enchants(&result), int(&result, "RepairCost")), (vec![(28, 1), (27, 1)], Some(3)));
    state.inventory.ui[usize::from(ui::GRINDSTONE_INPUT)] = cursed;
    let request = encode(&state, &grindstone_plan(&state).unwrap());
    let Content::CraftGrindstoneRequest(grind) = &request.actions[0].content else { panic!() };
    assert_eq!(grind.cost, 3, "cost = the result's RepairCost");

    let book = enchanted(stack("minecraft:enchanted_book", 1, 52), &[(26, 1)]);
    let result = grindstone_result(&state, &book, None).unwrap();
    assert_eq!((result.network_id, result.nbt.is_none()), (id("minecraft:book"), true));

    let (a, b) = (tagged("minecraft:diamond_sword", 51, &[("Damage", 1000)]), tagged("minecraft:diamond_sword", 52, &[("Damage", 1000)]));
    assert_eq!(int(&grindstone_result(&state, &a, Some(&b)).unwrap(), "Damage"), Some(1000 + 1000 - 1561 - 78));
    assert!(refused(grindstone_result(&state, &a, Some(&stack("minecraft:diamond", 1, 53)))));
}
