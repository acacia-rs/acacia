//! Joins a server and prints the tracked game state every few seconds.
//! `cargo run -p acacia-bot --features socks --example state -- <server> <name|@account> <seconds>`
//! `BEDROCK_PROXY=host:port:user:pass`, `BEDROCK_ENTITIES=1` (entity tracking), `BEDROCK_CMD="/cmd;chat"`,
//! `BEDROCK_DEBUG_SCORES=1` (print raw scoreboard packets), `BEDROCK_SKIP_PACKS=1` (join as a player
//! with every resource pack cached), `BEDROCK_DEBUG_PACKETS=1` (print every packet received; with
//! `RUST_LOG=acacia_client::send=trace` every packet sent too).
use std::sync::Arc;
use std::time::Duration;

use acacia_bot::client::auth::{Account, AuthClient, AuthConfig, FileTokenCache};
use acacia_bot::client::{Client, EveryPack, PacketFilter, Socks5Proxy};
use acacia_bot::proto::packets::{RemoveObjective, SetDisplayObjective, SetScore};
use acacia_bot::proto::Packet;
use acacia_bot::state::Trackers;
use acacia_bot::text::strip_formatting;
use acacia_bot::{Bot, BotConfig, BotEvent, GameState};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19140".into());
    let name = args.next().unwrap_or_else(|| "Bot".into());
    let secs: u64 = args.next().map_or(20, |s| s.parse().expect("seconds"));
    let proxy = std::env::var("BEDROCK_PROXY").ok().map(|p| Socks5Proxy::parse(&p)).transpose()?;

    let mut builder = Client::builder(&server).login_timeout(Duration::from_secs(180));
    if let Some(p) = &proxy {
        builder = builder.proxy(p.clone());
    }
    if std::env::var_os("BEDROCK_SKIP_PACKS").is_some() {
        builder = builder.pack_store(Arc::new(EveryPack));
    }
    builder = match name.strip_prefix('@') {
        Some(account) => {
            let config = AuthConfig { proxy: proxy.as_ref().map(Socks5Proxy::to_url), ..AuthConfig::default() };
            let account = Account::new(Arc::new(AuthClient::new(config)?), Arc::new(FileTokenCache::new(".tokens")?), account);
            let (key, credentials) = account.login_credentials().await?;
            builder.online(credentials, key)
        }
        None => builder.offline(&name),
    };
    let trackers = Trackers { entities: std::env::var("BEDROCK_ENTITIES").is_ok(), ..Trackers::default() };
    let debug_scores = std::env::var("BEDROCK_DEBUG_SCORES").is_ok();
    let debug_packets = std::env::var("BEDROCK_DEBUG_PACKETS").is_ok();
    let subscribe = if debug_packets {
        PacketFilter::all()
    } else if debug_scores {
        [SetScore::ID, SetDisplayObjective::ID, RemoveObjective::ID].into_iter().collect()
    } else {
        PacketFilter::none()
    };
    let start = std::time::Instant::now();
    let mut bot = Bot::connect(builder, BotConfig { trackers, subscribe, ..BotConfig::default() }).await?;
    println!("spawned as {}", bot.client().display_name());

    let mut script = std::env::var("BEDROCK_CMD").unwrap_or_default().split(';').filter(|s| !s.is_empty()).map(str::to_owned).collect::<Vec<_>>().into_iter();
    let mut report = tokio::time::interval(Duration::from_secs(5));
    let deadline = tokio::time::sleep(Duration::from_secs(secs));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            event = bot.next() => match event {
                Some(BotEvent::Disconnected(reason)) => { println!("disconnected: {reason:?}"); break; }
                Some(BotEvent::Packet(p)) if debug_packets => println!("[{:>6} ms] <- {} ({} bytes){}", start.elapsed().as_millis(), p.id, p.body.len(), describe(&p)),
                Some(BotEvent::Packet(p)) => {
                    if let Ok(s) = p.decode::<SetScore>() { println!("[SetScore] {s:?}"); }
                    if let Ok(d) = p.decode::<SetDisplayObjective>() { println!("[SetDisplayObjective] {d:?}"); }
                    if let Ok(r) = p.decode::<RemoveObjective>() { println!("[RemoveObjective] {r:?}"); }
                }
                Some(_) => {}
                None => break,
            },
            _ = report.tick() => {
                if let Some(line) = script.next() {
                    if line.starts_with('/') { bot.client().command(&line); } else { bot.client().chat(line); }
                }
                print_state(bot.state());
            }
            _ = &mut deadline => bot.client().close(),
        }
    }
    Ok(())
}

/// The decoded form of the small packets that say what the server is doing with the player.
fn describe(p: &acacia_bot::proto::RawPacket) -> String {
    use acacia_bot::proto::packets::{Animate, Emote, EmoteList, InventorySlot, MobEquipment, PlayerHotbar, Text};
    macro_rules! first_decoded {
        ($($t:ty),*) => {$(
            if let Ok(packet) = p.decode::<$t>() {
                return format!(" {packet:?}");
            }
        )*};
    }
    first_decoded!(Text, MobEquipment, Animate, Emote, EmoteList, PlayerHotbar, InventorySlot);
    String::new()
}

fn print_state(s: &GameState) {
    let p = &s.player;
    println!("---");
    println!(
        "pos ({:.1}, {:.1}, {:.1}) yaw {:.0} | {:?} dim {} | hp {:.0}/{:.0} food {:.0} lvl {} alive {}",
        p.position.x, p.position.y, p.position.z, p.yaw, p.game_mode, p.dimension, p.health, p.max_health, p.hunger, p.xp_level, p.alive
    );
    if let Some(o) = s.scoreboard.sidebar() {
        let lines: Vec<String> = o.lines().iter().map(|(name, score)| format!("{}: {score}", strip_formatting(name))).collect();
        println!("sidebar [{}]: {}", strip_formatting(&o.display_name), lines.join(" | "));
    }
    let held = s.item_name(s.held_item()).unwrap_or("-");
    println!("held: {held} | items: {:?}", s.inventory_summary());
    if let Some(c) = &s.containers.open {
        println!("open container: {:?} window {} ({} slots)", c.window_type, c.window_id, c.slots.len());
    }
    println!("players online (tab list): {} | item registry: {} | entities tracked: {}", s.player_list.len(), s.items.len(), s.entities.len());
}
