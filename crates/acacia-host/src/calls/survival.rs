use acacia_api::{CraftableItemsResult, DigParams, EquipBestToolResult, ItemCount, SetAutoEatParams, WriteSignParams};
use acacia_bot::items::SlotRef;
use acacia_bot::signs::SignSide;
use acacia_bot::{ActionError, Bot};

use crate::convert;

pub(super) async fn dig(bot: &mut Bot, p: DigParams) -> Result<(), ActionError> {
    let pos = [p.x, p.y, p.z];
    if p.equip_best_tool == Some(true) {
        bot.equip_best_tool(pos).await?;
    }
    bot.break_block(pos).await
}

pub(super) async fn equip_best_tool(bot: &mut Bot, pos: [i32; 3]) -> Result<EquipBestToolResult, ActionError> {
    let slot = match bot.equip_best_tool(pos).await? {
        Some(SlotRef::Main(slot)) => Some(slot),
        _ => None,
    };
    Ok(EquipBestToolResult { slot })
}

pub(super) fn set_auto_eat(bot: &mut Bot, p: &SetAutoEatParams) {
    bot.set_auto_eat(p.enabled.then(|| convert::auto_eat(&p.policy)));
}

pub(super) async fn write_sign(bot: &mut Bot, p: WriteSignParams) -> Result<(), ActionError> {
    let side = if p.back == Some(true) { SignSide::Back } else { SignSide::Front };
    bot.write_sign([p.x, p.y, p.z], side, &p.lines.join("\n")).await
}

pub(super) fn craftable_items(bot: &Bot, table: bool) -> CraftableItemsResult {
    let items = &bot.state().items;
    let made = bot.craftable(table).into_iter().map(|s| ItemCount { item: convert::stack_name(items, &s), count: u32::from(s.count) });
    CraftableItemsResult { items: made.collect() }
}
