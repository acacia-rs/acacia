//! Builds a row of patterned banners on a flat-world server where the bot is operator, as a scene
//! for viewer screenshots: each banner is made at a loom and placed by hand, since no command
//! writes a banner's patterns. Standing ones go along x at the origin's z + 3, wall ones on a
//! stone wall east of them, all facing north.
//! `cargo run -p acacia-bot --example banners -- <server> [x y z of the scene's ground corner]`
use acacia_bot::client::Client;
use acacia_bot::interact::Face;
use acacia_bot::items::SlotRef;
use acacia_bot::{Bot, BotConfig};

type Error = Box<dyn std::error::Error>;

/// Item aux of the banner (0 black to 15 white) and its layers: pattern code, dye, pattern item.
type Spec = (u8, &'static [(&'static str, &'static str, Option<&'static str>)]);

const STANDING: [Spec; 5] = [
    (15, &[("tl", "blue_dye", None), ("bs", "green_dye", None), ("bo", "red_dye", None)]),
    (1, &[("ld", "yellow_dye", None), ("mc", "black_dye", None)]),
    (4, &[("gra", "white_dye", None), ("cr", "orange_dye", None)]),
    (11, &[("vh", "purple_dye", None), ("drs", "light_blue_dye", None), ("tts", "black_dye", None)]),
    (0, &[("cre", "lime_dye", Some("creeper_banner_pattern")), ("cbo", "pink_dye", Some("bordure_indented_banner_pattern"))]),
];
const WALL: [Spec; 3] = [
    (10, &[("hh", "white_dye", None), ("mr", "cyan_dye", None), ("bt", "magenta_dye", None)]),
    (14, &[("sc", "white_dye", None), ("bts", "brown_dye", None), ("glb", "blue_dye", Some("globe_banner_pattern"))]),
    (7, &[("ss", "yellow_dye", None), ("rud", "red_dye", None)]),
];
const OMINOUS_LOOT: &str = "entities/pillager_captain_equipment";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19176".into());
    let mut coordinate = |default: i32| args.next().and_then(|a| a.parse().ok()).unwrap_or(default);
    let [x, y, z] = [coordinate(5000), coordinate(-60), coordinate(5000)];
    let config = BotConfig { physics: true, ..BotConfig::default() };
    let mut bot = Bot::connect(Client::builder(&server).offline("BannerSmith"), config).await?;
    while bot.movement().is_none_or(|m| !m.is_started()) {
        bot.wait_ticks(1).await?;
    }
    let loom = [x - 2, y, z];
    let scene = [
        // A flat world's slimes shove and kill a survival bot at its loom, and its banner is gone with it.
        "/gamemode creative @s".to_owned(),
        format!("/kill @e[type=slime,x={x},y={y},z={z},r=40]"),
        format!("/tp @s {} {y} {}", x as f32 + 0.5, z as f32 + 0.5),
        format!("/fill {} {y} {} {} {} {} air", x - 3, z - 1, x + 18, y + 5, z + 6),
        format!("/fill {} {y} {} {} {} {} stone", x + 10, z + 5, x + 16, y + 3, z + 5),
        format!("/setblock {} {y} {} loom", loom[0], loom[2]),
    ];
    // A bot that died here last time respawns at the world spawn first.
    bot.wait_ticks(100).await?;
    run(&mut bot, &scene, 60).await?;
    for (index, spec) in STANDING.iter().enumerate() {
        let column = x + 2 * index as i32;
        // Off to a side by a different step each: the banner turns to face its placer.
        let stand = column as f32 + 0.5 + (index as f32 - 2.0) * 0.75;
        make(&mut bot, [x, y, z], loom, Some(spec)).await?;
        place(&mut bot, [stand, y as f32, z as f32 + 1.5], [column, y - 1, z + 3], Face::Up).await?;
    }
    for (index, spec) in WALL.iter().map(Some).chain([None]).enumerate() {
        let column = x + 10 + 2 * index as i32;
        make(&mut bot, [x, y, z], loom, spec).await?;
        place(&mut bot, [column as f32 + 0.5, y as f32, z as f32 + 2.5], [column, y + 2, z + 5], Face::North).await?;
    }
    run(&mut bot, &["/clear @s".to_owned()], 10).await?;
    for (position, nbt) in bot.state().block_entities.iter() {
        println!("{position:?}: {:?}", nbt.value);
    }
    Ok(())
}

async fn run(bot: &mut Bot, commands: &[String], settle: u32) -> Result<(), Error> {
    for command in commands {
        bot.client().command(command);
        bot.wait_ticks(4).await?;
    }
    Ok(bot.wait_ticks(settle).await?)
}

/// Leaves the inventory holding only the banner of `spec`, or the ominous banner without one.
async fn make(bot: &mut Bot, [x, y, z]: [i32; 3], loom: [i32; 3], spec: Option<&Spec>) -> Result<(), Error> {
    let home = format!("/tp @s {} {y} {} 90 0", x as f32 + 0.5, z as f32 + 0.5);
    let Some((aux, layers)) = spec else {
        return run(bot, &[home, "/clear @s".into(), format!("/loot give @s loot \"{OMINOUS_LOOT}\"")], 20).await;
    };
    run(bot, &[home, "/clear @s".into(), format!("/give @s banner 1 {aux}")], 20).await?;
    for (pattern, dye, item) in *layers {
        let gives: Vec<String> = [Some(*dye), *item].into_iter().flatten().map(|name| format!("/give @s {name}")).collect();
        run(bot, &gives, 10).await?;
        let (banner, dye) = (find(bot, "banner").await?, find(bot, dye).await?);
        let item = match item {
            Some(name) => Some(find(bot, name).await?),
            None => None,
        };
        bot.loom(loom, banner, dye, pattern, item).await?;
        bot.wait_ticks(10).await?;
    }
    Ok(())
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

/// Stands at `feet` and places the held banner against `face` of the block at `against`.
async fn place(bot: &mut Bot, feet: [f32; 3], against: [i32; 3], face: Face) -> Result<(), Error> {
    run(bot, &[format!("/tp @s {} {} {}", feet[0], feet[1], feet[2])], 20).await?;
    let banner = find(bot, "banner").await?;
    let hotbar = match banner {
        SlotRef::Main(slot) if slot < 9 => slot,
        other => {
            bot.move_item(other, SlotRef::Main(8), 1).await?;
            8
        }
    };
    bot.select_hotbar(hotbar)?;
    bot.wait_ticks(5).await?;
    println!("placing against {against:?} {face:?}: {:?}", bot.state().held_item().nbt.as_ref().map(|nbt| &nbt.value));
    bot.place_block(against, face).await?;
    Ok(bot.wait_ticks(10).await?)
}
