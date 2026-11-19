//! `destroy`: ten dirt in the first hotbar slot. In survival `destroy_item` is refused; in creative it deletes three,
//! then the other seven, each read back from the server's copy of the slot with `tellraw`. Then the own inventory
//! opens and closes.

use acacia_bot::items::SlotRef;
use acacia_bot::BotEvent;

use crate::drills::{check, start, Outcome};
use crate::Drill;

pub const NAMES: &[&str] = &["destroy"];

/// The server's count in hotbar slot 0 after `label`, e.g. "destroy-3 7"; just the label when the slot is empty.
async fn server_count(d: &mut Drill, label: &str) -> Result<String, String> {
    d.bot.command(&format!(r#"tellraw @s ["{label} ",{{"nbt":"Inventory[{{Slot:0b}}].count","entity":"@s"}}]"#));
    d.ticks(20).await?;
    let events = d.events.lock().unwrap();
    let said = events.iter().rev().find_map(|e| match e {
        BotEvent::Chat(m) if m.message.starts_with(label) => Some(m.message.trim().to_owned()),
        _ => None,
    });
    Ok(said.unwrap_or_default())
}

pub async fn run(d: &mut Drill, x: i32) -> Outcome {
    let base = start(d, x, &["give @s dirt 10"]).await?;
    let survival = d.bot.destroy_item(SlotRef::Main(0), 3).await;
    d.bot.command("gamemode creative @s");
    d.ticks(10).await?;
    let some = d.bot.destroy_item(SlotRef::Main(0), 3).await;
    let after_some = server_count(d, "destroy-3").await?;
    let rest = d.bot.destroy_item(SlotRef::Main(0), 7).await;
    let after_rest = server_count(d, "destroy-all").await?;
    d.bot.open_inventory();
    d.ticks(2).await?;
    let closed = d.bot.close_container().await;
    d.ticks(5).await?;
    let ok = survival.is_err() && some.is_ok() && after_some == "destroy-3 7" && rest.is_ok() && after_rest == "destroy-all" && closed.is_ok();
    let what = || format!("survival {survival:?}, 3: {some:?} -> {after_some:?}, 7: {rest:?} -> {after_rest:?}, close {closed:?}");
    Ok((base, check(ok, what)))
}
