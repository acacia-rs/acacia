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
    let slot = bot.equip_best_tool(s.at("stone")).await?;
    match held_name(bot).as_str() {
        "minecraft:diamond_pickaxe" => Ok(format!("holding the diamond pickaxe from {slot:?}")),
        other => Err(format!("holding {other} ({slot:?})").into()),
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

/// Teleports up, opens the elytra, boosts and closes it again (physics bots).
pub async fn glide(bot: &mut Bot) -> Check {
    bot.equip(find(bot, "minecraft:firework_rocket")?, Destination::Hand).await?;
    bot.client().command("/tp @s ~ ~30 ~");
    bot.wait_ticks(15).await?;
    bot.start_gliding().await?;
    bot.boost_with_firework().await?;
    bot.wait_ticks(20).await?;
    bot.stop_gliding().await?;
    Ok("glided".into())
}
