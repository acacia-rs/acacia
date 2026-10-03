//! The inventory-action half of `InventoryTransaction`, which vanilla still fills when its own
//! prediction changes the held stack: placing an item, opening a new book, finishing or signing a
//! book (capture 2026-10-02; docs/research/survival-signs-beds.md).

use acacia_client::proto::packets::InventoryTransaction;
use acacia_client::proto::types::{
    ItemV4, Transaction, TransactionActionsItem, TransactionActionsItemSourceType, TransactionLegacy,
    TransactionLegacyLegacyTransactionsItem, TransactionLegacyLegacyTransactionsItemChangedSlotsItem, TransactionTransactionData,
    TransactionTransactionType,
};

use super::to_wire;
use crate::state::ItemStack;

/// `ContainerSlotType::Inventory`, which vanilla names for hotbar slots in the legacy list too.
const INVENTORY_CONTAINER: u8 = 29;
/// Creative-source slots: 0 takes the old stack away, 1 hands out the new one.
const CREATIVE_DELETE: u32 = 0;
const CREATIVE_CREATE: u32 = 1;

/// Hotbar `slot` going from `old` to `new`, as the transaction's actions name it.
pub(crate) struct SlotChange<'a> {
    pub slot: u8,
    pub old: &'a ItemStack,
    pub new: &'a ItemStack,
}

fn item(stack: &ItemStack) -> ItemV4 {
    ItemV4 { has_stack_id: false, stack_id: None, ..to_wire(stack) }
}

fn action(source_type: TransactionActionsItemSourceType, window_id: Option<i8>, slot: u32, old: &ItemStack, new: &ItemStack) -> TransactionActionsItem {
    TransactionActionsItem { source_type, window_id, flags: None, slot, old_item: item(old), new_item: item(new) }
}

fn container_action(change: &SlotChange) -> TransactionActionsItem {
    action(TransactionActionsItemSourceType::Container, Some(0), u32::from(change.slot), change.old, change.new)
}

/// `legacy_request_id` with the changed slot listed, or 0 and no list.
fn legacy(id: Option<i32>, slot: u8) -> TransactionLegacy {
    let listed = |_| {
        vec![TransactionLegacyLegacyTransactionsItem {
            container_id: INVENTORY_CONTAINER,
            changed_slots: vec![TransactionLegacyLegacyTransactionsItemChangedSlotsItem { slot_id: slot }],
        }]
    };
    TransactionLegacy { legacy_request_id: id.unwrap_or(0), legacy_transactions: id.map(listed) }
}

/// Adds the held slot's change to an item-use transaction.
pub(crate) fn with_slot_change(mut packet: InventoryTransaction, legacy_id: Option<i32>, change: &SlotChange) -> InventoryTransaction {
    packet.transaction.legacy = legacy(legacy_id, change.slot);
    packet.transaction.actions = vec![container_action(change)];
    packet
}

/// A `Normal` transaction rewriting a hotbar stack (book text, signing): the slot change plus the
/// creative delete/create pair vanilla adds.
pub(crate) fn rewrite_slot(legacy_id: Option<i32>, change: &SlotChange) -> InventoryTransaction {
    let air = ItemStack::default();
    let actions = vec![
        container_action(change),
        action(TransactionActionsItemSourceType::Creative, None, CREATIVE_DELETE, &air, change.old),
        action(TransactionActionsItemSourceType::Creative, None, CREATIVE_CREATE, change.new, &air),
    ];
    InventoryTransaction {
        transaction: Transaction {
            legacy: legacy(legacy_id, change.slot),
            transaction_type: TransactionTransactionType::Normal,
            actions,
            transaction_data: TransactionTransactionData::Normal,
        },
    }
}
