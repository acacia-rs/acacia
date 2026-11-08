mod fetch;
mod tools;

use acacia_client::proto::packets::InventoryTransaction;
use acacia_client::proto::types::{
    TransactionTransactionData as Data, TransactionUseItemActionType, TransactionUseItemTriggerType,
};
use acacia_world::neutral::Item;
use acacia_world::{BlockRegistry, Mining};

use super::auto_eat::{Act, AutoEater, Ctx, Delay};
use super::equip::{armor_slot, hotbar_target};
use super::food::{always_consumable, choose_food, food, use_ticks, FoodChoice};
use super::item_use::{eating_data, ItemUse, Snapshot, UseTick};
use super::tools::fastest_slot;
use crate::interact::{to_wire, BreakConditions};
use crate::interact::wire::{self, Hand};
use crate::state::{Inventory, ItemStack};
use crate::ActionError;

#[test]
fn food_values_and_durations() {
    assert_eq!(food(Item::CookedBeef).map(|f| (f.hunger, f.saturation)), Some((8, 12.8)));
    assert!(food(Item::RottenFlesh).unwrap().harmful);
    assert!(food(Item::GoldenApple).unwrap().precious);
    assert_eq!(food(Item::Stone), None);
    assert_eq!(use_ticks(Item::Bread), Some(32));
    assert_eq!(use_ticks(Item::DriedKelp), Some(16));
    assert_eq!(use_ticks(Item::HoneyBottle), Some(40));
    assert_eq!(use_ticks(Item::Potion), Some(32));
    assert_eq!(use_ticks(Item::SplashPotion), None);
    assert!(always_consumable(Item::MilkBucket) && always_consumable(Item::GoldenApple));
    assert!(!always_consumable(Item::Bread));
}

#[test]
fn food_choice() {
    let hotbar = || [(0, Item::RottenFlesh), (1, Item::Bread), (2, Item::CookedBeef), (3, Item::GoldenApple)].into_iter();
    assert_eq!(choose_food(hotbar(), 10.0, FoodChoice::Saturation), Some(2));
    // Beef (8) would overflow a bar missing 6; bread (5) fits.
    assert_eq!(choose_food(hotbar(), 6.0, FoodChoice::NoWaste), Some(1));
    assert_eq!(choose_food(hotbar(), 9.0, FoodChoice::NoWaste), Some(2));
    // Nothing fits: the smallest meal wastes least.
    assert_eq!(choose_food(hotbar(), 2.0, FoodChoice::NoWaste), Some(1));
    assert_eq!(choose_food([(0, Item::SpiderEye), (4, Item::EnchantedGoldenApple)].into_iter(), 10.0, FoodChoice::Saturation), None);
}

fn snapshot(count: u16, hunger: f32) -> Snapshot {
    Snapshot { network_id: 7, count, hunger, saturation: 1.0 }
}

/// Ticks `use` until it returns something other than `Nothing`; (ticks taken, result).
fn run_until(u: &mut ItemUse, now: &Snapshot) -> (u32, UseTick) {
    for n in 1..200 {
        match u.tick(now, 3, true) {
            UseTick::Nothing => {}
            other => return (n, other),
        }
    }
    panic!("item use never progressed");
}

#[test]
fn item_use_eats_then_releases() {
    let before = snapshot(5, 10.0);
    let mut u = ItemUse::new(3, before, 32, 2, false);
    let mut effects = 0;
    let finish_tick = loop {
        let (n, tick) = run_until(&mut u, &before);
        effects += 1;
        if tick == UseTick::Finish {
            break n;
        }
        assert_eq!(tick, UseTick::Effect);
    };
    // Java's cadence: remaining ticks 24, 20, ..., 4 → 6 effects, then the finish on tick 33.
    assert_eq!(effects - 1, 6);
    assert_eq!(finish_tick, 5, "the last effect is on tick 28");
    let after = snapshot(4, 15.0);
    assert_eq!(u.tick(&after, 3, true), UseTick::Nothing, "server applied it: start releasing");
    assert_eq!(u.tick(&after, 3, true), UseTick::Nothing);
    assert_eq!(u.tick(&after, 3, true), UseTick::Nothing);
    assert_eq!(u.tick(&after, 3, true), UseTick::Done(Ok(())));
}

#[test]
fn item_use_without_server_reply_is_rejected_and_interruptions_end_it() {
    let before = snapshot(5, 10.0);
    let mut u = ItemUse::new(3, before, 16, 0, true);
    while u.tick(&before, 3, true) != UseTick::Finish {}
    let (n, tick) = run_until(&mut u, &before);
    assert!(matches!(tick, UseTick::Done(Err(ActionError::Rejected(_)))), "{tick:?}");
    assert!(n > 40);

    let mut u = ItemUse::new(3, before, 2, 0, false);
    let eaten = snapshot(4, 15.0);
    assert_eq!([u.tick(&before, 3, true), u.tick(&before, 3, true), u.tick(&eaten, 3, true)], [UseTick::Nothing, UseTick::Nothing, UseTick::Nothing], "BDS ate it already: no finish");
    assert_eq!(u.tick(&eaten, 3, true), UseTick::Nothing);
    assert_eq!(u.tick(&eaten, 3, true), UseTick::Done(Ok(())));

    let mut u = ItemUse::new(3, before, 32, 0, false);
    assert!(matches!(u.tick(&before, 4, true), UseTick::Done(Err(ActionError::NotPossible(_)))));
    let mut u = ItemUse::new(3, before, 32, 0, false);
    assert!(matches!(u.tick(&before, 3, false), UseTick::Done(Err(_))));
}

#[test]
fn eating_wire() {
    assert_eq!(eating_data(257, 0), 257 << 16);
    assert_eq!(eating_data(300, 2), (300 << 16) | 2);
    let hand = || Hand { slot: 4, item: to_wire(&ItemStack { network_id: 257, count: 3, ..ItemStack::default() }), eye: [0.0, 65.62, 0.0] };
    let check = |tx: InventoryTransaction, trigger| {
        let Data::ItemUse(u) = tx.transaction.transaction_data else { panic!("not a use-item transaction") };
        assert_eq!((u.action_type, u.trigger_type, u.face, u.hotbar_slot), (TransactionUseItemActionType::ClickAir, trigger, 255, 4));
        assert_eq!((u.block_position.x, u.block_position.y, u.block_position.z), (0, 0, 0));
        assert_eq!(u.held_item.network_id, 257);
    };
    check(wire::finish_use(hand()), TransactionUseItemTriggerType::SimulationTick);
    check(wire::click_air(hand()), TransactionUseItemTriggerType::UnknownValue);
}

fn ctx(hungry: bool, free: bool, using: bool, food: Option<u8>) -> Ctx {
    Ctx { hungry, free, using, selected: 0, food, stored: None }
}

#[test]
fn auto_eat_switches_eats_and_switches_back() {
    let mut eater = AutoEater::default();
    let delay = |d: Delay| if d == Delay::Notice { 2 } else { 1 };
    let hungry = ctx(true, true, false, Some(5));
    assert_eq!(eater.tick(&hungry, delay), None, "notices");
    assert_eq!(eater.tick(&hungry, delay), None);
    assert_eq!(eater.tick(&hungry, delay), None);
    assert_eq!(eater.tick(&hungry, delay), Some(Act::Select(5)));
    assert_eq!(eater.tick(&ctx(true, true, false, Some(5)), delay), None);
    assert_eq!(eater.tick(&ctx(true, true, false, Some(5)), delay), Some(Act::Eat));
    assert_eq!(eater.tick(&ctx(true, false, true, Some(5)), delay), None, "eating");
    assert_eq!(eater.tick(&ctx(false, true, false, None), delay), None, "done: wait to switch back");
    assert_eq!(eater.tick(&ctx(false, true, false, None), delay), None);
    assert_eq!(eater.tick(&ctx(false, true, false, None), delay), Some(Act::Select(0)));
    assert_eq!(eater.tick(&ctx(false, true, false, None), delay), None);
}

#[test]
fn auto_eat_waits_for_a_free_bot_and_backs_off_without_food() {
    let mut eater = AutoEater::default();
    let delay = |_: Delay| 0;
    assert_eq!(eater.tick(&ctx(true, false, false, Some(0)), delay), None, "an action is running");
    assert_eq!(eater.tick(&ctx(true, true, false, Some(0)), delay), None);
    assert_eq!(eater.tick(&ctx(true, true, false, Some(0)), delay), Some(Act::Eat), "food already held");
    eater.eat_failed();
    for _ in 0..100 {
        assert_eq!(eater.tick(&ctx(true, true, false, Some(0)), delay), None, "cooling down");
    }
    let mut eater = AutoEater::default();
    eater.tick(&ctx(true, true, false, None), delay);
    assert_eq!(eater.tick(&ctx(true, true, false, None), delay), None);
    assert_eq!(eater.tick(&ctx(true, true, false, Some(1)), delay), None, "no food found: cooling down");
}

#[test]
fn armour_slots_and_hotbar_target() {
    assert_eq!(armor_slot(Item::IronHelmet), Some(0));
    assert_eq!(armor_slot(Item::TurtleHelmet), Some(0));
    assert_eq!(armor_slot(Item::CarvedPumpkin), Some(0));
    assert_eq!(armor_slot(Item::CreeperHead), Some(0));
    assert_eq!(armor_slot(Item::Elytra), Some(1));
    assert_eq!(armor_slot(Item::NetheriteLeggings), Some(2));
    assert_eq!(armor_slot(Item::LeatherBoots), Some(3));
    assert_eq!(armor_slot(Item::DiamondSword), None);
    // The suffix rule this replaced counted every `*_head` and `*_skull` item; the tag must agree.
    for item in Item::all() {
        let n = &item.name()[10..];
        let by_name = n.ends_with("_helmet") || n == "carved_pumpkin" || n.ends_with("_head") || n.ends_with("_skull");
        assert_eq!(armor_slot(item) == Some(0), by_name, "{item}");
    }

    let mut inv = Inventory { selected_hotbar_slot: 4, ..Inventory::default() };
    for slot in 0..3 {
        inv.main[slot] = ItemStack { network_id: 1, count: 1, ..ItemStack::default() };
    }
    assert_eq!(hotbar_target(&inv), 3);
    for slot in 0..9 {
        inv.main[slot] = ItemStack { network_id: 1, count: 1, ..ItemStack::default() };
    }
    assert_eq!(hotbar_target(&inv), 4);
}

fn mining(block: &str) -> Mining {
    BlockRegistry::vanilla().states_of(block).next().unwrap().1.mining
}

#[test]
fn fastest_tool() {
    let names = [(1, "minecraft:wooden_pickaxe"), (2, "minecraft:diamond_pickaxe"), (3, "minecraft:diamond_shovel"), (4, "minecraft:stick")];
    let name = |s: &ItemStack| names.iter().find(|(id, _)| *id == s.network_id).map(|(_, n)| *n);
    let item = |id| ItemStack { network_id: id, count: 1, ..ItemStack::default() };
    let mut inv = Inventory::default();
    inv.main[0] = item(4);
    inv.main[2] = item(1);
    inv.main[20] = item(2);
    inv.main[5] = item(3);
    let base = BreakConditions { on_ground: true, ..BreakConditions::default() };
    assert_eq!(fastest_slot(&inv, name, &mining("minecraft:stone"), base, 1), Some(20));
    assert_eq!(fastest_slot(&inv, name, &mining("minecraft:dirt"), base, 1), Some(5));
    // Nothing beats the hand on instantly-broken blocks; bedrock cannot be broken at all.
    assert_eq!(fastest_slot(&inv, name, &mining("minecraft:short_grass"), base, 1), None);
    assert_eq!(fastest_slot(&inv, name, &mining("minecraft:bedrock"), base, 1), None);
    // Equal tools: the hotbar copy wins over the inventory one, the held one over both.
    inv.main[20] = item(1);
    assert_eq!(fastest_slot(&inv, name, &mining("minecraft:stone"), base, 1), Some(2));
    inv.main[7] = item(1);
    inv.selected_hotbar_slot = 7;
    assert_eq!(fastest_slot(&inv, name, &mining("minecraft:stone"), base, 1), Some(7));
}
