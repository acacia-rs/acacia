use acacia_api::{CraftParams, CraftResult, RecipesForResult};
use acacia_bot::{ActionError, Bot};

use crate::convert;

pub(super) async fn craft(bot: &mut Bot, p: CraftParams) -> Result<CraftResult, ActionError> {
    let crafted = bot.craft(&convert::item_name(&p.item), p.count.unwrap_or(1), p.table.map(Into::into)).await?;
    Ok(CraftResult { crafted })
}

pub(super) fn recipes_for(bot: &Bot, item: &str) -> RecipesForResult {
    let items = &bot.state().items;
    RecipesForResult { recipes: bot.recipes_for(&convert::item_name(item)).into_iter().map(|r| convert::recipe(items, r)).collect() }
}
