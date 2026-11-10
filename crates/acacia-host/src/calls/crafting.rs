use acacia_api::{CraftParams, CraftResult, RecipesForResult, SmeltParams, SmeltResult};
use acacia_bot::{ActionError, Bot};

use crate::convert;

pub(super) async fn craft(bot: &mut Bot, p: CraftParams) -> Result<CraftResult, ActionError> {
    let crafted = bot.craft(convert::item(&p.item)?, p.count.unwrap_or(1), p.table.map(Into::into)).await?;
    Ok(CraftResult { crafted })
}

pub(super) async fn smelt(bot: &mut Bot, p: SmeltParams) -> Result<SmeltResult, ActionError> {
    let (item, fuel) = (convert::item_name(&p.item), p.fuel.as_deref().map(convert::item_name));
    let fuel = fuel.as_deref().map(|f| (f, p.fuel_count.unwrap_or(1)));
    let smelted = bot.smelt([p.x, p.y, p.z], &item, p.count.unwrap_or(1), fuel).await?;
    Ok(SmeltResult { smelted: u32::from(smelted) })
}

pub(super) fn recipes_for(bot: &Bot, item: &str) -> RecipesForResult {
    let items = &bot.state().items;
    let found = convert::item(item).map(|i| bot.recipes_for(i)).unwrap_or_default();
    RecipesForResult { recipes: found.into_iter().map(|r| convert::recipe(items, r)).collect() }
}
