use acacia_client::proto::types::{Enchant, WindowID, WindowType};

use super::*;
use crate::state::queries::test_support::{fixtures, offer, raw, update_trade};

#[test]
fn trade_offers_from_nbt() {
    let mut stations = Stations::default();
    let mut discounted = offer(("minecraft:emerald", 9), Some(("minecraft:book", 1)), ("minecraft:enchanted_book", 1), 2);
    if let Value::Compound(entries) = &mut discounted {
        entries.retain(|(k, _)| k != "buyCountA");
        entries.push(("buyCountA".into(), Value::Int(5)));
    }
    let packet = update_trade(vec![offer(("minecraft:paper", 24), None, ("minecraft:emerald", 1), 1), discounted]);
    stations.apply(&raw(&packet)).unwrap();

    let trade = stations.trade.as_ref().unwrap();
    assert_eq!((trade.window_id, trade.trader, trade.tier), (1, 77, 1));
    let paper = &trade.offers[0];
    assert_eq!((paper.buy_a.name.as_str(), paper.buy_a.count, paper.buy_b.clone()), ("minecraft:paper", 24, None));
    assert_eq!((paper.sell.name.as_str(), paper.recipe_network_id, paper.max_uses), ("minecraft:emerald", 1, 12));
    let book = &trade.offers[1];
    assert_eq!(book.buy_a.count, 5, "buyCountA is the price after discounts");
    assert_eq!(book.buy_b.as_ref().map(|b| b.name.as_str()), Some("minecraft:book"));
    assert!(!book.is_disabled());

    stations.apply(&raw(&ContainerClose { window_id: WindowID::First, window_type: WindowType::Trading, server: true })).unwrap();
    assert!(stations.trade.is_none());
}

#[test]
fn enchant_option_ids_are_unsigned_varints() {
    // One option as BDS writes it: cost 3, slot flags, no enchants, name "ab", option id 4851.
    let body = [1, 3, 0, 0, 0, 0, 0, 0, 0, 2, b'a', b'b', 0xf3, 0x25];
    let packet = RawPacket { id: PlayerEnchantOptions::ID, sender_subclient: 0, target_subclient: 0, body: body.to_vec().into() };
    let mut stations = Stations::default();
    stations.apply(&packet).unwrap();
    assert_eq!((stations.enchant_options[0].cost, stations.enchant_options[0].recipe_network_id), (3, 4851));
}

#[test]
fn enchant_options_until_close() {
    let mut stations = Stations::default();
    let option = |cost, id| acacia_client::proto::types::EnchantOption {
        cost,
        slot_flags: 0,
        equip_enchants: vec![],
        held_enchants: vec![Enchant { id: 9, level: 1 }],
        self_enchants: vec![],
        name: "galactic".into(),
        option_id: id,
    };
    stations.apply(&raw(&PlayerEnchantOptions { options: vec![option(1, 100_000), option(5, 100_001), option(12, 100_002)] })).unwrap();
    let options = &stations.enchant_options;
    assert_eq!(options.len(), 3);
    assert_eq!((options[1].cost, options[1].recipe_network_id, options[1].enchants.clone()), (5, 100_001, vec![(9, 1)]));
    stations.apply(&raw(&ContainerClose { window_id: WindowID::First, window_type: WindowType::Enchantment, server: false })).unwrap();
    assert!(stations.enchant_options.is_empty());
}

#[test]
fn decodes_fixtures() {
    let mut stations = Stations::default();
    for packet in fixtures::<UpdateTrade>().into_iter().chain(fixtures::<PlayerEnchantOptions>()) {
        stations.apply(&packet).unwrap();
    }
}
