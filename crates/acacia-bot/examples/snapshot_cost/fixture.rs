//! A realistic `GameState`: the FakeServer BDS spawn script, then a full inventory, effects and players.

use acacia_bot::GameState;
use acacia_bot::proto::manual::{self, Uuid};
use acacia_bot::proto::nbt::{List, Nbt, Value};
use acacia_bot::proto::packets::{InventoryContent, ItemRegistry, MobEffect, MobEffectEventId, PlayerList};
use acacia_bot::proto::types::{
    ContainerSlotType, FullContainerName, ItemExtraDataWithoutBlockingTick, ItemExtraDataWithoutBlockingTickHasNbt,
    ItemExtraDataWithoutBlockingTickNbt, ItemV4, ItemV4Extra, PlayerRecord, PlayerRecordContent, WindowIDVarint,
};
use acacia_bot::proto::{Packet, RawPacket, encode_packet};
use acacia_testserver::{Script, Step};
use bytes::BytesMut;

pub const PLAYERS: u8 = 20;

const ITEMS: &[&str] = &[
    "minecraft:diamond_sword", "minecraft:diamond_pickaxe", "minecraft:iron_axe", "minecraft:cobblestone",
    "minecraft:oak_log", "minecraft:torch", "minecraft:cooked_beef", "minecraft:bread", "minecraft:water_bucket",
    "minecraft:dirt", "minecraft:oak_planks", "minecraft:stick", "minecraft:coal", "minecraft:iron_ingot",
    "minecraft:gold_ingot", "minecraft:redstone", "minecraft:arrow", "minecraft:bow",
];
const ARMOR: &[&str] =
    &["minecraft:diamond_helmet", "minecraft:diamond_chestplate", "minecraft:diamond_leggings", "minecraft:diamond_boots"];

fn raw<T: Packet>(packet: &T) -> RawPacket {
    let mut buf = BytesMut::new();
    encode_packet(packet, &mut buf);
    RawPacket::parse(buf.freeze()).expect("encoded packet parses")
}

fn item(state: &GameState, name: &str, count: u16, stack_id: i32) -> ItemV4 {
    let id = state.items.id(name).unwrap_or_else(|| panic!("{name} not in the script's item registry"));
    ItemV4 {
        network_id: id as i16,
        count,
        metadata: 0,
        has_stack_id: true,
        stack_id: Some(stack_id),
        block_runtime_id: 0,
        extra: ItemV4Extra::Default(None),
    }
}

/// An item whose NBT carries `display.Name` and `display.Lore`, as shop servers send.
fn named(base: ItemV4, name: &str, lore: &[&str]) -> ItemV4 {
    let lore = List { tag: 8, items: lore.iter().map(|l| Value::String((*l).into())).collect() };
    let display = Value::Compound(vec![("Name".into(), Value::String(name.into())), ("Lore".into(), Value::List(lore))]);
    let nbt = Nbt { name: String::new(), value: Value::Compound(vec![("display".into(), display)]) };
    let extra = ItemExtraDataWithoutBlockingTick {
        has_nbt: ItemExtraDataWithoutBlockingTickHasNbt::True,
        nbt: Some(ItemExtraDataWithoutBlockingTickNbt { version: 1, nbt }),
        can_place_on: vec![],
        can_destroy: vec![],
    };
    ItemV4 { extra: ItemV4Extra::Default(Some(extra)), ..base }
}

fn content(window: WindowIDVarint, input: Vec<ItemV4>) -> InventoryContent {
    let empty = ItemV4 { network_id: 0, count: 0, has_stack_id: false, stack_id: None, ..input[0].clone() };
    InventoryContent {
        window_id: window,
        input,
        container: FullContainerName { container_id: ContainerSlotType::AnvilInput, dynamic_container_id: None },
        storage_item: ItemV4 { extra: ItemV4Extra::Default(None), ..empty },
    }
}

fn players(template: PlayerRecord) -> PlayerList {
    let records = (1..=PLAYERS)
        .map(|n| {
            let mut r = template.clone();
            if let PlayerRecordContent::Add(a) = &mut r.content {
                a.uuid = Uuid([n; 16]);
                a.username = format!("Player{n:02}_{}", "x".repeat(usize::from(n % 6)));
                a.xbox_user_id = format!("25353{:011}", u64::from(n) * 7_919);
                a.entity_unique_id = -i64::from(n) * 4_294_967_311;
            }
            r
        })
        .collect();
    PlayerList { records }
}

/// What was applied: (script packets, script packets that failed to apply).
pub struct Built {
    pub state: GameState,
    pub script_packets: usize,
    pub script_errors: usize,
}

pub fn game_state() -> Built {
    let raws: Vec<RawPacket> = Script::bds_spawn()
        .steps
        .into_iter()
        .filter_map(|s| match s {
            Step::Send { packet, .. } => RawPacket::parse(packet).ok(),
            Step::Wait(_) => None,
        })
        .collect();
    // As the session does: item decoding branches on the server's shield id.
    for r in raws.iter().filter(|r| r.is::<ItemRegistry>()) {
        if let Ok(Some(shield)) = manual::shield_item_id_in_registry(&r.body) {
            manual::set_shield_item_id(shield);
        }
    }
    let mut state = GameState::default();
    let script_errors = raws.iter().filter(|r| state.apply(r).is_err()).count();
    let me = state.player.runtime_entity_id;

    let mut main: Vec<ItemV4> = (0..36).map(|i| item(&state, ITEMS[i % ITEMS.len()], 1 + (i as u16 * 7) % 64, 100 + i as i32)).collect();
    main[4] = named(main[4].clone(), "§6Shop Diamond Pick", &["§7Efficiency V", "§7Unbreaking III", "§8Bound"]);
    main[13] = named(main[13].clone(), "§bQuest Token", &["§7Bring to the guide"]);
    let armor = ARMOR.iter().enumerate().map(|(i, n)| item(&state, n, 1, 200 + i as i32)).collect();
    let offhand = vec![item(&state, "minecraft:shield", 1, 300)];
    let template = raws
        .iter()
        .filter(|r| r.is::<PlayerList>())
        .flat_map(|r| r.decode::<PlayerList>().expect("script PlayerList decodes").records)
        .find(|r| matches!(r.content, PlayerRecordContent::Add(_)))
        .expect("the spawn script lists the bot");
    let effects = [(1, 1), (10, 0), (16, 0)].map(|(effect_id, amplifier)| MobEffect {
        runtime_entity_id: me,
        event_id: MobEffectEventId::Add,
        effect_id,
        amplifier,
        particles: true,
        duration: 600,
        tick: 0,
        ambient: false,
    });
    let extra = [raw(&content(WindowIDVarint::Inventory, main)), raw(&content(WindowIDVarint::Armor, armor)),
        raw(&content(WindowIDVarint::Offhand, offhand)), raw(&players(template))];
    for r in extra.iter().chain(effects.iter().map(raw).collect::<Vec<_>>().iter()) {
        state.apply(r).expect("synthetic packet applies");
    }
    Built { state, script_packets: raws.len(), script_errors }
}
