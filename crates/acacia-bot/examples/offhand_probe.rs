//! Does BDS use the offhand stack for a `UseItem` with `hand` = `OffHand`? `OFFHAND_ITEM` (default golden_apple) in
//! the offhand and an empty main hand: a control `use_item` (main hand) should do nothing, `use_item_in(Offhand)`
//! eats / throws / raises it; then the same item in the main hand as the positive control. Shows the stack count
//! and whether the player's `BLOCKING` flag was ever set in the 45 ticks after each use.
//! Result on strict BDS 1.26 (2026-11-10): nothing is used from the offhand (apple, snowball); the shield's flag
//! never showed for the main hand either, so shields are unproven.
//! `cargo run -p acacia-bot --example offhand_probe -- <server> [name]`
mod support;

use acacia_bot::items::SlotRef;
use acacia_bot::proto::types::MetadataFlags2;
use acacia_bot::Bot;
use support::Error;

/// Ticks 45 and reports whether the observer saw the player blocking at any of them (a player's own metadata is not kept).
async fn watch(bot: &mut Bot, observer: &mut Bot) -> Result<bool, Error> {
    let mut blocking = false;
    for _ in 0..45 {
        bot.wait_ticks(1).await?;
        observer.wait_ticks(1).await?;
        let me = observer.state().player.position.clone();
        let seen = observer.state().entities.nearest(&me, |e| e.is_player());
        blocking |= seen.is_some_and(|e| e.metadata.flags_extended().contains(MetadataFlags2::BLOCKING));
    }
    Ok(blocking)
}

async fn give(bot: &mut Bot, cmds: &[String]) -> Result<(), Error> {
    for cmd in cmds {
        bot.client().command(cmd);
        bot.wait_ticks(5).await?;
    }
    bot.wait_ticks(10).await?;
    Ok(())
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

    let item = std::env::var("OFFHAND_ITEM").unwrap_or_else(|_| "golden_apple".into());
    give(&mut bot, &["/clear @s".into(), format!("/replaceitem entity @s slot.weapon.offhand 0 {item} 3")]).await?;
    println!("start: offhand {:?} x{}, held {:?}", bot.state().inventory.offhand.network_id, bot.state().inventory.offhand.count, bot.state().inventory.held().network_id);

    bot.use_item();
    let blocking = watch(&mut bot, &mut observer).await?;
    println!("control (main hand empty, use_item): offhand x{}, blocking {blocking}", bot.state().inventory.offhand.count);
    bot.release_item();
    bot.wait_ticks(5).await?;

    bot.use_item_in(SlotRef::Offhand);
    let blocking = watch(&mut bot, &mut observer).await?;
    println!("use_item_in(Offhand): offhand x{}, blocking {blocking}", bot.state().inventory.offhand.count);
    bot.release_item();
    bot.wait_ticks(5).await?;

    give(&mut bot, &["/clear @s".into(), format!("/give @s {item} 3")]).await?;
    let slot = bot.find_item(&format!("minecraft:{item}")).ok_or("no item")?;
    bot.equip(slot, acacia_bot::survival::Destination::Hand).await?;
    bot.use_item();
    let blocking = watch(&mut bot, &mut observer).await?;
    println!("main hand, use_item: held x{}, blocking {blocking}", bot.state().inventory.held().count);
    bot.release_item();
    bot.wait_ticks(5).await?;
    Ok(())
}
