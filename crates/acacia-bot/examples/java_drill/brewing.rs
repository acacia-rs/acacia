//! `brew`: three water bottles and a nether wart at a brewing stand two blocks ahead, fuelled with blaze powder; the
//! bot puts them in by clicks, waits out the brew and takes the potions back.

use acacia_bot::items::SlotRef;

use crate::drills::{check, start, Outcome};
use crate::{Drill, FLOOR};

pub const NAMES: &[&str] = &["brew"];

fn count(d: &Drill, name: &str) -> u32 {
    let state = d.bot.state();
    let id = state.items.id(name);
    state.inventory.main.iter().filter(|s| !s.is_empty() && Some(s.network_id) == id).map(|s| u32::from(s.count)).sum()
}

pub async fn run(d: &mut Drill, x: i32) -> Outcome {
    let surface = FLOOR + 1;
    let stand = format!("setblock {} {surface} 0 brewing_stand", x + 2);
    let water = "give @s potion[potion_contents={potion:\"minecraft:water\"}] 3";
    let base = start(d, x, &[&stand, water, "give @s nether_wart", "give @s blaze_powder"]).await?;
    d.ticks(10).await?;
    let bottles = [SlotRef::Main(0), SlotRef::Main(1), SlotRef::Main(2)];
    let brewed = d.bot.brew([x + 2, surface, 0], &bottles, "minecraft:nether_wart").await;
    d.ticks(10).await?;
    let (potions, wart) = (count(d, "minecraft:potion"), count(d, "minecraft:nether_wart"));
    // A slot the server resends unchanged is a hash miss: the potion's `potion_contents` hashed wrong.
    let misses = d.bot.java().map_or(0, |j| j.client().inventory.resent);
    let ok = brewed.is_ok() && potions == 3 && wart == 0 && misses == 0;
    Ok((base, check(ok, || format!("brew {brewed:?}, {potions} potions back, {wart} nether wart left, {misses} hash misses"))))
}
