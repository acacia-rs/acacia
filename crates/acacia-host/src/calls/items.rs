use acacia_api::{ConsumeParams, EquipDestination, EquipParams, TossParams, TossResult};
use acacia_bot::items::SlotRef;
use acacia_bot::survival::Destination;
use acacia_bot::{ActionError, Bot};

use crate::convert;

pub(super) async fn equip(bot: &mut Bot, p: EquipParams) -> Result<(), ActionError> {
    hold(bot, &p.item, convert::destination(p.destination.unwrap_or(EquipDestination::Hand))).await
}

async fn hold(bot: &mut Bot, item: &str, to: Destination) -> Result<(), ActionError> {
    let name = convert::item_name(item);
    let slot = bot.find_item(&name).ok_or_else(|| ActionError::NotPossible(format!("no {name} in the inventory")))?;
    bot.equip(slot, to).await
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
