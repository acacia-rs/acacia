//! Java bot by eight workstations: renames a diamond sword "Blade" at the anvil at (2, -60, 2), strips an enchanted
//! iron sword at the grindstone at (-2, -60, 2), upgrades a diamond pickaxe at the smithing table at (2, -60, -2),
//! enchants an iron pickaxe (option 0) at the table at (-2, -60, -2), cuts 4 stone slabs at the stonecutter at
//! (0, -60, 3), borders a white banner red at the loom at (-3, -60, 0), makes a map from the 2 empty ones in hotbar
//! slot 0 and clones it at the cartography table at (3, -60, 0), and sets the beacon at (0, -60, -4) to speed for an
//! iron ingot. Give it those items, the netherite upgrade template and ingot, 3 lapis, some stone and some levels
//! first; it prints each step and the inventory.
//! `cargo run -p acacia-bot --features java --example java_stations -- <host> <port> [name]`

use acacia_bot::items::SlotRef;
use acacia_bot::workstation::BeaconEffect;
use acacia_bot::{Bot, BotConfig};
use acacia_world::neutral::Item;

fn describe(bot: &Bot, s: &acacia_bot::state::ItemStack) -> String {
    let name = bot.state().items.name(s.network_id).unwrap_or("?");
    let custom = s.custom_name.as_deref().map(|n| format!(" \"{n}\"")).unwrap_or_default();
    format!("{name}{custom} ench {:?}", s.enchantments())
}

/// The first main slot holding `name` (and enchanted, if `enchanted` says so).
fn slot(bot: &Bot, name: &str, enchanted: Option<bool>) -> Option<SlotRef> {
    let state = bot.state();
    let fits = |s: &acacia_bot::state::ItemStack| state.items.name(s.network_id) == Some(name) && enchanted.is_none_or(|e| e == !s.enchantments().is_empty());
    let i = state.inventory.main.iter().position(fits)?;
    Some(SlotRef::Main(u8::try_from(i).ok()?))
}

fn of(bot: &Bot, item: Item) -> Option<SlotRef> {
    let i = bot.state().inventory.main.iter().position(|s| !s.is_empty() && bot.state().item(s) == Some(item))?;
    Some(SlotRef::Main(u8::try_from(i).ok()?))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [host, port] = &args[..2] else { return Err("usage: java_stations <host> <port> [name]".into()) };
    let name = args.get(2).map_or("Smith", String::as_str);
    let mut bot = Bot::connect_java_offline(host, port.parse()?, name, BotConfig::default()).await?;
    bot.wait_ticks(60).await?;

    let sword = slot(&bot, "minecraft:diamond_sword", None).ok_or("no diamond sword")?;
    println!("anvil {:?}", bot.anvil([2, -60, 2], sword, None, Some("Blade")).await);
    let enchanted = slot(&bot, "minecraft:iron_sword", Some(true)).ok_or("no enchanted iron sword")?;
    println!("grindstone {:?}", bot.grindstone([-2, -60, 2], enchanted, None).await);
    let (template, ingot, pickaxe) = (
        slot(&bot, "minecraft:netherite_upgrade_smithing_template", None).ok_or("no template")?,
        slot(&bot, "minecraft:netherite_ingot", None).ok_or("no ingot")?,
        slot(&bot, "minecraft:diamond_pickaxe", None).ok_or("no diamond pickaxe")?,
    );
    println!("smithing {:?}", bot.smith([2, -60, -2], pickaxe, ingot, template).await);
    let plain = slot(&bot, "minecraft:iron_pickaxe", Some(false)).ok_or("no iron pickaxe")?;
    println!("enchant {:?}", bot.enchant([-2, -60, -2], plain, 0).await);
    println!("stonecut {:?}", bot.stonecut([0, -60, 3], Item::StoneSlab, 4).await);
    let (banner, dye) = (of(&bot, Item::WhiteBanner).ok_or("no banner")?, of(&bot, Item::RedDye).ok_or("no red dye")?);
    println!("loom {:?}", bot.loom([-3, -60, 0], banner, dye, "bo", None).await);
    bot.select_hotbar(0)?;
    println!("make map {:?}", bot.make_map().await);
    let (filled, empty) = (of(&bot, Item::FilledMap).ok_or("no filled map")?, of(&bot, Item::Map).ok_or("no empty map")?);
    println!("cartography {:?}", bot.cartography([3, -60, 0], filled, Some(empty), None, 1).await);
    let iron = of(&bot, Item::IronIngot).ok_or("no iron ingot")?;
    println!("beacon {:?}", bot.beacon([0, -60, -4], BeaconEffect::Speed, None, iron).await);
    bot.wait_ticks(10).await?;
    for s in bot.state().inventory.main.iter().filter(|s| !s.is_empty()) {
        println!("  {}", describe(&bot, s));
    }
    bot.disconnect().await;
    Ok(())
}
