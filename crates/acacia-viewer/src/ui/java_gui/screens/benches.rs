//! Workstations whose slots are the player's UI window's (Bedrock's offsets) in the Java menu's
//! order: anvil, grindstone, smithing and cartography tables, enchanting table, beacon,
//! stonecutter and loom.

use acacia_bot::items::{SlotRef, ui};
use acacia_bot::workstation::AnvilCost;
use acacia_ui::inventory::{self, Bench, Layout, Slot};
use acacia_ui::java::container::anvil::Anvil;
use acacia_ui::java::container::beacon::{Beacon, PAYMENT};
use acacia_ui::java::container::cartography::{Cartography, MapData};
use acacia_ui::java::container::enchanting::{Clue, Enchanting};
use acacia_ui::java::container::grindstone::Grindstone;
use acacia_ui::java::container::loom::{Loom, Pattern};
use acacia_ui::java::container::smithing::Smithing;
use acacia_ui::java::container::stonecutter::Stonecutter;
use acacia_ui::java::container::Base;
use acacia_ui::java::text::Text;

use super::super::items::plain;
use super::build::{Kit, fill, player_targets};
use super::{Open, Target};
use crate::enchant_names::JAVA as ENCHANTMENTS;

/// The bench's own slots in Java's order, then the player's: Bedrock's offsets come in Java's order
/// from `inventory::slots`, the result last.
pub(super) fn targets(kit: &Kit, bench: Bench) -> Vec<Target> {
    let own = inventory::slots(Layout::Bench(bench)).into_iter().skip(36).map(|(slot, _)| match slot {
        Slot::Ui(i) => Target::Slot(SlotRef::Ui(i)),
        Slot::Result => result(kit, bench),
        _ => Target::None,
    });
    let mut targets: Vec<Target> = own.collect();
    targets.extend(player_targets());
    targets
}

fn result(kit: &Kit, bench: Bench) -> Target {
    let (inv, pick) = (kit.view.inventory, kit.state.pick);
    match bench {
        Bench::Stonecutter => Target::Cut(pick.and_then(|i| inv.picks.get(i)).map(|p| p.0)),
        Bench::Loom => Target::Loom(pick.and_then(|i| inv.patterns.get(i)).map(|p| p.0)),
        _ => Target::Crafted,
    }
}

fn filled(kit: &mut Kit, mut base: Base, targets: &[Target]) -> Base {
    fill(kit, &mut base, targets);
    base
}

pub(super) fn bench(kit: &mut Kit, bench: Bench) -> (Open, Vec<Target>) {
    let targets = targets(kit, bench);
    let [w, h] = kit.size();
    let inv = kit.inventory_title();
    let open = match bench {
        Bench::Anvil => {
            let base = filled(kit, Base::new(w, h, [176, 166], kit.title("container.repair"), inv, Anvil::slots()), &targets);
            let xp = kit.station(Vec::new()).xp;
            let cost = match kit.view.inventory.cost {
                Some(AnvilCost::Levels(n)) => n,
                Some(AnvilCost::Short(n)) => xp + n,
                Some(AnvilCost::TooExpensive) => 40,
                None => 0,
            };
            let typed = Some(kit.view.name.0.as_str()).filter(|n| !n.is_empty());
            let blink = kit.ticks() / 6 % 2 == 0;
            Open::Anvil(Anvil::new(base, kit.station(vec![cost]), typed, blink, kit.env.font, kit.env.lang, kit.env.game))
        }
        Bench::Grindstone => {
            let mut s = Grindstone::new(w, h, kit.title("container.grindstone_title"), inv);
            fill(kit, &mut s.base, &targets);
            Open::Grindstone(s)
        }
        Bench::Smithing => {
            let mut s = Smithing::new(w, h, kit.title("container.upgrade"), inv, kit.station(Vec::new()), kit.ticks());
            fill(kit, &mut s.base, &targets);
            Open::Smithing(s)
        }
        Bench::Cartography => {
            // A map's scale and lock are in its item data, not tracked: scale 0, unlocked, extendable.
            let map = kit.view.inventory.ui.get(usize::from(ui::CARTOGRAPHY_INPUT)).and_then(Option::as_ref).map(|_| MapData { scale: 0, locked: false, decorations: 0 });
            let mut s = Cartography::new(w, h, kit.title("container.cartography_table"), inv, map, true);
            fill(kit, &mut s.base, &targets);
            Open::Cartography(s)
        }
        Bench::Enchanting => enchanting(kit, &targets, inv),
        Bench::Beacon => beacon(kit, &targets, inv),
        Bench::Stonecutter => stonecutter(kit, &targets, inv),
        Bench::Loom => loom(kit, &targets, inv),
        Bench::Crafting => unreachable!("the crafting table is a recipe book screen"),
    };
    (open, targets)
}

fn enchanting(kit: &mut Kit, targets: &[Target], inv: Text) -> Open {
    let options = &kit.view.inventory.enchants;
    let option = |i: usize| options.get(i);
    let java = |id: u8| ENCHANTMENTS.get(usize::from(id));
    // Data: three costs, the seed (not sent: 0), three clue ids, three clue levels.
    let mut data: Vec<i32> = (0..3).map(|i| option(i).map_or(0, |o| i32::from(o.0))).collect();
    data.push(0);
    data.extend((0..3).map(|i| option(i).and_then(|o| o.1).filter(|h| java(h.0).is_some()).map_or(-1, |h| i32::from(h.0))));
    data.extend((0..3).map(|i| option(i).and_then(|o| o.1).map_or(-1, |h| i32::from(h.1))));
    let clues = std::array::from_fn(|i| {
        let (id, max) = *java(option(i)?.1?.0)?;
        Some(Clue { description: kit.tr(&format!("enchantment.minecraft.{id}")), curse: id.ends_with("_curse"), max_level: max })
    });
    let book = if options.is_empty() { [0.0; 2] } else { [1.0; 2] };
    let mut s = Enchanting::new(kit.size()[0], kit.size()[1], kit.title("container.enchant"), inv, kit.station(data), clues, book, [0.0; 2]);
    fill(kit, &mut s.base, targets);
    Open::Enchanting(s)
}

fn beacon(kit: &mut Kit, targets: &[Target], inv: Text) -> Open {
    let station = kit.station(vec![i32::from(kit.view.inventory.beacon)]);
    let paid = kit.view.inventory.ui.get(usize::from(ui::BEACON_PAYMENT)).is_some_and(Option::is_some);
    let (primary, secondary) = &kit.state.beacon;
    let mut s = Beacon::new(kit.size()[0], kit.size()[1], kit.title("container.beacon"), inv, &station, primary.as_deref(), secondary.as_deref(), paid);
    fill(kit, &mut s.base, targets);
    for (i, name) in ["minecraft:emerald", "minecraft:diamond", "minecraft:gold_ingot", "minecraft:iron_ingot", "minecraft:netherite_ingot"].iter().enumerate() {
        kit.keyed(&plain(name), PAYMENT + i as i32);
    }
    Open::Beacon(s)
}

fn stonecutter(kit: &mut Kit, targets: &[Target], inv: Text) -> Open {
    let mut s = Stonecutter::new(kit.size()[0], kit.size()[1], kit.title("container.stonecutter"), inv);
    fill(kit, &mut s.base, targets);
    let picks = &kit.view.inventory.picks;
    s.recipes = picks.iter().map(|(_, stack)| kit.listed(stack)).collect();
    s.container_changed();
    s.selected = kit.state.pick.map_or(-1, |i| i as i32);
    s.set_scroll(kit.state.scroll);
    Open::Stonecutter(s)
}

/// `block.minecraft.banner.<pattern>` and `entity/banner/<pattern>` of a Bedrock pattern code (an
/// unknown code stands for itself).
fn pattern(code: &str) -> Pattern {
    let banner = acacia_render::banner::Banner::from_bedrock(0, [(code, 0)], 0);
    let name = banner.layers.first().map_or(code, |l| l.0);
    Pattern { translation_key: format!("block.minecraft.banner.{name}"), sprite: format!("minecraft:entity/banner/{name}") }
}

fn loom(kit: &mut Kit, targets: &[Target], inv: Text) -> Open {
    let mut s = Loom::new(kit.size()[0], kit.size()[1], kit.title("container.loom"), inv);
    fill(kit, &mut s.base, targets);
    s.patterns = kit.view.inventory.patterns.iter().map(|(code, _)| pattern(code)).collect();
    s.selected = kit.state.pick.map_or(-1, |i| i as i32);
    s.container_changed();
    s.set_scroll(kit.state.scroll);
    Open::Loom(s)
}
