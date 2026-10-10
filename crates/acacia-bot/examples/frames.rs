//! Hangs a row of item frames on a stone wall and fills them, on a flat-world server where the
//! bot is operator, as a scene for viewer screenshots: no command puts an item into a frame.
//! The frames face north along x from the origin; the last holds a map the bot fills there.
//! Three blocks north of the row one lies on the floor and one hangs under a stone.
//! `cargo run -p acacia-bot --example frames -- <server> [x y z of the first frame's block]`
use acacia_bot::client::Client;
use acacia_bot::interact::Face;
use acacia_bot::items::SlotRef;
use acacia_bot::{Bot, BotConfig};

type Error = Box<dyn std::error::Error>;

const ITEMS: [&str; 4] = ["diamond_sword", "apple", "grass_block", "empty_map"];
const FILLED: &str = "minecraft:filled_map";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let server = args.first().cloned().unwrap_or_else(|| "127.0.0.1:19174".into());
    let coordinate = |index: usize, default: i32| args.get(index).and_then(|a| a.parse().ok()).unwrap_or(default);
    let [x, y, z] = [coordinate(1, 260), coordinate(2, -59), coordinate(3, 104)];
    let config = BotConfig { physics: true, ..BotConfig::default() };
    let mut bot = Bot::connect(Client::builder(&server).offline("Framer"), config).await?;
    while bot.movement().is_none_or(|m| !m.is_started()) {
        bot.wait_ticks(1).await?;
    }
    // A bot that died here last time respawns at the world spawn first.
    bot.wait_ticks(100).await?;
    let last = x + 2 * (ITEMS.len() as i32 - 1);
    let scene = [
        "/gamemode creative @s".to_owned(),
        format!("/tp @s {} {} {} 0 0", x as f32 + 0.5, y - 1, z as f32 - 1.5),
        format!("/fill {} {} {} {} {} {} air", x - 2, y - 1, z - 4, last + 2, y + 2, z + 1),
        format!("/fill {} {} {} {} {} {} stone", x - 1, y - 1, z + 1, last + 1, y + 1, z + 1),
    ];
    run(&mut bot, &scene, 40).await?;
    for (index, item) in ITEMS.iter().enumerate() {
        let column = x + 2 * index as i32;
        let hang = [
            format!("/tp @s {} {} {} 0 0", column as f32 + 0.5, y - 1, z as f32 - 1.5),
            format!("/setblock {column} {y} {z} frame [\"facing_direction\"=2]"),
            "/clear @s".to_owned(),
            format!("/replaceitem entity @s slot.hotbar 0 {item}"),
        ];
        run(&mut bot, &hang, 20).await?;
        bot.select_hotbar(0)?;
        bot.wait_ticks(5).await?;
        if *item == "empty_map" {
            bot.use_item();
            bot.wait_ticks(30).await?;
            let filled = bot.find_item(FILLED).ok_or_else(|| format!("no filled map: {:?}", bot.state().inventory_summary()))?;
            if filled != SlotRef::Main(0) {
                bot.move_item(filled, SlotRef::Main(0), 1).await?;
            }
            bot.wait_ticks(5).await?;
        }
        bot.use_item_on_block([column, y, z], Face::North).await?;
        bot.wait_ticks(10).await?;
    }
    // One on the floor behind the bot and one under a stone over it, each with a sword.
    let lying = [(x, y - 1, 1, Face::Up), (x + 2, y + 1, 0, Face::Down)];
    run(&mut bot, &[format!("/setblock {} {} {} stone", x + 2, y + 2, z - 3)], 10).await?;
    for (column, level, facing, face) in lying {
        let hang = [
            format!("/tp @s {} {} {} 180 0", column as f32 + 0.5, y - 1, z as f32 - 1.5),
            format!("/setblock {column} {level} {} frame [\"facing_direction\"={facing}]", z - 3),
            "/clear @s".to_owned(),
            "/replaceitem entity @s slot.hotbar 0 diamond_sword".to_owned(),
        ];
        run(&mut bot, &hang, 20).await?;
        bot.select_hotbar(0)?;
        bot.wait_ticks(5).await?;
        bot.use_item_on_block([column, level, z - 3], face).await?;
        bot.wait_ticks(10).await?;
    }
    for (position, nbt) in bot.state().block_entities.iter() {
        println!("{position:?}: {:?}", nbt.value);
    }
    println!("framed");
    Ok(())
}

async fn run(bot: &mut Bot, commands: &[String], settle: u32) -> Result<(), Error> {
    for command in commands {
        bot.client().command(command);
        bot.wait_ticks(4).await?;
    }
    Ok(bot.wait_ticks(settle).await?)
}
