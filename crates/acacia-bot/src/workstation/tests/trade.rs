use acacia_client::proto::nbt::Value;
use acacia_client::proto::types::{
    ContainerSlotType as T, ItemStackRequestActionsItemContent as Content, ItemStackRequestActionsItemTypeId as TypeId, WindowType,
};

use super::*;
use crate::items::{ui, SlotRef};
use crate::state::queries::test_support::{offer, update_trade};
use crate::workstation::trade::{fill_ops, trade_plan};
use crate::ActionError;

/// The captured wandering-trader offer: 1 emerald for packed ice, with a stack of 32 in slot 19.
fn trader_state() -> GameState {
    let mut state = state(&[(19, stack("minecraft:emerald", 32, 40))]);
    state.apply(&raw(&update_trade(vec![offer(("minecraft:emerald", 1), None, ("minecraft:packed_ice", 1), 4771)]))).unwrap();
    state
}

#[test]
fn trade_pays_in_the_whole_stack_as_the_auto_fill() {
    let trader = trader_state();
    assert_eq!(trader.containers.open.as_ref().map(|c| c.window_type), Some(WindowType::Trading), "UpdateTrade opens the screen");
    assert_eq!(fill_ops(&trader, 0).unwrap(), [Op::Transfer { from: SlotRef::Main(19), to: SlotRef::Ui(ui::TRADE_INGREDIENT_1), count: 32 }]);
    assert!(matches!(fill_ops(&trader, 1), Err(ActionError::NotPossible(_))));

    let mut two = state(&[(0, stack("minecraft:paper", 30, 71)), (5, stack("minecraft:paper", 50, 72))]);
    two.apply(&raw(&update_trade(vec![offer(("minecraft:paper", 40), None, ("minecraft:emerald", 1), 3)]))).unwrap();
    let fill = fill_ops(&two, 0).unwrap();
    assert_eq!(fill[1], Op::Transfer { from: SlotRef::Main(5), to: SlotRef::Ui(ui::TRADE_INGREDIENT_1), count: 34 }, "one slot holds 64");
}

#[test]
fn trade_takes_the_result_onto_the_cursor() {
    let mut state = trader_state();
    state.inventory.main[19] = ItemStack::default();
    state.inventory.ui[usize::from(ui::TRADE_INGREDIENT_1)] = stack("minecraft:emerald", 32, 40);
    assert!(fill_ops(&state, 0).unwrap().is_empty(), "already paid");

    let request = encode(&state, &trade_plan(&state, 0).unwrap());
    assert_eq!(kinds(&request), [(TypeId::CraftRecipe, 12), (TypeId::ResultsDeprecated, 19), (TypeId::Consume, 5), (TypeId::Take, 0)]);
    let Content::CraftRecipe(craft) = &request.actions[0].content else { panic!() };
    assert_eq!((craft.recipe_network_id, craft.times_crafted), (4771, 1));
    let Content::Consume(consume) = &request.actions[2].content else { panic!() };
    assert_eq!((consume.count, consume.source.clone()), (1, info(T::Trade2Ingredient1, ui::TRADE_INGREDIENT_1, 40)));
    let Content::Take(take) = &request.actions[3].content else { panic!() };
    assert_eq!((take.count, take.source.clone(), take.destination.clone()), (1, info(T::CreativeOutput, 50, REQUEST), info(T::Cursor, 0, 0)));

    // The next trade stacks onto the cursor, naming its stack.
    state.inventory.ui[0] = ItemStack { count: 1, ..stack("minecraft:packed_ice", 1, 65) };
    let request = encode(&state, &trade_plan(&state, 0).unwrap());
    let Content::Take(take) = &request.actions[3].content else { panic!() };
    assert_eq!(take.destination, info(T::Cursor, 0, 65));
}

#[test]
fn trade_payment_aux_values() {
    let emeralds = |metadata, damage: i16| {
        let mut state = state(&[(0, ItemStack { metadata, ..stack("minecraft:emerald", 32, 71) })]);
        let mut wanted = offer(("minecraft:emerald", 3), None, ("minecraft:paper", 1), 4);
        if let Value::Compound(entries) = &mut wanted
            && let Some((_, Value::Compound(buy_a))) = entries.iter_mut().find(|(k, _)| k == "buyA")
        {
            buy_a.retain(|(k, _)| k != "Damage");
            buy_a.push(("Damage".into(), Value::Short(damage)));
        }
        state.apply(&raw(&update_trade(vec![wanted]))).unwrap();
        fill_ops(&state, 0)
    };
    // BDS sends Damage 32767 for "any aux value".
    let paid = [Op::Transfer { from: SlotRef::Main(0), to: SlotRef::Ui(ui::TRADE_INGREDIENT_1), count: 32 }];
    assert_eq!(emeralds(1, i16::MAX).unwrap(), paid);
    assert_eq!(emeralds(0, 0).unwrap(), paid);
    // 0 is a real value (water bottle vs awkward potion).
    assert!(matches!(emeralds(1, 0), Err(ActionError::NotPossible(_))));
}
