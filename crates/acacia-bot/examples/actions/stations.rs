//! Crafting, workstation and trading checks.
use acacia_bot::Bot;
use acacia_bot::items::SlotRef;
use acacia_bot::state::TradeItem;

use crate::{count, find, has_nbt, server_entities, server_inv, stack, stacks, Check, Scene};

pub async fn craft(bot: &mut Bot, s: &Scene) -> Check {
    let planks = bot.craft("minecraft:oak_planks", 8, None).await?;
    let sticks = bot.craft("minecraft:stick", 4, None).await?;
    let shovel = bot.craft("minecraft:wooden_shovel", 1, Some(s.at("crafting_table"))).await?;
    let have = (count(bot, "minecraft:oak_planks"), count(bot, "minecraft:stick"), count(bot, "minecraft:wooden_shovel"));
    if have.2 == 0 {
        return Err(format!("made {planks} planks, {sticks} sticks, {shovel} shovels; have {have:?}").into());
    }
    Ok(format!("made {planks} planks, {sticks} sticks, {shovel} shovel; have {have:?}"))
}

pub async fn smelt(bot: &mut Bot, s: &Scene) -> Check {
    let before = count(bot, "minecraft:iron_ingot");
    let taken = bot.smelt(s.at("furnace"), "minecraft:raw_iron", 1, Some(("minecraft:coal", 1))).await?;
    let after = count(bot, "minecraft:iron_ingot");
    if after != before + u32::from(taken) {
        return Err(format!("took {taken}, iron ingots {before} -> {after}").into());
    }
    Ok(format!("took {taken} iron ingot"))
}

pub async fn brew(bot: &mut Bot, s: &Scene) -> Check {
    let bottles: Vec<SlotRef> = (0..36u8)
        .map(SlotRef::Main)
        .filter(|&slot| stack(bot, slot).is_some_and(|st| !st.is_empty() && bot.state().item_name(st) == Some("minecraft:potion")))
        .take(3)
        .collect();
    bot.brew(s.at("brewing_stand"), &bottles, "minecraft:nether_wart").await?;
    let metadata: Vec<u32> = stacks(bot, "minecraft:potion").iter().map(|p| p.metadata).collect();
    // Awkward potion = metadata 4.
    if metadata.len() != bottles.len() || metadata.iter().any(|&m| m != 4) {
        return Err(format!("potion metadata after brewing: {metadata:?}").into());
    }
    Ok(format!("{} awkward potions", metadata.len()))
}

pub async fn enchant(bot: &mut Bot, s: &Scene) -> Check {
    let level = bot.state().player.xp_level;
    bot.enchant(s.at("enchanting_table"), find(bot, "minecraft:iron_sword")?, 0).await?;
    let ours = stacks(bot, "minecraft:iron_sword").first().map(|s| has_nbt(s, "ench"));
    let server = server_inv(bot, "minecraft:iron_sword").await?;
    if server.contains("\"ench\":[]") || ours != Some(true) {
        return Err(format!("server {server}; bot sees enchantments: {ours:?}").into());
    }
    Ok(format!("level {level} -> {}; server {server}", bot.state().player.xp_level))
}

pub async fn anvil(bot: &mut Bot, s: &Scene) -> Check {
    let (pick, ingot) = (find(bot, "minecraft:iron_pickaxe")?, find(bot, "minecraft:iron_ingot")?);
    bot.anvil(s.at("anvil"), pick, Some(ingot), None).await?;
    let ours = stacks(bot, "minecraft:iron_pickaxe").first().and_then(|p| p.nbt.as_ref().and_then(|n| n.value.get("Damage").cloned()));
    let server = server_inv(bot, "minecraft:iron_pickaxe").await?;
    if server.contains("\"damage\":200") {
        return Err(format!("not repaired: server {server}").into());
    }
    Ok(format!("repaired: server {server}; bot sees Damage {ours:?}"))
}

pub async fn anvil_rename(bot: &mut Bot, s: &Scene) -> Check {
    bot.anvil(s.at("anvil"), find(bot, "minecraft:name_tag")?, None, Some("Tag")).await?;
    let server = server_inv(bot, "minecraft:name_tag").await?;
    let ours = stacks(bot, "minecraft:name_tag").first().and_then(|t| t.custom_name.clone());
    if !server.contains("\"name\":\"Tag\"") {
        return Err(format!("server {server}; bot sees {ours:?}").into());
    }
    Ok(format!("server {server}; bot sees {ours:?}"))
}

pub async fn grindstone(bot: &mut Bot, s: &Scene) -> Check {
    let before = server_inv(bot, "minecraft:golden_sword").await?;
    if before.contains("\"ench\":[]") {
        return Err(format!("the scene's sword is not enchanted: {before}").into());
    }
    bot.grindstone(s.at("grindstone"), find(bot, "minecraft:golden_sword")?, None).await?;
    let ours = stacks(bot, "minecraft:golden_sword").first().map(|s| has_nbt(s, "ench"));
    let server = server_inv(bot, "minecraft:golden_sword").await?;
    if !server.contains("\"ench\":[]") || ours != Some(false) {
        return Err(format!("server {server}; bot sees enchantments: {ours:?}").into());
    }
    Ok(format!("server {server}"))
}

pub async fn stonecut(bot: &mut Bot, s: &Scene) -> Check {
    let made = bot.stonecut(s.at("stonecutter"), "minecraft:stone_bricks", 4).await?;
    let have = count(bot, "minecraft:stone_bricks");
    if have < 4 {
        return Err(format!("made {made}, have {have} stone bricks").into());
    }
    Ok(format!("made {made} stone bricks"))
}

pub async fn smith(bot: &mut Bot, s: &Scene) -> Check {
    let (sword, ingot) = (find(bot, "minecraft:diamond_sword")?, find(bot, "minecraft:netherite_ingot")?);
    let template = find(bot, "minecraft:netherite_upgrade_smithing_template")?;
    bot.smith(s.at("smithing_table"), sword, ingot, template).await?;
    match count(bot, "minecraft:netherite_sword") {
        1 => Ok("netherite sword".into()),
        n => Err(format!("{n} netherite swords").into()),
    }
}

pub async fn loom(bot: &mut Bot, s: &Scene) -> Check {
    let (banner, dye) = (find(bot, "minecraft:banner")?, find(bot, "minecraft:red_dye")?);
    bot.loom(s.at("loom"), banner, dye, "bo", None).await?;
    let banner = stacks(bot, "minecraft:banner").into_iter().next().ok_or("the banner is gone")?;
    if !has_nbt(banner, "Patterns") {
        return Err(format!("banner without patterns: {:?}", banner.nbt).into());
    }
    Ok(format!("banner nbt {:?}", banner.nbt))
}

/// Trades the first offer the inventory can pay with the nearest entity of `kind`.
pub async fn trade(bot: &mut Bot, kind: &str) -> Check {
    let me = bot.state().player.position.clone();
    let trader = bot.state().entities.nearest(&me, |e| e.kind.starts_with(kind)).map(|e| e.runtime_id).ok_or(format!("no {kind} tracked"))?;
    // The offers are only known while the screen is open: peek at them first. A scene villager
    // between jobs cannot trade (tools/actiontest-pack/README.md), hence the retries.
    let mut seen = Vec::new();
    for _ in 0..3 {
        bot.interact_entity(trader).await?;
        for _ in 0..60 {
            if bot.state().stations.trade.is_some() {
                break;
            }
            bot.wait_ticks(1).await?;
        }
        if bot.state().stations.trade.is_some() {
            break;
        }
        seen.push(server_entities(bot, kind).await);
        bot.wait_ticks(100).await?;
    }
    let Some(offers) = bot.state().stations.trade.as_ref().map(|t| t.offers.clone()) else {
        return Err(format!("no trade screen opened; server sees {seen:?}").into());
    };
    bot.close_container().await?;
    bot.wait_ticks(10).await?;
    let have = |item: &TradeItem| -> u16 {
        stacks(bot, &item.name).iter().filter(|s| item.metadata == i16::MAX || s.metadata == item.metadata as u32).map(|s| s.count).sum()
    };
    let affordable =
        offers.iter().position(|o| o.buy_a.count <= have(&o.buy_a) && o.buy_b.as_ref().is_none_or(|b| b.count <= have(b)) && o.tier == 0);
    let Some(offer) = affordable else {
        return Err(format!("nothing affordable in {offers:?}").into());
    };
    let sell = offers[offer].sell.name.clone();
    let before = count(bot, &sell);
    let n = bot.trade(trader, offer, 1).await.map_err(|e| format!("{e}; offer {offer} of {offers:?}"))?;
    let after = count(bot, &sell);
    if after <= before {
        return Err(format!("{n} trade(s) of offer {offer} but {sell} {before} -> {after}").into());
    }
    Ok(format!("offer {offer}: {sell} {before} -> {after}; failed opens {seen:?}; server sees {}", server_entities(bot, kind).await))
}
