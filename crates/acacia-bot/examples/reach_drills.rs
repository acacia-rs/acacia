//! What BDS accepts with no line of sight and at the edge of reach, judged by server state (`/execute if
//! block`, `EntityEvent`, `UpdateBlock`), never by the bot's prediction. Clicks go past the library's own reach
//! guards on purpose. Static reading and results: docs/confirmed-actions.md "Reach and line of sight".
//! `cargo run --release -p acacia-bot --example reach_drills -- <server> <name> [drill,...]`
//! Drills: start_game, wall_place, wall_break, corner, corner_creative, entity, library_attack (default: all).
mod support;

use std::time::Duration;

use acacia_bot::client::proto::packets::{EntityEvent, EntityEventEventId, InventoryTransaction, StartGame, UpdateBlock};
use acacia_bot::client::proto::types::{
    Transaction, TransactionLegacy, TransactionTransactionData, TransactionTransactionDataItemUseOnEntity,
    TransactionTransactionDataItemUseOnEntityActionType, TransactionTransactionType, Vec3f,
};
use acacia_bot::client::proto::Packet;
use acacia_bot::client::PacketFilter;
use acacia_bot::interact::{to_wire, Face};
use acacia_bot::{Bot, BotConfig, BotEvent, Events};
use support::{Error, Fill, Pad};

type Check = Result<String, Error>;

const EYE: f32 = 1.62;
/// Hidden blocks sit behind this stone wall, two blocks east of the bot.
const WALL: Fill<'static> = ([2, 0, -1], [2, 2, 1], "stone");

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}

fn corner(p: [i32; 3]) -> [f32; 3] {
    p.map(|v| v as f32)
}

/// Every event that arrives within `ticks`.
async fn collect(bot: &mut Bot, ticks: u32) -> Result<Vec<BotEvent>, Error> {
    bot.wait_ticks(ticks).await?;
    let mut events = Vec::new();
    while let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(100), bot.next()).await {
        events.push(event);
    }
    Ok(events)
}

/// The server's block at `pos` is `block`: `/execute` answers with a `say` the bot hears.
async fn server_block_is(bot: &mut Bot, pad: &Pad, pos: [i32; 3], block: &str) -> Result<bool, Error> {
    collect(bot, 1).await?;
    let [x, y, z] = pos;
    pad.run(bot, &format!("/execute if block {x} {y} {z} {block} run say RAYCAST is yes"));
    pad.run(bot, &format!("/execute unless block {x} {y} {z} {block} run say RAYCAST is no"));
    for event in collect(bot, 10).await? {
        if let BotEvent::Chat(m) = event {
            let text = m.plain();
            if text.contains("RAYCAST is yes") {
                return Ok(true);
            }
            if text.contains("RAYCAST is no") {
                return Ok(false);
            }
        }
    }
    Err(format!("no /execute answer for {pos:?}").into())
}

/// `UpdateBlock`s within two blocks of `near`.
fn resends(events: &[BotEvent], near: [i32; 3]) -> usize {
    events
        .iter()
        .filter_map(|e| match e {
            BotEvent::Packet(p) if p.id == UpdateBlock::ID => p.decode::<UpdateBlock>().ok(),
            _ => None,
        })
        .filter(|u| [u.position.x - near[0], u.position.y - near[1], u.position.z - near[2]].iter().map(|d| d.abs()).sum::<i32>() <= 2)
        .count()
}

fn aim(bot: &mut Bot, at: [f32; 3]) {
    let eye = bot.eye_position();
    if let Some(c) = bot.controls() {
        c.look_at(eye, at);
    }
}

async fn hold(bot: &mut Bot, pad: &Pad, slot: u8, item: &str) -> Result<(), Error> {
    pad.run(bot, &format!("/replaceitem entity @s slot.hotbar {slot} {item}"));
    bot.wait_ticks(10).await?;
    bot.select_hotbar(slot)?;
    Ok(bot.wait_ticks(2).await?)
}

/// Places onto the far face of a stone behind a wall: a face no ray from the eye reaches.
async fn wall_place(bot: &mut Bot, pad: &Pad) -> Check {
    pad.build(bot, &[WALL, ([3, 0, 0], [3, 0, 0], "stone")], 0).await?;
    pad.teleport(bot, [0.5, 0.0, 0.5], -90.0).await?;
    hold(bot, pad, 8, "cobblestone 16").await?;
    let (target, into) = (pad.block([3, 0, 0]), pad.block([4, 0, 0]));
    aim(bot, [target[0] as f32 + 1.0, target[1] as f32 + 0.5, target[2] as f32 + 0.5]);
    bot.wait_ticks(2).await?;
    bot.right_click_block(target, Face::East)?;
    bot.wait_ticks(10).await?;
    let placed = server_block_is(bot, pad, into, "cobblestone").await?;
    let detail = format!("hidden East face of {target:?} from eye {:?}: placed={placed}", bot.eye_position());
    if placed { Ok(detail) } else { Err(detail.into()) }
}

/// Mines a stone at eye height behind a wall (StartBreak, ContinueBreak..., PredictDestroy, AbortBreak).
async fn wall_break(bot: &mut Bot, pad: &Pad) -> Check {
    pad.build(bot, &[WALL, ([3, 1, 0], [3, 1, 0], "stone")], 0).await?;
    pad.teleport(bot, [0.5, 0.0, 0.5], -90.0).await?;
    hold(bot, pad, 7, "netherite_pickaxe").await?;
    let target = pad.block([3, 1, 0]);
    aim(bot, [target[0] as f32, target[1] as f32 + 0.5, target[2] as f32 + 0.5]);
    bot.wait_ticks(2).await?;
    let mut ticks = 0;
    while ticks < 100 && bot.block_name(target) != Some("minecraft:air") {
        bot.start_mining(target, Face::West)?;
        bot.wait_ticks(1).await?;
        ticks += 1;
    }
    bot.stop_mining();
    bot.wait_ticks(10).await?;
    let gone = server_block_is(bot, pad, target, "air").await?;
    let detail = format!("hidden stone {target:?}: server air={gone} after {ticks} ticks");
    if gone { Ok(detail) } else { Err(detail.into()) }
}

/// Places against a floating stone with the eye `d` from its integer corner; `UpdateBlock`s near it are counted.
async fn place_at(bot: &mut Bot, pad: &Pad, x: i32, d: f32) -> Result<(bool, String), Error> {
    let against = [x, 1, 0];
    pad.build(bot, &[(against, against, "stone")], 0).await?;
    // Eye 0.62 above and 0.5 beside the corner (feet on the pad, z 0.5).
    let feet_x = x as f32 - (d * d - (EYE - 1.0).powi(2) - 0.25).sqrt();
    pad.teleport(bot, [feet_x, 0.0, 0.5], -90.0).await?;
    hold(bot, pad, 8, "cobblestone 16").await?;
    let (pos, into) = (pad.block(against), pad.block([x - 1, 1, 0]));
    let eye = bot.eye_position();
    let centre = corner(pos).map(|v| v + 0.5);
    aim(bot, [pos[0] as f32, pos[1] as f32 + 0.5, pos[2] as f32 + 0.5]);
    collect(bot, 2).await?;
    bot.right_click_block(pos, Face::West)?;
    let events = collect(bot, 10).await?;
    let placed = server_block_is(bot, pad, into, "cobblestone").await?;
    let line = format!(
        "corner {:.3} centre {:.3}: placed={placed} UpdateBlocks near={}",
        dist(eye, corner(pos)),
        dist(eye, centre),
        resends(&events, pos)
    );
    Ok((placed, line))
}

/// Inside the limit (7.2 survival, 12.5 creative, to the corner) places; past it is refused.
async fn corner_drill(bot: &mut Bot, pad: &Pad, creative: bool) -> Check {
    let (x, inside, outside) = if creative { (14, 12.4, 12.6) } else { (8, 7.1, 7.3) };
    if creative {
        pad.run(bot, "/gamemode creative");
    }
    let near = place_at(bot, pad, x, inside).await;
    let far = place_at(bot, pad, x, outside).await;
    if creative {
        pad.run(bot, "/gamemode survival");
        bot.wait_ticks(5).await?;
    }
    let ((near_ok, near), (far_ok, far)) = (near?, far?);
    let detail = format!("{near} | {far}");
    if near_ok && !far_ok { Ok(detail) } else { Err(detail.into()) }
}

fn attack(bot: &Bot, runtime_id: u64, click: [f32; 3]) -> InventoryTransaction {
    let inv = &bot.state().inventory;
    let v = |[x, y, z]: [f32; 3]| Vec3f { x, y, z };
    InventoryTransaction {
        transaction: Transaction {
            legacy: TransactionLegacy { legacy_request_id: 0, legacy_transactions: None },
            transaction_type: TransactionTransactionType::ItemUseOnEntity,
            actions: Vec::new(),
            transaction_data: TransactionTransactionData::ItemUseOnEntity(TransactionTransactionDataItemUseOnEntity {
                entity_runtime_id: runtime_id,
                action_type: TransactionTransactionDataItemUseOnEntityActionType::Attack,
                hotbar_slot: i32::from(inv.selected_hotbar_slot),
                held_item: to_wire(inv.held()),
                player_pos: v(bot.eye_position()),
                click_pos: v(click),
            }),
        },
    }
}

/// Hits a penned cow with the eye `d` from its feet; the server's hurt event says whether it landed.
async fn hit_at(bot: &mut Bot, pad: &Pad, d: f32) -> Result<(bool, String), Error> {
    let cow = bot.state().entities.iter().find(|e| e.kind == "minecraft:cow").map(|e| (e.runtime_id, e.feet())).ok_or("no cow tracked")?;
    let (id, feet) = cow;
    let origin = corner(pad.block([0, 0, 0]));
    let feet_x = feet.x - (d * d - EYE * EYE).sqrt();
    pad.teleport(bot, [feet_x - origin[0], 0.0, feet.z - origin[2]], -90.0).await?;
    let feet = bot.state().entities.get(id).map(|e| e.feet()).ok_or("the cow is gone")?;
    let target = [feet.x, feet.y, feet.z];
    let click = [feet.x, feet.y + 0.6, feet.z];
    aim(bot, click);
    collect(bot, 2).await?;
    let eye = bot.eye_position();
    bot.client().send(&attack(bot, id, click));
    let events = collect(bot, 10).await?;
    let hurt = events.iter().any(|e| match e {
        BotEvent::Packet(p) if p.id == EntityEvent::ID => {
            p.decode::<EntityEvent>().is_ok_and(|ev| ev.runtime_entity_id == id && ev.event_id == EntityEventEventId::HurtAnimation)
        }
        _ => false,
    });
    let health = bot.state().entities.get(id).and_then(|e| e.health());
    Ok((hurt, format!("eye to feet {:.3}: hurt={hurt} health={health:?}", dist(eye, target))))
}

/// Inside 6.7 from the eye to the target's position lands; past it is dropped silently.
async fn entity(bot: &mut Bot, pad: &Pad) -> Check {
    let pen: [Fill; 2] = [([9, 0, -1], [11, 1, 1], "glass"), ([10, 0, 0], [10, 1, 0], "air")];
    pad.build(bot, &pen, 0).await?;
    let [x, y, z] = pad.block([10, 0, 0]);
    pad.run(bot, &format!("/summon cow {} {y} {}", x as f32 + 0.5, z as f32 + 0.5));
    hold(bot, pad, 6, "wooden_sword").await?;
    bot.wait_ticks(20).await?;
    let (near_ok, near) = hit_at(bot, pad, 6.4).await?;
    bot.wait_ticks(20).await?;
    let (far_ok, far) = hit_at(bot, pad, 7.0).await?;
    let detail = format!("{near} | {far}");
    if near_ok && !far_ok { Ok(detail) } else { Err(detail.into()) }
}

/// `Bot::attack` end to end: an open hit lands (aim, pause, CPS gate), one through the pen's glass is refused
/// `NotVisible` before anything is sent.
async fn library_attack(bot: &mut Bot, pad: &Pad) -> Check {
    let pen: [Fill; 2] = [([5, 0, -1], [7, 1, 1], "glass"), ([6, 0, 0], [6, 1, 0], "air")];
    pad.build(bot, &pen, 0).await?;
    let [x, y, z] = pad.block([6, 0, 0]);
    pad.run(bot, &format!("/summon cow {} {y} {}", x as f32 + 0.5, z as f32 + 0.5));
    hold(bot, pad, 6, "wooden_sword").await?;
    bot.wait_ticks(20).await?;
    let id = bot.state().entities.iter().find(|e| e.kind == "minecraft:cow").map(|e| e.runtime_id).ok_or("no cow tracked")?;
    pad.teleport(bot, [4.0, 0.0, 0.5], -90.0).await?;
    collect(bot, 2).await?;
    let hidden = bot.attack(id).await;
    let quiet = collect(bot, 10).await?.iter().all(|e| !matches!(e, BotEvent::Packet(p) if p.id == EntityEvent::ID));
    pad.run(bot, &format!("/setblock {} {} {} air", x - 1, y + 1, z));
    pad.run(bot, &format!("/setblock {} {y} {z} air", x - 1));
    collect(bot, 5).await?;
    let open = bot.attack(id).await;
    let hurt = collect(bot, 10).await?.iter().any(|e| match e {
        BotEvent::Packet(p) if p.id == EntityEvent::ID => {
            p.decode::<EntityEvent>().is_ok_and(|ev| ev.runtime_entity_id == id && ev.event_id == EntityEventEventId::HurtAnimation)
        }
        _ => false,
    });
    let detail = format!("behind glass: {hidden:?}, no hurt={quiet} | open: {open:?}, hurt={hurt}");
    let refused = matches!(hidden, Err(acacia_bot::ActionError::NotVisible(_)));
    if refused && quiet && open.is_ok() && hurt { Ok(detail) } else { Err(detail.into()) }
}

fn start_game(events: &[BotEvent]) -> Check {
    let packet = events
        .iter()
        .find_map(|e| match e {
            BotEvent::Packet(p) if p.id == StartGame::ID => Some(p),
            _ => None,
        })
        .ok_or("no StartGame among the queued events")?;
    let start: StartGame = packet.decode()?;
    Ok(format!("server_authoritative_block_breaking={}", start.server_authoritative_block_breaking))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    support::watchdog(600);
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19230".into());
    let name = args.next().unwrap_or_else(|| "RayBot".into());
    let only = args.next();
    let wanted = |d: &str| only.as_deref().is_none_or(|o| o.split(',').any(|n| n == d));
    let subscribe = PacketFilter::none().with(StartGame::ID).with(UpdateBlock::ID).with(EntityEvent::ID);
    let config = BotConfig { events: Events::CHAT, subscribe, trackers: acacia_bot::state::Trackers { entities: true, ..Default::default() }, ..BotConfig::default() };
    let (mut bot, pad) = support::connect_config(&server, &name, config).await?;
    let early = collect(&mut bot, 1).await?;
    pad.run(&bot, "/gamemode survival");

    let mut results: Vec<(&str, Check)> = Vec::new();
    if wanted("start_game") {
        results.push(("start_game", start_game(&early)));
    }
    if wanted("wall_place") {
        results.push(("wall_place", wall_place(&mut bot, &pad).await));
    }
    if wanted("wall_break") {
        results.push(("wall_break", wall_break(&mut bot, &pad).await));
    }
    if wanted("corner") {
        results.push(("corner", corner_drill(&mut bot, &pad, false).await));
    }
    if wanted("corner_creative") {
        results.push(("corner_creative", corner_drill(&mut bot, &pad, true).await));
    }
    if wanted("entity") {
        results.push(("entity", entity(&mut bot, &pad).await));
    }
    if wanted("library_attack") {
        results.push(("library_attack", library_attack(&mut bot, &pad).await));
    }
    for (name, result) in &results {
        match result {
            Ok(detail) => println!("PASS {name:<16} {detail}"),
            Err(e) => println!("FAIL {name:<16} {e}"),
        }
    }
    pad.go_home(&mut bot).await?;
    bot.disconnect().await;
    Ok(())
}
