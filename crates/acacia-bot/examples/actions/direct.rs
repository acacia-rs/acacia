//! The direct controls a player at the keyboard uses (`interact/direct.rs`): aim set by the caller,
//! mining advanced per tick, clicks sent at once.

use acacia_bot::interact::{facing_face, Face};
use acacia_bot::items::SlotRef;
use acacia_bot::Bot;

use crate::{find, Check, Pos};

const AIR: &str = "minecraft:air";

/// Two blocks east of the feet, at feet height (the flat world's ground is just below).
fn spot(bot: &Bot) -> Result<Pos, Box<dyn std::error::Error>> {
    let feet = bot.movement().and_then(|m| m.position()).ok_or("no simulated position")?;
    Ok([feet[0].floor() as i32 + 2, feet[1].floor() as i32, feet[2].floor() as i32])
}

fn aim(bot: &mut Bot, pos: Pos, face: Face) {
    let eye = bot.eye_position();
    let o = face.click_offset();
    let target = [pos[0] as f32 + o[0], pos[1] as f32 + o[1], pos[2] as f32 + o[2]];
    if let Some(c) = bot.controls() {
        c.look_at(eye, target);
    }
}

/// Holds attack on the block until it is gone: the server confirms every tick of the break time.
async fn hold_mine(bot: &mut Bot, pos: Pos) -> Result<u32, Box<dyn std::error::Error>> {
    let face = facing_face(bot.eye_position(), pos);
    aim(bot, pos, face);
    bot.wait_ticks(1).await?;
    for tick in 0..200 {
        if bot.block_name(pos) == Some(AIR) {
            return Ok(tick);
        }
        bot.start_mining(pos, face)?;
        bot.wait_ticks(1).await?;
    }
    bot.stop_mining();
    Err(format!("{:?} still at {pos:?} after 200 ticks", bot.block_name(pos)).into())
}

pub async fn mine_held(bot: &mut Bot) -> Check {
    let pos = spot(bot)?;
    bot.client().command(&format!("/setblock {} {} {} dirt", pos[0], pos[1], pos[2]));
    bot.wait_ticks(10).await?;
    let ticks = hold_mine(bot, pos).await?;
    Ok(format!("dirt broken by hand after {ticks} ticks"))
}

/// Creative breaks at once, with an empty hand (a held sword would break nothing). Needs
/// `MOUSE=1` (`BotConfig::mouse_input`): with touch input BDS mines creative blocks as survival.
pub async fn mine_creative(bot: &mut Bot) -> Check {
    let pos = spot(bot)?;
    bot.client().command("/replaceitem entity @s slot.hotbar 7 air");
    bot.client().command(&format!("/setblock {} {} {} stone", pos[0], pos[1], pos[2]));
    bot.client().command("/gamemode creative");
    bot.wait_ticks(10).await?;
    bot.select_hotbar(7)?;
    let result = hold_mine(bot, pos).await;
    bot.client().command("/gamemode survival");
    bot.wait_ticks(10).await?;
    let ticks = result?;
    if ticks > 3 {
        return Err(format!("creative took {ticks} ticks").into());
    }
    Ok(format!("stone gone {ticks} ticks into a creative break"))
}

pub async fn place_aimed(bot: &mut Bot) -> Check {
    let pos = spot(bot)?;
    bot.client().command(&format!("/setblock {} {} {} air", pos[0], pos[1], pos[2]));
    bot.client().command("/replaceitem entity @s slot.hotbar 8 cobblestone 4");
    bot.wait_ticks(10).await?;
    if find(bot, "minecraft:cobblestone")? != SlotRef::Main(8) {
        return Err("cobblestone is not in hotbar slot 8".into());
    }
    bot.select_hotbar(8)?;
    let below = [pos[0], pos[1] - 1, pos[2]];
    aim(bot, below, Face::Up);
    bot.wait_ticks(2).await?;
    bot.right_click_block(below, Face::Up)?;
    for tick in 0..20 {
        bot.wait_ticks(1).await?;
        if bot.block_name(pos) == Some("minecraft:cobblestone") {
            return Ok(format!("placed after {tick} ticks"));
        }
    }
    Err(format!("{:?} at {pos:?} after the click", bot.block_name(pos)).into())
}
