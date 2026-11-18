//! Signs. `sign`: clicks a sign two blocks ahead, facing the bot, and writes two lines on its front. `sign_place`:
//! places a sign from the hotbar, whose editor the server opens at once, and writes that. Both read the text back
//! with `tellraw`'s block NBT.

use acacia_bot::interact::Face;
use acacia_bot::signs::SignSide;
use acacia_bot::BotEvent;

use crate::drills::{check, start, Outcome};
use crate::{Drill, FLOOR};

pub const NAMES: &[&str] = &["sign", "sign_place"];

pub async fn run(name: &str, d: &mut Drill, x: i32) -> Outcome {
    let surface = FLOOR + 1;
    let pos = [x + 2, surface, 0];
    // Rotation 4 turns the front west, to the bot.
    let setup = if name == "sign" { format!("setblock {} {surface} 0 oak_sign[rotation=4]", pos[0]) } else { "give @s oak_sign".to_owned() };
    let base = start(d, x, &[&setup]).await?;
    if name == "sign_place" {
        d.bot.place_block([pos[0], FLOOR, 0], Face::Up).await.map_err(|e| format!("place: {e}"))?;
    }
    let written = d.bot.write_sign(pos, SignSide::Front, "drill-one\ndrill-two").await;
    d.ticks(10).await?;
    d.bot.command(&format!(r#"tellraw @s {{"nbt":"front_text.messages","block":"{} {} {}"}}"#, pos[0], pos[1], pos[2]));
    d.ticks(20).await?;
    let read = d.seen(|e| matches!(e, BotEvent::Chat(m) if m.message.contains("drill-one") && m.message.contains("drill-two")));
    Ok((base, check(written.is_ok() && read, || format!("write {written:?}, read back {read}"))))
}
