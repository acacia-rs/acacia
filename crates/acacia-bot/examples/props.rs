//! A second player for viewer screenshots, on a flat-world server where the bot is operator: it
//! wears the flat skin with a cape,
//! stands at a spot facing north, leashes a cow it summons beside itself, looms a patterned
//! banner into its main hand, then sneaks with a shield in its off hand (Bedrock's way to block)
//! for a minute.
//! With `pace` it walks east and back instead of sneaking.
//! `cargo run -p acacia-bot --example props -- <server> [x y z of its feet] [pace]`
use acacia_bot::client::Client;
use acacia_bot::client::auth::login::Skin;
use acacia_bot::items::SlotRef;
use acacia_bot::state::Trackers;
use acacia_bot::{Bot, BotConfig};

type Error = Box<dyn std::error::Error>;

const COW: &str = "minecraft:cow";
/// The held banner's layers on white: pattern code and dye.
const BANNER: [(&str, &str); 2] = [("cr", "red_dye"), ("bo", "blue_dye")];
/// Ticks the pose is held for a screenshot.
const HOLD: u32 = 2400;
/// Ticks of each leg when it paces instead (some six blocks).
const PACE: u32 = 30;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let server = args.first().cloned().unwrap_or_else(|| "127.0.0.1:19174".into());
    let coordinate = |index: usize, default: i32| args.get(index).and_then(|a| a.parse().ok()).unwrap_or(default);
    let [x, y, z] = [coordinate(1, 240), coordinate(2, -60), coordinate(3, 104)];
    let pacing = args.get(4).is_some_and(|a| a == "pace");
    let trackers = Trackers { entities: true, ..Trackers::default() };
    let config = BotConfig { physics: true, trackers, ..BotConfig::default() };
    let mut bot = Bot::connect(Client::builder(&server).offline("Prop").skin(caped()), config).await?;
    while bot.movement().is_none_or(|m| !m.is_started()) {
        bot.wait_ticks(1).await?;
    }
    // A bot that died here last time respawns at the world spawn first.
    bot.wait_ticks(100).await?;
    let scene = [
        "/gamemode creative @s".to_owned(),
        format!("/tp @s {} {y} {} 180 0", x as f32 + 0.5, z as f32 + 0.5),
        format!("/kill @e[type=cow,x={x},y={y},z={z},r=40]"),
        "/clear @s".to_owned(),
        "/replaceitem entity @s slot.weapon.offhand 0 shield".to_owned(),
        "/replaceitem entity @s slot.hotbar 0 lead".to_owned(),
        format!("/summon cow {} {y} {}", x as f32 + 2.5, z as f32 + 0.5),
    ];
    for command in &scene {
        bot.client().command(command);
        bot.wait_ticks(6).await?;
    }
    bot.select_hotbar(0)?;
    bot.wait_ticks(20).await?;
    let off = |e: &acacia_bot::state::Entity| (e.position.x - (x as f32 + 2.5)).abs() + (e.position.z - (z as f32 + 0.5)).abs();
    let cows = bot.state().entities.iter().filter(|e| e.kind == COW);
    let cow = cows.min_by(|a, b| off(a).total_cmp(&off(b))).map(|e| e.runtime_id).ok_or("no cow was tracked")?;
    bot.interact_entity(cow).await?;
    bot.wait_ticks(10).await?;
    println!("leashed cow {cow}");
    banner(&mut bot, [x - 2, y, z]).await?;
    println!("holding {:?}", bot.state().inventory_summary());
    // Turned back to face north after aiming at the cow and the loom.
    bot.client().command(&format!("/tp @s {} {y} {} 180 0", x as f32 + 0.5, z as f32 + 0.5));
    bot.wait_ticks(10).await?;
    println!("posed");
    for leg in 0..HOLD / PACE {
        if let Some(controls) = bot.controls() {
            (controls.sneak, controls.forward) = (!pacing, if pacing { 1.0 } else { 0.0 });
            // East, then back west.
            controls.yaw = if !pacing { 180.0 } else if leg % 2 == 0 { -90.0 } else { 90.0 };
        }
        bot.wait_ticks(PACE).await?;
    }
    Ok(())
}

/// The flat skin with a red cape, gold across its top third.
fn caped() -> Skin {
    let texel = |index: u32| if index / 64 < 7 { [0xE0, 0xB0, 0x20, 0xFF] } else { [0xB0, 0x20, 0x20, 0xFF] };
    Skin::classic_with_cape(64, 32, &(0..64 * 32).flat_map(texel).collect::<Vec<u8>>())
}

/// Looms a patterned banner at a loom set at `loom` and holds it in the main hand: no command
/// makes one.
async fn banner(bot: &mut Bot, loom: [i32; 3]) -> Result<(), Error> {
    let setup = [format!("/setblock {} {} {} loom", loom[0], loom[1], loom[2]), "/clear @s".to_owned(), "/give @s banner 1 15".to_owned()];
    for command in &setup {
        bot.client().command(command);
        bot.wait_ticks(6).await?;
    }
    for (pattern, dye) in BANNER {
        bot.client().command(&format!("/give @s {dye}"));
        let (cloth, dye) = (find(bot, "banner").await?, find(bot, dye).await?);
        bot.loom(loom, cloth, dye, pattern, None).await?;
        bot.wait_ticks(10).await?;
    }
    let cloth = find(bot, "banner").await?;
    if cloth != SlotRef::Main(0) {
        bot.move_item(cloth, SlotRef::Main(0), 1).await?;
    }
    bot.select_hotbar(0)?;
    bot.client().command("/replaceitem entity @s slot.weapon.offhand 0 shield");
    Ok(bot.wait_ticks(10).await?)
}

/// The slot of item `name`, waiting up to three seconds for the server to hand it over.
async fn find(bot: &mut Bot, name: &str) -> Result<SlotRef, Error> {
    for _ in 0..60 {
        if let Some(slot) = bot.find_item(&format!("minecraft:{name}")) {
            return Ok(slot);
        }
        bot.wait_ticks(1).await?;
    }
    Err(format!("no {name} in the inventory: {:?}", bot.state().inventory_summary()).into())
}
