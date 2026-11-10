//! Held uses from the offhand on strict BDS: `OFFHAND_ITEM` (default shield) in the offhand, `MAIN_ITEM` (default
//! none) in the main hand, `USE_TICKS` (default 25) held before the release. Each phase reports the observer's
//! view of the player's `BLOCKING` flag, the offhand and arrow counts, and the observer's arrow entities.
//! Phases: `main` (the item in the main hand, control), `off` (`start_use_in(Offhand)`), `off-plain` (as `off`
//! without the `StartUsingItem` flag).
//! `cargo run -p acacia-bot --example hold_probe -- <server> [name]`
mod support;

use acacia_bot::items::SlotRef;
use acacia_bot::proto::types::MetadataFlags2;
use acacia_bot::Bot;
use support::Error;

struct Seen {
    blocking: bool,
    arrows: usize,
    flags: String,
}

async fn watch(bot: &mut Bot, observer: &mut Bot, ticks: u32) -> Result<Seen, Error> {
    let mut seen = Seen { blocking: false, arrows: 0, flags: String::new() };
    for _ in 0..ticks {
        bot.wait_ticks(1).await?;
        observer.wait_ticks(1).await?;
        let me = observer.state().player.position.clone();
        let player = observer.state().entities.nearest(&me, |e| e.is_player());
        seen.blocking |= player.is_some_and(|e| e.metadata.flags_extended().contains(MetadataFlags2::BLOCKING));
        if let Some(e) = player {
            seen.flags = format!("{:?} {:?}", e.metadata.flags(), e.metadata.flags_extended());
        }
        let arrows = observer.state().entities.iter().filter(|e| e.kind.contains("arrow")).count();
        seen.arrows = seen.arrows.max(arrows);
    }
    Ok(seen)
}

async fn give(bot: &mut Bot, cmds: &[String]) -> Result<(), Error> {
    for cmd in cmds {
        bot.client().command(cmd);
        bot.wait_ticks(5).await?;
    }
    bot.wait_ticks(10).await?;
    Ok(())
}

fn report(label: &str, bot: &Bot, seen: &Seen) {
    let inv = &bot.state().inventory;
    println!("{label}: blocking {} arrow entities {} offhand x{} held x{} last flags {}", seen.blocking, seen.arrows, inv.offhand.count, inv.held().count, seen.flags);
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19170".into());
    let name = args.next().unwrap_or_else(|| "Probe".into());
    let (mut bot, _pad) = support::connect(&server, &name).await?;
    let watching = acacia_bot::BotConfig { trackers: acacia_bot::state::Trackers { entities: true, ..Default::default() }, ..Default::default() };
    let (mut observer, _) = support::connect_config(&server, "ProbeObs", watching).await?;

    let item = std::env::var("OFFHAND_ITEM").unwrap_or_else(|_| "shield".into());
    let main_item = std::env::var("MAIN_ITEM").ok();
    let ticks: u32 = std::env::var("USE_TICKS").ok().and_then(|v| v.parse().ok()).unwrap_or(25);
    let setup = [format!("/replaceitem entity @s slot.weapon.offhand 0 {item} 1"), "/give @s arrow 16".into()];

    // TOTEM=1: a totem in the offhand, lethal damage; reports whether it was used up and the bot survived.
    if std::env::var("TOTEM").is_ok() {
        give(&mut bot, &["/clear @s".into(), "/replaceitem entity @s slot.weapon.offhand 0 totem_of_undying 1".into()]).await?;
        bot.client().command("/damage @s 100 entity_attack");
        bot.wait_ticks(30).await?;
        let s = bot.state();
        println!("totem: offhand x{} alive {} health {}", s.inventory.offhand.count, s.player.alive, s.player.health);
        return Ok(());
    }

    // EXTRA_FLAGS=66,67: one main-hand attempt per raw PlayerAuthInput flag number, sent with the use.
    if let Ok(list) = std::env::var("EXTRA_FLAGS") {
        give(&mut bot, &["/clear @s".into(), format!("/give @s {item} 1")]).await?;
        let slot = bot.find_item(&format!("minecraft:{item}")).ok_or("no item")?;
        bot.equip(slot, acacia_bot::survival::Destination::Hand).await?;
        let held = SlotRef::Main(bot.state().inventory.selected_hotbar_slot);
        for n in list.split(',').filter_map(|n| n.parse::<i64>().ok()) {
            bot.queue_input_flag(acacia_bot::proto::types::InputData::from_raw(n));
            bot.start_use_in(held);
            let seen = watch(&mut bot, &mut observer, 12).await?;
            bot.release_item_in(held);
            report(&format!("flag {n}"), &bot, &seen);
            bot.wait_ticks(8).await?;
        }
        return Ok(());
    }

    // Control: the item in the main hand.
    give(&mut bot, &["/clear @s".into(), format!("/give @s {item} 1"), "/give @s arrow 16".into()]).await?;
    let slot = bot.find_item(&format!("minecraft:{item}")).ok_or("no item")?;
    bot.equip(slot, acacia_bot::survival::Destination::Hand).await?;
    let held = SlotRef::Main(bot.state().inventory.selected_hotbar_slot);
    bot.start_use_in(held);
    let seen = watch(&mut bot, &mut observer, ticks).await?;
    bot.release_item_in(held);
    report("main", &bot, &seen);
    bot.wait_ticks(10).await?;

    for (label, flag) in [("off", true), ("off-plain", false)] {
        give(&mut bot, &["/clear @s".into()]).await?;
        give(&mut bot, &setup).await?;
        if let Some(m) = &main_item {
            give(&mut bot, &[format!("/give @s {m} 1")]).await?;
            let slot = bot.find_item(&format!("minecraft:{m}")).ok_or("no main item")?;
            bot.equip(slot, acacia_bot::survival::Destination::Hand).await?;
        }
        if flag {
            bot.start_use_in(SlotRef::Offhand);
        } else {
            bot.use_item_in(SlotRef::Offhand);
        }
        let seen = watch(&mut bot, &mut observer, ticks).await?;
        bot.release_item_in(SlotRef::Offhand);
        report(label, &bot, &seen);
        bot.wait_ticks(10).await?;
    }
    Ok(())
}
