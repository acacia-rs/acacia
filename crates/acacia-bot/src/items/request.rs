use acacia_client::proto::packets::{ItemStackRequest as ItemStackRequestPacket, ItemStackResponse};
use acacia_client::proto::types::{
    ItemStackRequest, ItemStackRequestActionsItem as Action, ItemStackRequestActionsItemContent as Content,
    ItemStackRequestActionsItemContentBeaconPayment, ItemStackRequestActionsItemContentConsume, ItemStackRequestActionsItemContentCreate,
    ItemStackRequestActionsItemContentDestroy, ItemStackRequestActionsItemContentDrop,
    ItemStackRequestActionsItemContentPlace, ItemStackRequestActionsItemContentSwap,
    ItemStackRequestActionsItemContentTake, ItemStackRequestActionsItemTypeId as TypeId, ItemStackRequestCause,
    ItemStackResponsesItem, ItemStackResponsesItemStatus, StackRequestSlotInfo,
};
use acacia_client::proto::{Packet, RawPacket};

use super::{ui, SlotRef};
use crate::ActionError;

/// One step of an item stack request, in terms of the bot's slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// Move `count` items onto an empty or matching stack (sent as `Take` into the cursor, `Place` otherwise).
    Transfer { from: SlotRef, to: SlotRef, count: u8 },
    Swap { a: SlotRef, b: SlotRef },
    /// Throw `count` items into the world.
    Drop { from: SlotRef, count: u8 },
    /// Delete `count` items (creative mode only).
    Destroy { from: SlotRef, count: u8 },
    /// Use up `count` crafting inputs; only valid after a craft action in the same request.
    Consume { from: SlotRef, count: u8 },
    /// Put result `index` of a multi-output craft into the created-output slot.
    Create { index: u8 },
}

/// Request ids of one session: vanilla's first request is -3, then -5, -7, ... (2026-10-02 capture);
/// even negative ids belong to legacy InventoryTransactions.
#[derive(Debug)]
pub(crate) struct RequestIds {
    pub(super) next: i32,
}

impl Default for RequestIds {
    fn default() -> Self {
        RequestIds { next: -3 }
    }
}

impl RequestIds {
    pub fn next(&mut self) -> i32 {
        let id = self.next;
        // Wraps after ~1e9 requests; restart the sequence.
        self.next = id.checked_sub(2).unwrap_or(-3);
        id
    }
}

/// The wire action for `op`; `info` gives each slot's request view as it is before the op runs,
/// told whether the op is an automatic move (see [`auto_move`]).
pub(crate) fn encode(op: Op, info: impl Fn(SlotRef, bool) -> StackRequestSlotInfo) -> Action {
    let auto = auto_move(op);
    let info = |slot| info(slot, auto);
    let (type_id, content) = match op {
        Op::Transfer { from, to: SlotRef::Cursor, count } => {
            let (source, destination) = (info(from), info(SlotRef::Cursor));
            (TypeId::Take, Content::Take(ItemStackRequestActionsItemContentTake { count, source, destination }))
        }
        Op::Transfer { from, to, count } => {
            let (source, destination) = (info(from), info(to));
            (TypeId::Place, Content::Place(ItemStackRequestActionsItemContentPlace { count, source, destination }))
        }
        Op::Swap { a, b } => {
            (TypeId::Swap, Content::Swap(ItemStackRequestActionsItemContentSwap { source: info(a), destination: info(b) }))
        }
        Op::Drop { from, count } => {
            (TypeId::Drop, Content::Drop(ItemStackRequestActionsItemContentDrop { count, source: info(from), randomly: false }))
        }
        Op::Destroy { from, count } => {
            (TypeId::Destroy, Content::Destroy(ItemStackRequestActionsItemContentDestroy { count, source: info(from) }))
        }
        Op::Consume { from, count } => {
            (TypeId::Consume, Content::Consume(ItemStackRequestActionsItemContentConsume { count, source: info(from) }))
        }
        Op::Create { index } => (TypeId::Create, Content::Create(ItemStackRequestActionsItemContentCreate { result_slot_id: index })),
    };
    action(type_id, content)
}

/// Moves the client makes for the player rather than slot to slot: consuming craft inputs, moving
/// out of a container or workstation, and filling a crafting grid or trade payment slot. Vanilla
/// names the player's slots in these `HotbarAndInventory`, otherwise `Hotbar`/`Inventory`
/// (2026-10-02 capture; the own inventory screen per BDS, see `SlotRef::wire`).
fn auto_move(op: Op) -> bool {
    match op {
        Op::Consume { .. } => true,
        Op::Transfer { from, to, .. } => {
            !from.is_player() || matches!(to, SlotRef::Ui(ui::TRADE_INGREDIENT_1 | ui::TRADE_INGREDIENT_2 | ui::CRAFTING_2X2..=ui::CRAFTING_3X3_LAST))
        }
        _ => false,
    }
}

/// `BeaconPayment` with Bedrock effect ids (0 = none).
pub(crate) fn beacon_payment(primary_effect: i32, secondary_effect: i32) -> Action {
    action(TypeId::BeaconPayment, Content::BeaconPayment(ItemStackRequestActionsItemContentBeaconPayment { primary_effect, secondary_effect }))
}

pub(crate) fn action(type_id: TypeId, content: Content) -> Action {
    Action { type_id, legacy_type_id: legacy_type_id(type_id), content }
}

/// The legacy id still counts the removed `PlaceInContainer`/`TakeOutContainer` (7, 8), so every
/// later action's legacy id is its variant + 2 (gophertunnel `item_stack.go`).
fn legacy_type_id(type_id: TypeId) -> u8 {
    let variant = type_id.to_raw();
    (if variant >= 7 { variant + 2 } else { variant }) as u8
}

/// The request body besides its actions; only anvil and cartography renames carry text.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Texts {
    pub custom_names: Vec<String>,
    pub cause: ItemStackRequestCause,
}

impl Default for Texts {
    fn default() -> Self {
        // Vanilla sends -1 ("none") with every request that carries no text.
        Texts { custom_names: Vec::new(), cause: ItemStackRequestCause::Unknown(-1) }
    }
}

pub(crate) fn request_packet(request_id: i32, actions: Vec<Action>, texts: Texts) -> ItemStackRequestPacket {
    ItemStackRequestPacket {
        requests: vec![ItemStackRequest { request_id, actions, custom_names: texts.custom_names, cause: texts.cause }],
    }
}

/// The response to request `request_id`, if `packet` carries it.
pub(crate) fn response_for(packet: &RawPacket, request_id: i32) -> Option<ItemStackResponsesItem> {
    if packet.id != ItemStackResponse::ID {
        return None;
    }
    let response: ItemStackResponse = packet.decode().ok()?;
    response.responses.into_iter().find(|r| r.request_id == request_id)
}

pub(crate) fn check_status(response: &ItemStackResponsesItem) -> Result<(), ActionError> {
    match response.status {
        ItemStackResponsesItemStatus::Ok => Ok(()),
        status => Err(ActionError::Rejected(format!("item stack request {} failed with status {}", response.request_id, status.to_raw()))),
    }
}
