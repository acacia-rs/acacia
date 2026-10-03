//! Live check of phase-3 actions on a server where the bot is operator (sets up its own scene with
//! commands): hotbar, break, place, chest open/transfer/close, inventory moves, drop, attack.
//! `cargo run -p acacia-bot --features socks --example interact -- <server> <name|@account>`
use std::sync::Arc;
use std::time::Instant;

use acacia_bot::client::auth::{Account, AuthClient, AuthConfig, FileTokenCache};
use acacia_bot::client::Client;
use acacia_bot::interact::Face;
use acacia_bot::items::SlotRef;
use acacia_bot::state::Trackers;
use acacia_bot::{ActionError, Bot, BotConfig};
use acacia_world::BlockAccess;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19140".into());
    let name = args.next().unwrap_or_else(|| "@default".into());
    let mut builder = Client::builder(&server);
    builder = match name.strip_prefix('@') {
        Some(account) => {
            let account = Account::new(Arc::new(AuthClient::new(AuthConfig::default())?), Arc::new(FileTokenCache::new(".tokens")?), account);
            let (key, credentials) = account.login_credentials().await?;
            builder.online(credentials, key)
        }
        None => builder.offline(&name),
    };
    let config = BotConfig { physics: true, trackers: Trackers { entities: true, ..Trackers::default() }, ..BotConfig::default() };
    let mut bot = Bot::connect(builder, config).await?;
    while bot.movement().is_none_or(|m| !m.is_started()) {
        bot.wait_ticks(1).await?;
    }
    let feet = bot.movement().and_then(|m| m.position()).expect("started");
    let [x, y, z] = [feet[0].floor() as i32, feet[1].floor() as i32, feet[2].floor() as i32];
    let stone = [x + 2, y, z];
    let chest = [x, y, z + 2];
    for cmd in [
        "/clear @s".to_string(),
        format!("/setblock {} {} {} stone", stone[0], stone[1], stone[2]),
        format!("/setblock {} {} {} chest", chest[0], chest[1], chest[2]),
        "/give @s wooden_pickaxe".into(),
        "/give @s dirt 16".into(),
        format!("/summon pig {} {} {}", x - 2, y, z),
    ] {
        bot.client().command(&cmd);
        bot.wait_ticks(4).await?;
    }
    bot.wait_ticks(20).await?;
    println!("scene at feet ({x},{y},{z}); items: {:?}", bot.state().inventory_summary());

    let slot_of = |bot: &Bot, item: &str| {
        (0..9u8).find(|&s| bot.state().item_name(&bot.state().inventory.main[usize::from(s)]) == Some(item))
    };
    let block_name = |bot: &Bot, p: [i32; 3]| {
        let w = bot.world().expect("physics");
        w.registry().and_then(|r| r.get(w.view()?.block(p[0], p[1], p[2]))).map_or("?", |s| s.name)
    };

    let pick = slot_of(&bot, "minecraft:wooden_pickaxe").ok_or("no pickaxe")?;
    report("select pickaxe", bot.select_hotbar(pick), Instant::now());
    let t = Instant::now();
    report("break stone (expect ~1.15s)", bot.break_block(stone).await, t);
    println!("    block now: {}", block_name(&bot, stone));

    let dirt = slot_of(&bot, "minecraft:dirt").ok_or("no dirt")?;
    report("select dirt", bot.select_hotbar(dirt), Instant::now());
    let t = Instant::now();
    report("place dirt on ground", bot.place_block([stone[0], stone[1] - 1, stone[2]], Face::Up).await, t);
    println!("    block now: {}", block_name(&bot, stone));

    let t = Instant::now();
    match bot.open_container_at(chest).await {
        Ok(c) => {
            println!("OK   open chest {:?} window {} ({} slots) in {:?}", c.window_type, c.window_id, c.slots.len(), t.elapsed());
            let t = Instant::now();
            report("put 4 dirt in chest slot 0", bot.move_item(SlotRef::Main(dirt), SlotRef::Container(0), 4).await, t);
            let t = Instant::now();
            report("take chest slot 0 back", bot.move_to_inventory(0).await.map(drop), t);
            let t = Instant::now();
            report("close chest", bot.close_container().await, t);
        }
        Err(e) => println!("FAIL open chest: {e}"),
    }

    let t = Instant::now();
    report("move 2 dirt to Main(20)", bot.move_item(SlotRef::Main(dirt), SlotRef::Main(20), 2).await, t);
    let t = Instant::now();
    report("swap dirt with pickaxe", bot.swap_items(SlotRef::Main(dirt), SlotRef::Main(pick)).await, t);
    let t = Instant::now();
    report("drop 1 from Main(20)", bot.drop_item(SlotRef::Main(20), 1).await, t);
    println!("    items: {:?}", bot.state().inventory_summary());

    let eye = bot.eye_position();
    let pig = bot.state().entities.nearest(&acacia_bot::proto::types::Vec3f { x: eye[0], y: eye[1], z: eye[2] }, |e| e.kind == "minecraft:pig").map(|e| e.runtime_id);
    match pig {
        Some(id) => {
            let t = Instant::now();
            report("attack pig", bot.attack(id).await, t);
        }
        None => println!("SKIP attack: no pig tracked"),
    }
    bot.wait_ticks(10).await?;
    let m = bot.movement().expect("physics");
    println!("done: corrections {} teleports {}", m.corrections, m.teleports);
    bot.disconnect().await;
    Ok(())
}

fn report(what: &str, result: Result<(), ActionError>, since: Instant) {
    match result {
        Ok(()) => println!("OK   {what} ({:?})", since.elapsed()),
        Err(e) => println!("FAIL {what}: {e} ({:?})", since.elapsed()),
    }
}
