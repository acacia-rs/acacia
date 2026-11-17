use acacia_api::{self as api, ConsumeParams, EquipDestination, EquipParams, InventorySlot, MoveItemParams, SwapItemsParams, TossParams, TossResult};
use acacia_bot::items::SlotRef;
use acacia_bot::state::Inventory;
use acacia_bot::survival::Destination;
use acacia_bot::{ActionError, Bot};

use crate::convert;

pub(super) async fn equip(bot: &mut Bot, p: EquipParams) -> Result<(), ActionError> {
    hold(bot, &p.item, convert::destination(p.destination.unwrap_or(EquipDestination::Hand))).await
}

async fn hold(bot: &mut Bot, item: &str, to: Destination) -> Result<(), ActionError> {
    let name = convert::item_name(item);
    let slot = bot.find_item_named(&name).ok_or_else(|| ActionError::NotPossible(format!("no {name} in the inventory")))?;
    bot.equip(slot, to).await
}

/// The bot's own slot `slot` names: a place (window 0, 119 or 120) or the first main-inventory stack of an item.
pub(super) fn slot(bot: &Bot, slot: &api::SlotRef) -> Result<SlotRef, ActionError> {
    match slot {
        api::SlotRef::Item(item) => {
            let name = convert::item_name(item);
            bot.find_item_named(&name).ok_or_else(|| ActionError::NotPossible(format!("no {name} in the inventory")))
        }
        api::SlotRef::Slot(InventorySlot { window, slot }) => match (*window, *slot) {
            (Inventory::WINDOW_INVENTORY, s @ 0..=35) => Ok(SlotRef::Main(s)),
            (Inventory::WINDOW_OFFHAND, 0) => Ok(SlotRef::Offhand),
            (Inventory::WINDOW_ARMOR, s @ 0..=3) => Ok(SlotRef::Armor(s)),
            (w, s) => Err(ActionError::NotPossible(format!("window {w} slot {s} is not one of the bot's own slots"))),
        },
    }
}

pub(super) async fn move_item(bot: &mut Bot, p: MoveItemParams) -> Result<(), ActionError> {
    let (from, to) = (slot(bot, &p.from)?, slot(bot, &p.to)?);
    let count = match p.count {
        Some(n) => n,
        None => from.stack(bot.state()).filter(|s| !s.is_empty()).map_or(0, |s| s.count.min(u16::from(u8::MAX)) as u8),
    };
    if count == 0 {
        return Err(ActionError::NotPossible(format!("{from:?} is empty")));
    }
    bot.move_item(from, to, count).await
}

pub(super) async fn swap_items(bot: &mut Bot, p: SwapItemsParams) -> Result<(), ActionError> {
    let (a, b) = (slot(bot, &p.a)?, slot(bot, &p.b)?);
    bot.swap_items(a, b).await
}

pub(super) async fn consume(bot: &mut Bot, p: ConsumeParams) -> Result<(), ActionError> {
    if let Some(item) = &p.item {
        hold(bot, item, Destination::Hand).await?;
    }
    bot.consume().await
}

pub(super) async fn toss(bot: &mut Bot, p: TossParams) -> Result<TossResult, ActionError> {
    let name = convert::item_name(&p.item);
    let state = bot.state();
    let id = state.items.id(&name);
    let stacks: Vec<(u8, u16)> = (state.inventory.main.iter().enumerate())
        .filter(|(_, s)| !s.is_empty() && Some(s.network_id) == id)
        .map(|(slot, s)| (slot as u8, s.count))
        .collect();
    if stacks.is_empty() {
        return Err(ActionError::NotPossible(format!("no {name} in the inventory")));
    }
    if let Some(v) = p.toward {
        bot.look_at([v.x, v.y, v.z]).await?;
    }
    let (want, mut tossed) = (p.count.unwrap_or(u32::MAX), 0);
    for (slot, count) in stacks {
        if tossed >= want {
            break;
        }
        let n = u32::from(count).min(want - tossed).min(u32::from(u8::MAX)) as u8;
        bot.drop_item(SlotRef::Main(slot), n).await?;
        tossed += u32::from(n);
    }
    Ok(TossResult { tossed })
}
