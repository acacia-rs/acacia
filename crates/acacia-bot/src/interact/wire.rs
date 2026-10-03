//! Packet construction for interactions; field choices are documented in `interact/mod.rs`.

use acacia_client::proto::manual::shield_item_id;
use acacia_client::proto::packets::{
    Animate, AnimateActionId, Interact, InteractActionId, InventoryTransaction, MobEquipment, PlayerAction, PlayerAuthInputBlockActionItem,
};
use acacia_client::proto::types::{
    Action, BlockCoordinates, ItemExtraDataWithBlockingTick, ItemExtraDataWithBlockingTickHasNbt,
    ItemExtraDataWithBlockingTickNbt, ItemExtraDataWithoutBlockingTick, ItemExtraDataWithoutBlockingTickHasNbt,
    ItemExtraDataWithoutBlockingTickNbt, ItemV4, ItemV4Extra, Transaction, TransactionLegacy, TransactionTransactionData,
    TransactionTransactionDataItemRelease, TransactionTransactionDataItemReleaseActionType,
    TransactionTransactionDataItemUseOnEntity, TransactionTransactionDataItemUseOnEntityActionType,
    TransactionTransactionType, TransactionUseItem, TransactionUseItemActionType, TransactionUseItemClientCooldownState,
    TransactionUseItemClientPrediction, TransactionUseItemHand, TransactionUseItemTriggerType, Vec3f, Vec3i, WindowID,
};
use acacia_physics::{BlockPos, Vec3};

use super::Face;
use crate::state::ItemStack;

/// `face` of a click that hit no block.
const NO_FACE: u8 = 255;

/// A tracked stack as the item-instance form used in `held_item` / `MobEquipment`. `can_place_on` /
/// `can_destroy` are not tracked and go out empty; Geyser and Boar compare only the item type.
pub fn to_wire(item: &ItemStack) -> ItemV4 {
    if item.is_empty() {
        return ItemV4 {
            network_id: 0,
            count: 0,
            metadata: 0,
            has_stack_id: false,
            stack_id: None,
            block_runtime_id: 0,
            extra: ItemV4Extra::Default(None),
        };
    }
    let stack_id = item.stack_network_id.filter(|&id| id != 0);
    // A present item always carries its user-data blob, even when it has no NBT (gophertunnel `itemUserData`).
    let extra = if item.network_id == shield_item_id() {
        ItemV4Extra::ShieldItemID(Some(ItemExtraDataWithBlockingTick {
            has_nbt: if item.nbt.is_some() { ItemExtraDataWithBlockingTickHasNbt::True } else { ItemExtraDataWithBlockingTickHasNbt::False },
            nbt: item.nbt.clone().map(|nbt| ItemExtraDataWithBlockingTickNbt { version: 1, nbt }),
            can_place_on: Vec::new(),
            can_destroy: Vec::new(),
            blocking_tick: 0,
        }))
    } else {
        ItemV4Extra::Default(Some(ItemExtraDataWithoutBlockingTick {
            has_nbt: if item.nbt.is_some() { ItemExtraDataWithoutBlockingTickHasNbt::True } else { ItemExtraDataWithoutBlockingTickHasNbt::False },
            nbt: item.nbt.clone().map(|nbt| ItemExtraDataWithoutBlockingTickNbt { version: 1, nbt }),
            can_place_on: Vec::new(),
            can_destroy: Vec::new(),
        }))
    };
    ItemV4 {
        network_id: item.network_id as i16,
        count: item.count,
        metadata: item.metadata,
        has_stack_id: stack_id.is_some(),
        stack_id,
        block_runtime_id: item.block_runtime_id,
        extra,
    }
}

pub(crate) fn vec3f([x, y, z]: Vec3) -> Vec3f {
    Vec3f { x, y, z }
}

pub(crate) fn block_coordinates([x, y, z]: BlockPos) -> BlockCoordinates {
    BlockCoordinates { x, y, z }
}

/// Swing sources as vanilla names them (gophertunnel `swingSourceToString`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwingSource {
    Build,
    Mine,
    Interact,
    Attack,
    UseItem,
}

impl SwingSource {
    fn name(self) -> &'static str {
        match self {
            SwingSource::Build => "build",
            SwingSource::Mine => "mine",
            SwingSource::Interact => "interact",
            SwingSource::Attack => "attack",
            SwingSource::UseItem => "useitem",
        }
    }
}

pub(crate) fn swing(runtime_id: u64, source: Option<SwingSource>) -> Animate {
    Animate {
        action_id: AnimateActionId::SwingArm,
        runtime_entity_id: runtime_id,
        data: 0.0,
        has_swing_source: source.is_some(),
        swing_source: source.map(|s| s.name().to_owned()),
    }
}

/// Hotbar selection; `slot` is both the inventory slot and the hotbar slot.
/// The held item goes out without its stack id: Geyser reads a present id as a tagged variant and
/// rejects the untagged form (killing the session), BDS rejects the tagged form; both accept none.
pub(crate) fn mob_equipment(runtime_id: u64, slot: u8, item: &ItemStack) -> MobEquipment {
    let item = ItemV4 { has_stack_id: false, stack_id: None, ..to_wire(item) };
    MobEquipment { runtime_entity_id: runtime_id, item, slot, selected_slot: slot, window_id: WindowID::Inventory }
}

fn transaction(transaction_type: TransactionTransactionType, transaction_data: TransactionTransactionData) -> InventoryTransaction {
    InventoryTransaction {
        transaction: Transaction {
            legacy: TransactionLegacy { legacy_request_id: 0, legacy_transactions: None },
            transaction_type,
            actions: Vec::new(),
            transaction_data,
        },
    }
}

/// What a use-item transaction needs to know about the player.
pub(crate) struct Hand {
    pub slot: u8,
    pub item: ItemV4,
    pub eye: Vec3,
}

/// `UseItem` ClickBlock: `click` is relative to the block, `block` the clicked block's wire id.
pub(crate) fn click_block(hand: Hand, pos: BlockPos, face: Face, click: Vec3, block: u32) -> InventoryTransaction {
    use_item(hand, TransactionUseItemActionType::ClickBlock, block_coordinates(pos), face as u8, click, block, TransactionUseItemTriggerType::PlayerInput)
}

/// `UseItem` ClickAir. Boar rejects block positions over 12 blocks away unless they sum to 0.
/// Every pressed ClickAir in the 2026-10-02 capture (rod, firework, book, empty map, after a sign,
/// a missed entity) had trigger 0 and prediction Failure.
pub(crate) fn click_air(hand: Hand) -> InventoryTransaction {
    air(hand, TransactionUseItemTriggerType::UnknownValue, TransactionUseItemClientPrediction::Failure)
}

/// The ClickAir the client sends when a held use runs out (food eaten, potion drunk). Its trigger
/// is the simulation tick, not the player's press (gophertunnel `TriggerTypeSimulationTick`).
pub(crate) fn finish_use(hand: Hand) -> InventoryTransaction {
    air(hand, TransactionUseItemTriggerType::SimulationTick, TransactionUseItemClientPrediction::Success)
}

fn air(hand: Hand, trigger: TransactionUseItemTriggerType, prediction: TransactionUseItemClientPrediction) -> InventoryTransaction {
    let mut packet = use_item(hand, TransactionUseItemActionType::ClickAir, block_coordinates([0, 0, 0]), NO_FACE, [0.0; 3], 0, trigger);
    if let TransactionTransactionData::ItemUse(u) = &mut packet.transaction.transaction_data {
        u.client_prediction = prediction;
    }
    packet
}

/// `PlayerAction` StartItemUseOn / StopItemUseOn around a block click.
pub(crate) fn item_use_on(runtime_id: u64, action: Action, pos: BlockPos, result: BlockPos, face: i32) -> PlayerAction {
    PlayerAction { runtime_entity_id: runtime_id, action, position: block_coordinates(pos), result_position: block_coordinates(result), face }
}

/// `Interact` MouseOverEntity with no target: the cursor over nothing.
pub(crate) fn mouse_over_nothing() -> Interact {
    Interact { action_id: InteractActionId::MouseOverEntity, target_entity_id: 0, has_position: false, position: None }
}

/// `Interact` MouseOverEntity: the crosshair on entity `runtime_id` at world point `hit`.
pub(crate) fn mouse_over(runtime_id: u64, hit: Vec3) -> Interact {
    Interact { action_id: InteractActionId::MouseOverEntity, target_entity_id: runtime_id, has_position: true, position: Some(vec3f(hit)) }
}

fn use_item(
    hand: Hand,
    action_type: TransactionUseItemActionType,
    block_position: BlockCoordinates,
    face: u8,
    click: Vec3,
    block_runtime_id: u32,
    trigger_type: TransactionUseItemTriggerType,
) -> InventoryTransaction {
    transaction(
        TransactionTransactionType::ItemUse,
        TransactionTransactionData::ItemUse(TransactionUseItem {
            action_type,
            trigger_type,
            block_position,
            face,
            hotbar_slot: i32::from(hand.slot),
            hand: TransactionUseItemHand::MainHand,
            held_item: hand.item,
            player_pos: vec3f(hand.eye),
            click_pos: vec3f(click),
            block_runtime_id,
            client_prediction: TransactionUseItemClientPrediction::Success,
            client_cooldown_state: TransactionUseItemClientCooldownState::Off,
        }),
    )
}

/// `UseItemOnEntity`; `click` is the world position hit (Geyser subtracts the entity position).
pub(crate) fn use_on_entity(hand: Hand, runtime_id: u64, attack: bool, click: Vec3) -> InventoryTransaction {
    let action_type = if attack {
        TransactionTransactionDataItemUseOnEntityActionType::Attack
    } else {
        TransactionTransactionDataItemUseOnEntityActionType::Interact
    };
    transaction(
        TransactionTransactionType::ItemUseOnEntity,
        TransactionTransactionData::ItemUseOnEntity(TransactionTransactionDataItemUseOnEntity {
            entity_runtime_id: runtime_id,
            action_type,
            hotbar_slot: i32::from(hand.slot),
            held_item: hand.item,
            player_pos: vec3f(hand.eye),
            click_pos: vec3f(click),
        }),
    )
}

/// `ReleaseItem` Release: shoots a drawn bow / crossbow charge, stops using a shield or spyglass.
pub(crate) fn release(hand: Hand) -> InventoryTransaction {
    transaction(
        TransactionTransactionType::ItemRelease,
        TransactionTransactionData::ItemRelease(TransactionTransactionDataItemRelease {
            action_type: TransactionTransactionDataItemReleaseActionType::Release,
            hotbar_slot: i32::from(hand.slot),
            held_item: hand.item,
            head_pos: vec3f(hand.eye),
        }),
    )
}

pub(crate) fn block_action(action: Action, [x, y, z]: BlockPos, face: Face) -> PlayerAuthInputBlockActionItem {
    PlayerAuthInputBlockActionItem { action, position: Vec3i { x, y, z }, face: i32::from(face as u8) }
}
