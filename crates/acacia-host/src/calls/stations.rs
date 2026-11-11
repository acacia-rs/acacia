use acacia_api::{
    AnvilParams, BeaconEffect, BeaconParams, BrewParams, CartographyParams, CartographyResult, EnchantParams, GrindstoneParams,
    LoomParams, SlotRef, SmithParams, StonecutParams, StonecutResult, TradeParams, TradeResult,
};
use acacia_bot::workstation::BeaconEffect as Effect;
use acacia_bot::{items, ActionError, Bot};

use super::items::slot;
use crate::convert;

fn optional(bot: &Bot, s: Option<&SlotRef>) -> Result<Option<items::SlotRef>, ActionError> {
    s.map(|s| slot(bot, s)).transpose()
}

pub(super) async fn anvil(bot: &mut Bot, p: AnvilParams) -> Result<(), ActionError> {
    let (input, material) = (slot(bot, &p.input)?, optional(bot, p.material.as_ref())?);
    bot.anvil([p.x, p.y, p.z], input, material, p.rename.as_deref()).await
}

pub(super) async fn enchant(bot: &mut Bot, p: EnchantParams) -> Result<(), ActionError> {
    let item = slot(bot, &p.item)?;
    bot.enchant([p.x, p.y, p.z], item, usize::from(p.option)).await
}

pub(super) async fn grindstone(bot: &mut Bot, p: GrindstoneParams) -> Result<(), ActionError> {
    let (input, additional) = (slot(bot, &p.input)?, optional(bot, p.additional.as_ref())?);
    bot.grindstone([p.x, p.y, p.z], input, additional).await
}

pub(super) async fn smith(bot: &mut Bot, p: SmithParams) -> Result<(), ActionError> {
    let (base, addition, template) = (slot(bot, &p.base)?, slot(bot, &p.addition)?, slot(bot, &p.template)?);
    bot.smith([p.x, p.y, p.z], base, addition, template).await
}

pub(super) async fn stonecut(bot: &mut Bot, p: StonecutParams) -> Result<StonecutResult, ActionError> {
    let made = bot.stonecut([p.x, p.y, p.z], convert::item(&p.item)?, p.count.unwrap_or(1)).await?;
    Ok(StonecutResult { made })
}

pub(super) async fn loom(bot: &mut Bot, p: LoomParams) -> Result<(), ActionError> {
    let (banner, dye, pattern_item) = (slot(bot, &p.banner)?, slot(bot, &p.dye)?, optional(bot, p.pattern_item.as_ref())?);
    bot.loom([p.x, p.y, p.z], banner, dye, &p.pattern, pattern_item).await
}

pub(super) async fn cartography(bot: &mut Bot, p: CartographyParams) -> Result<CartographyResult, ActionError> {
    let (map, additional) = (slot(bot, &p.map)?, optional(bot, p.additional.as_ref())?);
    let crafted = bot.cartography([p.x, p.y, p.z], map, additional, p.rename.as_deref(), p.times.unwrap_or(1)).await?;
    Ok(CartographyResult { crafted })
}

pub(super) async fn beacon(bot: &mut Bot, p: BeaconParams) -> Result<(), ActionError> {
    let payment = slot(bot, &p.payment)?;
    bot.beacon([p.x, p.y, p.z], effect(p.primary), p.secondary.map(effect), payment).await
}

fn effect(e: BeaconEffect) -> Effect {
    match e {
        BeaconEffect::Speed => Effect::Speed,
        BeaconEffect::Haste => Effect::Haste,
        BeaconEffect::Strength => Effect::Strength,
        BeaconEffect::JumpBoost => Effect::JumpBoost,
        BeaconEffect::Regeneration => Effect::Regeneration,
        BeaconEffect::Resistance => Effect::Resistance,
    }
}

pub(super) async fn brew(bot: &mut Bot, p: BrewParams) -> Result<(), ActionError> {
    let mut bottles = Vec::with_capacity(p.bottles.len());
    for s in &p.bottles {
        let picked = match s {
            SlotRef::Item(name) => next_stack(bot, &convert::item_name(name), &bottles)?,
            s => slot(bot, s)?,
        };
        bottles.push(picked);
    }
    bot.brew([p.x, p.y, p.z], &bottles, &convert::item_name(&p.ingredient)).await
}

/// The first main-inventory stack of `name` not already `taken`: bottles do not stack, so each name is its own one.
fn next_stack(bot: &Bot, name: &str, taken: &[items::SlotRef]) -> Result<items::SlotRef, ActionError> {
    let state = bot.state();
    let id = state.items.id(name);
    let free = |i: usize| !state.inventory.main[i].is_empty() && Some(state.inventory.main[i].network_id) == id;
    (0..state.inventory.main.len())
        .map(|i| (i, items::SlotRef::Main(i as u8)))
        .find(|(i, s)| free(*i) && !taken.contains(s))
        .map(|(_, s)| s)
        .ok_or_else(|| ActionError::NotPossible(format!("not enough {name} in the inventory")))
}

pub(super) async fn trade(bot: &mut Bot, p: TradeParams) -> Result<TradeResult, ActionError> {
    let traded = bot.trade(p.entity.0, p.offer as usize, p.times.unwrap_or(1)).await?;
    Ok(TradeResult { traded })
}
