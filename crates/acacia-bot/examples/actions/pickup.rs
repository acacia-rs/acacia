//! Predicted inventory changes BDS never sends: picking items up and putting armour on by use.
use std::time::Duration;

use acacia_bot::Bot;
use acacia_bot::items::SlotRef;
use acacia_bot::survival::{Destination, EquipMethod};

use crate::{count, find, server_inv, stack, Check};

/// The pack drops each stack at the bot's feet; the predicted slots must match the server's.
pub async fn pickup(bot: &mut Bot) -> Check {
    let mut seen = Vec::new();
    for (item, n) in [("minecraft:wheat", 3), ("minecraft:apple", 2)] {
        let before = count(bot, item);
        bot.client().command(&format!("/scriptevent actiontest:drop {item} {n}"));
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while count(bot, item) < before + n {
            if tokio::time::Instant::now() > deadline {
                return Err(format!("{item}: {before} -> {} after the drop", count(bot, item)).into());
            }
            bot.wait_ticks(1).await?;
        }
        let ours: Vec<(usize, u16)> = bot.state().inventory.main.iter().enumerate()
            .filter(|(_, s)| !s.is_empty() && bot.state().item_name(s) == Some(item))
            .map(|(i, s)| (i, s.count))
            .collect();
        let server = server_inv(bot, item).await?;
        if let Some((slot, n)) = ours.iter().find(|(slot, n)| !server.contains(&format!("\"slot\":{slot},\"id\":\"{item}\",\"count\":{n}"))) {
            return Err(format!("{item}: bot has {n} in slot {slot}, server has {server}").into());
        }
        seen.push(format!("{item} {ours:?}"));
    }
    Ok(seen.join(", "))
}

pub async fn equip_use(bot: &mut Bot) -> Check {
    bot.equip_by(find(bot, "minecraft:iron_chestplate")?, Destination::Chest, EquipMethod::Use).await?;
    bot.wait_ticks(10).await?;
    let worn = stack(bot, SlotRef::Armor(1)).and_then(|s| bot.state().item_name(s)).unwrap_or("nothing").to_owned();
    let server = server_inv(bot, "minecraft:iron_chestplate").await?;
    if worn != "minecraft:iron_chestplate" || server != "[]" {
        return Err(format!("bot wears {worn}; server inventory has {server}").into());
    }
    Ok(format!("chestplate worn, hand now {:?}", bot.state().held_item().network_id))
}
