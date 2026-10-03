//! Eating, equipment, signs, books, beds, riding, fishing and elytra checks.
use std::time::Duration;

use acacia_bot::Bot;
use acacia_bot::items::SlotRef;
use acacia_bot::signs::SignSide;
use acacia_bot::survival::Destination;

use crate::{count, find, stack, wait_chat, Check, Scene};

fn held_name(bot: &Bot) -> String {
    bot.state().item_name(bot.state().held_item()).unwrap_or("nothing").to_owned()
}

pub async fn consume(bot: &mut Bot) -> Check {
    bot.equip(find(bot, "minecraft:bread")?, Destination::Hand).await?;
    let (bread, hunger) = (count(bot, "minecraft:bread"), bot.state().player.hunger);
    bot.consume().await?;
    let after = (count(bot, "minecraft:bread"), bot.state().player.hunger);
    // Long enough for a use restarted by the finishing click to complete as well.
    bot.wait_ticks(50).await?;
    if after.0 + 1 != bread || count(bot, "minecraft:bread") != after.0 {
        return Err(format!("bread {bread} -> {} -> {}, hunger {hunger} -> {}", after.0, count(bot, "minecraft:bread"), after.1).into());
    }
    Ok(format!("hunger {hunger} -> {}", after.1))
}

pub async fn equip(bot: &mut Bot) -> Check {
    bot.equip(find(bot, "minecraft:iron_helmet")?, Destination::Head).await?;
    bot.equip(find(bot, "minecraft:iron_boots")?, Destination::Feet).await?;
    bot.wait_ticks(10).await?;
    let worn = |slot| stack(bot, SlotRef::Armor(slot)).and_then(|s| bot.state().item_name(s)).unwrap_or("nothing").to_owned();
    let (head, feet) = (worn(0), worn(3));
    if head != "minecraft:iron_helmet" || feet != "minecraft:iron_boots" {
        return Err(format!("wearing {head} and {feet}").into());
    }
    Ok("helmet and boots worn".into())
}

pub async fn best_tool(bot: &mut Bot, s: &Scene) -> Check {
    let before = (bot.best_tool_for(s.at("stone")), bot.state().inventory.selected_hotbar_slot);
    let slot = bot.equip_best_tool(s.at("stone")).await?;
    match held_name(bot).as_str() {
        "minecraft:diamond_pickaxe" => Ok(format!("holding the diamond pickaxe from {slot:?}")),
        other => {
            let picks: Vec<_> = ["minecraft:diamond_pickaxe", "minecraft:iron_pickaxe"]
                .map(|p| (p, bot.find_item(p), bot.find_item(p).and_then(|s| stack(bot, s)).map(|s| s.nbt.clone())))
                .into();
            let server = crate::server_inv(bot, "").await?;
            let server: Vec<&str> = server.split("},{").filter(|s| s.contains("pickaxe")).collect();
            let why = format!("(best, held) before {before:?}, best now {:?}; bot sees {picks:?}; server {server:?}", bot.best_tool_for(s.at("stone")));
            Err(format!("holding {other} ({slot:?}); {why}").into())
        }
    }
}

pub async fn sign(bot: &mut Bot, s: &Scene) -> Check {
    bot.write_sign(s.at("sign"), SignSide::Front, "Hello\nfrom bot").await?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while let Some(text) = wait_chat(bot, "sign", deadline - tokio::time::Instant::now()).await {
        if text.contains("Hello") {
            return Ok(format!("server sees {text}"));
        }
    }
    Err("the server never showed the text".into())
}

pub async fn book(bot: &mut Bot) -> Check {
    bot.equip(find(bot, "minecraft:writable_book")?, Destination::Hand).await?;
    bot.write_book(&["Page one", "Page two"]).await?;
    bot.wait_ticks(10).await?;
    if let Err(e) = bot.sign_book("Bot book").await {
        let server = crate::server_inv(bot, "").await?;
        let books: Vec<&str> = server.split("},{").filter(|s| s.contains("book")).collect();
        return Err(format!("{e}; server has {books:?}").into());
    }
    match count(bot, "minecraft:written_book") {
        1 => Ok(format!("signed; bot predicts {:?}", bot.state().held_item().nbt)),
        n => Err(format!("{n} written books").into()),
    }
}

fn corrections(bot: &Bot) -> u32 {
    bot.movement().map_or(0, |m| m.corrections)
}

/// Simulated feet of a physics bot, the reported ones of an idle bot.
fn feet(bot: &Bot) -> Option<[f32; 3]> {
    match bot.movement() {
        Some(m) => m.position(),
        None => {
            let p = &bot.state().player.position;
            Some([p.x, p.y, p.z])
        }
    }
}

pub async fn sleep(bot: &mut Bot, s: &Scene) -> Check {
    if let Err(e) = bot.sleep(s.at("bed")).await {
        return Err(format!("{e}; chat: {:?}", crate::recent_chat(bot).await).into());
    }
    let asleep = corrections(bot);
    bot.wait_ticks(40).await?;
    if !bot.is_sleeping() {
        return Err("woke up on its own".into());
    }
    let in_bed = corrections(bot) - asleep;
    bot.wake().await?;
    bot.wait_ticks(20).await?;
    let woke = corrections(bot) - asleep - in_bed;
    let detail = format!("{in_bed} corrections in 40 ticks asleep, {woke} after waking; feet {:?}", feet(bot));
    match in_bed {
        0 => Ok(format!("slept and woke; {detail}")),
        _ => Err(detail.into()),
    }
}

pub async fn ride(bot: &mut Bot, kind: &str) -> Check {
    let me = bot.state().player.position.clone();
    let id = bot.state().entities.nearest(&me, |e| e.kind == kind).map(|e| e.runtime_id).ok_or(format!("no {kind} tracked"))?;
    let start = (feet(bot), bot.state().entities.get(id).map(|e| e.position.clone()));
    let vehicle = match bot.mount(id).await {
        Ok(v) => v,
        Err(e) => {
            let eye = bot.eye_position();
            let mine = format!("eye {eye:?} facing {:?}; (bot feet, vehicle) {start:?} now {:?}", bot.facing(), bot.state().entities.get(id));
            return Err(format!("{e}; {mine}; server sees {}", crate::server_entities(bot, kind).await).into());
        }
    };
    bot.wait_ticks(20).await?;
    if bot.vehicle().is_none() {
        return Err("thrown off the vehicle".into());
    }
    let seated = corrections(bot);
    let seat_feet = feet(bot);
    bot.dismount().await?;
    // Physics bots resume at the exit a tick or two later; idle bots are there at once.
    let mut stepped = feet(bot);
    for _ in 0..5 {
        if stepped != seat_feet {
            break;
        }
        bot.wait_ticks(1).await?;
        stepped = feet(bot);
    }
    bot.wait_ticks(25).await?;
    if let Some(v) = bot.vehicle() {
        return Err(format!("still riding {v:?}").into());
    }
    let server = crate::chat_lines(bot, "dismounted").await;
    let end = (feet(bot), bot.state().entities.get(id).map(|e| e.position.clone()));
    let detail = format!(
        "rode {vehicle:?}; (bot feet, vehicle) before {start:?} stepped off at {stepped:?} after {end:?}; server off the seat {server:?}; {} corrections",
        corrections(bot) - seated
    );
    // The server pushes the player off the (pushed) vehicle's box after placing it: compare where the
    // bot stepped off with each of the server's first positions.
    let spots: Vec<[f32; 3]> = server.iter().filter_map(|l| crate::parse_pos(l.rsplit(' ').next()?)).collect();
    match stepped {
        Some(f) if !spots.is_empty() && !spots.iter().any(|s| (0..3).all(|i| (f[i] - s[i]).abs() <= 0.05)) => {
            Err(format!("left at the wrong spot: {detail}").into())
        }
        _ => Ok(detail),
    }
}

pub async fn fish(bot: &mut Bot, s: &Scene) -> Check {
    // Earlier checks (bed, rides) leave the bot elsewhere; cast from the spot the scene was built for.
    let [bx, by, bz] = s.at("base");
    bot.client().command(&format!("/tp @s {bx}.5 {by} {bz}.5"));
    bot.wait_ticks(20).await?;
    bot.equip(find(bot, "minecraft:fishing_rod")?, Destination::Hand).await?;
    let [x, _, z] = s.at("water");
    let eye = bot.eye_position();
    // Level: a cast aimed down at the pool falls short of it.
    bot.look_at([x as f32 + 0.5, eye[1], z as f32 + 0.5]).await?;
    bot.wait_ticks(3).await?;
    match bot.fish(Duration::from_secs(60)).await? {
        Some(item) => Ok(format!("caught {:?} x{}", bot.state().item_name(&item), item.count)),
        None => Err(format!("nothing caught; server inventory {}", crate::server_inv(bot, "").await?).into()),
    }
}

pub async fn elytra(bot: &mut Bot) -> Check {
    bot.equip_elytra().await?;
    bot.wait_ticks(10).await?;
    if !bot.wears_elytra() {
        return Err("no elytra worn".into());
    }
    Ok("worn".into())
}

/// Teleports up, opens the elytra, glides level, boosts, climbs while turning, then dives until the
/// landing ends the glide (physics bots). Passes with no server corrections from the start to the landing.
pub async fn glide(bot: &mut Bot) -> Check {
    bot.equip(find(bot, "minecraft:firework_rocket")?, Destination::Hand).await?;
    bot.client().command("/tp @s ~ ~30 ~ -90 0");
    bot.wait_ticks(15).await?;
    bot.start_gliding().await?;
    let before = corrections(bot);
    bot.wait_ticks(20).await?;
    bot.boost_with_firework().await?;
    bot.wait_ticks(15).await?;
    for (yaw, pitch, ticks) in [(-60.0, -20.0, 12), (-75.0, 30.0, 0)] {
        if let Some(c) = bot.controls() {
            (c.yaw, c.pitch) = (yaw, pitch);
        }
        bot.wait_ticks(ticks).await?;
    }
    glide_until_landed(bot, before, "glided").await
}

/// Dives from above the scene's pool into the water, which ends the glide (physics bots).
pub async fn glide_water(bot: &mut Bot, s: &Scene) -> Check {
    let [x, y, z] = s.at("water");
    bot.client().command(&format!("/tp @s {} {} {} -90 0", x - 2, y + 14, z));
    bot.wait_ticks(15).await?;
    bot.start_gliding().await?;
    let before = corrections(bot);
    if let Some(c) = bot.controls() {
        c.pitch = 70.0;
    }
    glide_until_landed(bot, before, "glided into the water").await
}

/// Rides the scene's tamed, saddled horse: forward, a 90° turn, strafing, backing, stopping (physics
/// bots). Passes with no server corrections of the horse.
pub async fn ride_horse(bot: &mut Bot, s: &Scene) -> Check {
    let [x, y, z] = s.at("horse");
    bot.client().command(&format!("/tp @s {} {y} {} 90 0", x as f32 + 2.5, z as f32 + 0.5));
    bot.wait_ticks(15).await?;
    let me = bot.state().player.position.clone();
    let id = bot.state().entities.nearest(&me, |e| e.kind == "minecraft:horse").map(|e| e.runtime_id).ok_or("no horse tracked")?;
    bot.mount(id).await?;
    bot.wait_ticks(5).await?;
    // The first input after the mount may correct a horse that wandered since the bot last saw it.
    bot.wait_ticks(5).await?;
    let mut phases = Vec::new();
    let phase_list = [
        ("west", 1.0, 0.0, 90.0, 40),
        ("turn", 1.0, 0.0, 180.0, 30),
        ("strafe", 0.0, 1.0, 180.0, 15),
        ("back", -1.0, 0.0, 180.0, 15),
        ("stop", 0.0, 0.0, 180.0, 10),
    ];
    for (name, forward, strafe, yaw, ticks) in phase_list {
        let before = vehicle_corrections(bot);
        if let Some(c) = bot.controls() {
            (c.forward, c.strafe, c.yaw) = (forward, strafe, yaw);
        }
        bot.wait_ticks(ticks).await?;
        phases.push((name, vehicle_corrections(bot) - before));
    }
    if let Some(c) = bot.controls() {
        c.stop();
    }
    let speed = bot.vehicle().and_then(|v| v.runtime_id).and_then(|id| bot.state().entities.get(id)).and_then(|e| e.movement);
    bot.dismount().await?;
    let corrections: u32 = phases.iter().map(|(_, n)| n).sum();
    let detail = format!("horse corrections per phase {phases:?}; speed attribute {speed:?}");
    if corrections == 0 { Ok(format!("rode; {detail}")) } else { Err(detail.into()) }
}

fn vehicle_corrections(bot: &Bot) -> u32 {
    bot.movement().map_or(0, |m| m.vehicle_corrections)
}

/// Waits for the glide to end by itself, then checks the server never corrected the bot after `before`.
async fn glide_until_landed(bot: &mut Bot, before: u32, done: &str) -> Check {
    let mut ticks = 0;
    while bot.is_gliding() && ticks < 400 {
        bot.wait_ticks(1).await?;
        ticks += 1;
    }
    bot.wait_ticks(10).await?;
    let detail = format!("{} corrections while gliding; landed after {ticks} ticks at {:?}", corrections(bot) - before, feet(bot));
    match (bot.is_gliding(), corrections(bot) - before) {
        (true, _) => Err(format!("still gliding; {detail}").into()),
        (false, 0) => Ok(format!("{done}; {detail}")),
        _ => Err(detail.into()),
    }
}
