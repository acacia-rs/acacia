use acacia_api::{EquipDestination, EquipParams};
use acacia_bot::{ActionError, Bot};

use crate::convert;

pub(super) async fn equip(bot: &mut Bot, p: EquipParams) -> Result<(), ActionError> {
    let name = convert::item_name(&p.item);
    let slot = bot.find_item(&name).ok_or_else(|| ActionError::NotPossible(format!("no {name} in the inventory")))?;
    bot.equip(slot, convert::destination(p.destination.unwrap_or(EquipDestination::Hand))).await
}
