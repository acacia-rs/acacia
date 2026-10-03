//! Live test of the gameplay actions against BDS with tools/actiontest-pack (setup: its README).
//! Prints PASS/FAIL per action, verified from the bot's inventory and the pack's chat log.
//! `cargo run --release -p acacia-bot --example actions -- <server> <name> [idle]`
//! `idle`: no physics (no aiming, no best tool, no gliding). `ONLY=craft,sign` runs a subset.
use std::collections::HashMap;
use std::error::Error;
use std::time::Duration;

use acacia_bot::client::Client;
use acacia_bot::events::ChatPattern;
use acacia_bot::items::SlotRef;
use acacia_bot::state::{ItemStack, Trackers};
use acacia_bot::{Bot, BotConfig, BotEvent, Events};

#[path = "actions/stations.rs"]
mod stations;
#[path = "actions/survival.rs"]
mod survival;
#[path = "actions/pickup.rs"]
mod pickup;

pub type Check = Result<String, Box<dyn Error>>;
pub type Pos = [i32; 3];

/// Block positions the pack reported (`ACTIONTEST scene name=x,y,z ...`).
pub struct Scene(HashMap<String, Pos>);

impl Scene {
    fn parse(line: &str) -> Scene {
        let coords = line.split_whitespace().filter_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            let v: Vec<i32> = v.split(',').filter_map(|n| n.parse().ok()).collect();
            Some((k.to_owned(), [*v.first()?, *v.get(1)?, *v.get(2)?]))
        });
        Scene(coords.collect())
    }

    pub fn at(&self, name: &str) -> Pos {
        self.0[name]
    }
}

pub fn count(bot: &Bot, name: &str) -> u32 {
    bot.state().inventory_summary().into_iter().find(|(n, _)| n == name).map_or(0, |(_, c)| c)
}

pub fn find(bot: &Bot, name: &str) -> Result<SlotRef, Box<dyn Error>> {
    bot.find_item(name).ok_or_else(|| format!("no {name} in the inventory").into())
}

pub fn stack<'a>(bot: &'a Bot, slot: SlotRef) -> Option<&'a ItemStack> {
    let inv = &bot.state().inventory;
    match slot {
        SlotRef::Main(i) => inv.main.get(usize::from(i)),
        SlotRef::Armor(i) => inv.armor.get(usize::from(i)),
        _ => None,
    }
}

/// Main-inventory stacks of item `name`.
pub fn stacks<'a>(bot: &'a Bot, name: &str) -> Vec<&'a ItemStack> {
    bot.state().inventory.main.iter().filter(|s| !s.is_empty() && bot.state().item_name(s) == Some(name)).collect()
}

pub fn has_nbt(stack: &ItemStack, key: &str) -> bool {
    stack.nbt.as_ref().is_some_and(|n| n.value.get(key).is_some())
}

/// Waits for a chat line from the pack matching pattern `name`; returns its first group.
pub async fn wait_chat(bot: &mut Bot, name: &str, timeout: Duration) -> Option<String> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match tokio::time::timeout_at(deadline, bot.next()).await {
            Ok(Some(BotEvent::ChatMatch(m))) if m.pattern == name => return m.groups.into_iter().next(),
            Ok(Some(BotEvent::Disconnected(r))) => panic!("disconnected: {r:?}"),
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => return None,
        }
    }
}

/// First groups of the queued chat matches of pattern `name`, in order (other events are dropped).
pub async fn chat_lines(bot: &mut Bot, name: &str) -> Vec<String> {
    let mut lines = Vec::new();
    while let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(200), bot.next()).await {
        if let BotEvent::ChatMatch(m) = event
            && m.pattern == name
        {
            lines.extend(m.groups.into_iter().next());
        }
    }
    lines
}

/// `x,y,z` as floats.
pub fn parse_pos(s: &str) -> Option<[f32; 3]> {
    let v: Vec<f32> = s.split(',').filter_map(|n| n.parse().ok()).collect();
    Some([*v.first()?, *v.get(1)?, *v.get(2)?])
}

/// Chat lines among the events queued so far (the pack's own lines left out).
pub async fn recent_chat(bot: &mut Bot) -> Vec<String> {
    let mut lines = Vec::new();
    while let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(200), bot.next()).await {
        if let BotEvent::Chat(m) = event
            && !m.plain().contains("ACTIONTEST")
        {
            lines.push(m.plain());
        }
    }
    lines
}

/// The server's view of the bot's stacks of `item` (JSON from the pack's `actiontest:inv` event).
pub async fn server_inv(bot: &mut Bot, item: &str) -> Result<String, Box<dyn Error>> {
    bot.client().command(&format!("/scriptevent actiontest:inv {item}"));
    Ok(wait_chat(bot, "inv", Duration::from_secs(5)).await.ok_or("no inventory dump from the pack")?)
}

/// The server's view of the scene's entities of type `kind` (the pack's `actiontest:ent` event).
pub async fn server_entities(bot: &mut Bot, kind: &str) -> String {
    bot.client().command(&format!("/scriptevent actiontest:ent {kind}"));
    wait_chat(bot, "ent", Duration::from_secs(5)).await.unwrap_or_else(|| "no entity dump from the pack".into())
}

macro_rules! run {
    ($results:ident, $name:literal, $check:expr) => {
        let only = std::env::var("ONLY").ok();
        if only.as_deref().is_none_or(|o| o.split(',').any(|n| n == $name)) {
            let result: Check = $check.await;
            match &result {
                Ok(detail) => println!("PASS {:<16} {detail}", $name),
                Err(e) => println!("FAIL {:<16} {e}", $name),
            }
            $results.push(($name, result.is_ok()));
        }
    };
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19170".into());
    let name = args.next().unwrap_or_else(|| "ActionBot".into());
    let physics = args.next().as_deref() != Some("idle");
    let config = BotConfig {
        physics,
        trackers: Trackers { entities: true, ..Trackers::default() },
        events: Events::CHAT,
        chat_patterns: ["scene", "sign", "inv", "ent", "dismounted"]
            .into_iter()
            .map(|name| ChatPattern::new(name, &format!("ACTIONTEST {name} (.*)")))
            .collect::<Result<_, _>>()?,
        ..BotConfig::default()
    };
    let mut bot = Bot::connect(Client::builder(&server).offline(&name), config).await?;
    let line = wait_chat(&mut bot, "scene", Duration::from_secs(30)).await.ok_or("no scene message from the pack")?;
    let scene = Scene::parse(&line);
    while physics && bot.movement().is_none_or(|m| !m.is_started()) {
        bot.wait_ticks(1).await?;
    }
    bot.wait_ticks(20).await?;
    println!("scene: {line}\ninventory: {:?}", bot.state().inventory_summary());

    let mut results = Vec::new();
    let s = &scene;
    run!(results, "craft", stations::craft(&mut bot, s));
    run!(results, "smelt", stations::smelt(&mut bot, s));
    run!(results, "brew", stations::brew(&mut bot, s));
    run!(results, "enchant", stations::enchant(&mut bot, s));
    run!(results, "anvil", stations::anvil(&mut bot, s));
    run!(results, "anvil_rename", stations::anvil_rename(&mut bot, s));
    run!(results, "grindstone", stations::grindstone(&mut bot, s));
    run!(results, "stonecut", stations::stonecut(&mut bot, s));
    run!(results, "smith", stations::smith(&mut bot, s));
    run!(results, "loom", stations::loom(&mut bot, s));
    run!(results, "trade", stations::trade(&mut bot, "minecraft:wandering_trader"));
    run!(results, "trade_villager", stations::trade(&mut bot, "minecraft:villager"));
    run!(results, "consume", survival::consume(&mut bot));
    run!(results, "equip", survival::equip(&mut bot));
    run!(results, "equip_use", pickup::equip_use(&mut bot));
    run!(results, "pickup", pickup::pickup(&mut bot));
    run!(results, "equip_best_tool", survival::best_tool(&mut bot, s));
    run!(results, "write_sign", survival::sign(&mut bot, s));
    run!(results, "book", survival::book(&mut bot));
    run!(results, "sleep", survival::sleep(&mut bot, s));
    run!(results, "mount_pig", survival::ride(&mut bot, "minecraft:pig"));
    run!(results, "mount_boat", survival::ride(&mut bot, "minecraft:boat"));
    run!(results, "fish", survival::fish(&mut bot, s));
    run!(results, "equip_elytra", survival::elytra(&mut bot));
    run!(results, "glide", survival::glide(&mut bot));

    let failed: Vec<_> = results.iter().filter(|(_, ok)| !ok).map(|(n, _)| *n).collect();
    println!("{} passed, {} failed {failed:?}", results.len() - failed.len(), failed.len());
    bot.disconnect().await;
    Ok(())
}
