//! What BDS sends for an armor trim: its smithing-trim recipes (ingredient tags), then a real trim at a smithing
//! table and the NBT of the result.
//! `cargo run -p acacia-bot --example trim_probe -- <server> [name]`
mod support;

use acacia_bot::state::RecipeKind;
use support::Error;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19170".into());
    let name = args.next().unwrap_or_else(|| "Probe".into());
    let (mut bot, pad) = support::connect(&server, &name).await?;
    pad.build(&mut bot, &[], 5).await?;

    let trims: Vec<_> = bot.state().recipes.book().iter().filter(|r| r.kind == RecipeKind::SmithingTrim).collect();
    println!("{} smithing_trim recipes; first: {:?}", trims.len(), trims.first().map(|r| (&r.id, &r.block, &r.inputs)));
    for r in bot.state().recipes.book().iter().filter(|r| r.kind == RecipeKind::SmithingTransform).take(1) {
        println!("transform sample: {:?} {:?}", r.id, r.inputs);
    }

    let [x, y, z] = pad.block([5, 0, 0]);
    for cmd in [
        "/clear @s".to_string(),
        "/give @s sentry_armor_trim_smithing_template 1".into(),
        "/give @s iron_chestplate 1".into(),
        "/give @s copper_ingot 1".into(),
        format!("/setblock {x} {y} {z} smithing_table"),
    ] {
        bot.client().command(&cmd);
        bot.wait_ticks(4).await?;
    }
    pad.teleport(&mut bot, [5.5, 0.0, -1.5], 0.0).await?;
    bot.wait_ticks(20).await?;
    let find = |bot: &acacia_bot::Bot, n: &str| bot.find_item(n).ok_or_else(|| format!("no {n}"));
    let (base, add, template) = (find(&bot, "minecraft:iron_chestplate")?, find(&bot, "minecraft:copper_ingot")?, find(&bot, "minecraft:sentry_armor_trim_smithing_template")?);
    let result = bot.smith([x, y, z], base, add, template).await;
    println!("smith: {result:?}");
    bot.wait_ticks(20).await?;
    // The server's own stack, not acacia's prediction: what it tells a second player who sees this one wear it
    let watching = acacia_bot::BotConfig { trackers: acacia_bot::state::Trackers { entities: true, ..Default::default() }, ..Default::default() };
    let (mut observer, _) = support::connect_config(&server, "ProbeObs", watching).await?;
    let chest = bot.find_item("minecraft:iron_chestplate").ok_or("no chestplate")?;
    bot.equip(chest, acacia_bot::survival::Destination::Chest).await?;
    for _ in 0..40 {
        bot.wait_ticks(1).await?;
        observer.wait_ticks(1).await?;
    }
    let me = observer.state().player.position.clone();
    println!("observer tracks {} entities, {} players, me {:?}", observer.state().entities.iter().count(), observer.state().entities.players().count(), me);
    for e in observer.state().entities.players() {
        let worn = e.equipment.as_ref().map(|q| &q.armor[1]);
        println!("observer sees player at {:?}: chestplate {:?}, dist {}", e.position, worn.map(|s| (s.network_id, &s.nbt)), e.distance_sq(&me));
    }
    Ok(())
}
