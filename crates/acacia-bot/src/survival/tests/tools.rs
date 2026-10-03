use acacia_client::proto::nbt::{Nbt, Value};
use acacia_world::Tool;

use super::mining;
use crate::interact::BreakConditions;
use crate::state::{Inventory, ItemStack};
use crate::survival::tools::{durability_left, fastest_slot};

fn damaged(network_id: i32, damage: Value) -> ItemStack {
    let nbt = Nbt { name: String::new(), value: Value::Compound(vec![("Damage".into(), damage)]) };
    ItemStack { network_id, count: 1, nbt: Some(nbt), ..ItemStack::default() }
}

#[test]
fn durability_from_the_damage_tag() {
    let pickaxe = Tool::from_identifier("minecraft:iron_pickaxe").unwrap();
    assert_eq!(pickaxe.max_durability(), 250);
    assert_eq!(Tool::from_identifier("minecraft:shears").unwrap().max_durability(), 238);
    assert_eq!(durability_left(&ItemStack { network_id: 1, count: 1, ..ItemStack::default() }, pickaxe), 250);
    assert_eq!(durability_left(&damaged(1, Value::Int(200)), pickaxe), 50);
    assert_eq!(durability_left(&damaged(1, Value::Short(250)), pickaxe), 0);
    assert_eq!(durability_left(&damaged(1, Value::Int(300)), pickaxe), 0);
}

#[test]
fn best_tool_spares_worn_tools_and_prefers_durability() {
    let names = [(1, "minecraft:iron_pickaxe"), (2, "minecraft:stone_pickaxe")];
    let name = |s: &ItemStack| names.iter().find(|(id, _)| *id == s.network_id).map(|(_, n)| *n);
    let stone = mining("minecraft:stone");
    let base = BreakConditions { on_ground: true, ..BreakConditions::default() };
    let mut inv = Inventory::default();
    inv.main[0] = damaged(1, Value::Int(240));
    inv.main[1] = damaged(1, Value::Int(100));
    inv.main[2] = damaged(2, Value::Int(0));
    assert_eq!(fastest_slot(&inv, name, &stone, base, 1), Some(1), "equally fast: the less worn one");
    inv.main[1] = damaged(1, Value::Int(250));
    assert_eq!(fastest_slot(&inv, name, &stone, base, 1), Some(0), "the next use would break slot 1");
    assert_eq!(fastest_slot(&inv, name, &stone, base, 20), Some(2), "slot 0 is below the spare durability");
    assert_eq!(fastest_slot(&inv, name, &stone, base, 0), Some(0), "with nothing spared, the worn-out copy still loses");
}
