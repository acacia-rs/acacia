//! Harmless inventory round-trip for servers where the bot has no permissions: select hotbar
//! slots, move the first stack to an empty slot and back, and check the tracked state agrees.
//! `cargo run -p acacia-bot --features socks --example inventory_check -- <server> @account`
use std::sync::Arc;
use std::time::Instant;

use acacia_bot::client::auth::{Account, AuthClient, AuthConfig, FileTokenCache};
use acacia_bot::client::{Client, Socks5Proxy};
use acacia_bot::items::SlotRef;
use acacia_bot::{Bot, BotConfig};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().ok_or("server required")?;
    let account = args.next().ok_or("@account required")?;
    let account = account.strip_prefix('@').ok_or("online account (@name) required")?;
    let proxy = std::env::var("BEDROCK_PROXY").ok().map(|p| Socks5Proxy::parse(&p)).transpose()?;
    let mut builder = Client::builder(&server);
    if let Some(p) = &proxy {
        builder = builder.proxy(p.clone());
    }
    let config = AuthConfig { proxy: proxy.as_ref().map(Socks5Proxy::to_url), ..AuthConfig::default() };
    let account = Account::new(Arc::new(AuthClient::new(config)?), Arc::new(FileTokenCache::new(".tokens")?), account);
    let (key, credentials) = account.login_credentials().await?;
    let physics = std::env::var("BEDROCK_PHYSICS").is_ok();
    let mut bot = Bot::connect(builder.online(credentials, key), BotConfig { physics, ..BotConfig::default() }).await?;
    bot.wait_ticks(60).await?;

    let summary = |bot: &Bot| bot.state().inventory_summary();
    println!("start: {:?}", summary(&bot));
    let main = &bot.state().inventory.main;
    let from = (0..36u8).find(|&s| !main[usize::from(s)].is_empty()).ok_or("inventory empty")?;
    let to = (9..36u8).find(|&s| main[usize::from(s)].is_empty()).ok_or("no empty slot")?;
    let item = bot.state().item_name(&main[usize::from(from)]).unwrap_or("?").to_owned();

    // BEDROCK_PREMOVE=a:b moves the whole stack Main(a) -> Main(b) before any slot switching.
    if let Some((a, b)) = std::env::var("BEDROCK_PREMOVE").ok().and_then(|s| s.split_once(':').map(|(a, b)| (a.parse::<u8>(), b.parse::<u8>()))) {
        let (a, b) = (a?, b?);
        let count = bot.state().inventory.main[usize::from(a)].count as u8;
        println!("premove Main({a}) -> Main({b}): {:?}", bot.move_item(SlotRef::Main(a), SlotRef::Main(b), count).await);
    }
    // BEDROCK_SELECT: comma-separated hotbar slots to select, 20 ticks apart (default "1,0").
    let slots = std::env::var("BEDROCK_SELECT").unwrap_or_else(|_| "1,0".into());
    for slot in slots.split(',').filter_map(|s| s.trim().parse::<u8>().ok()) {
        println!("select hotbar {slot}: {:?}", bot.select_hotbar(slot));
        bot.wait_ticks(20).await?;
    }
    // BEDROCK_SETTLE_TICKS: idle this long after switching slots (diagnoses whether the server still processes us).
    let settle: u32 = std::env::var("BEDROCK_SETTLE_TICKS").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    if settle > 0 {
        let result = bot.wait_ticks(settle).await;
        println!("idled {settle} ticks after switching: {result:?} (inventory open: {:?})", bot.state().containers.open.as_ref().map(|c| c.window_id));
    }
    for (a, b) in [(from, to), (to, from)] {
        let count = bot.state().inventory.main[usize::from(a)].count as u8;
        let t = Instant::now();
        let result = bot.move_item(SlotRef::Main(a), SlotRef::Main(b), count).await;
        let now = bot.state().item_name(&bot.state().inventory.main[usize::from(b)]).unwrap_or("empty").to_owned();
        println!("move {item} x{count} Main({a}) -> Main({b}): {result:?} in {:?}; slot {b} now {now}", t.elapsed());
    }
    bot.wait_ticks(40).await?;
    println!("end:   {:?}", summary(&bot));
    bot.disconnect().await;
    Ok(())
}
