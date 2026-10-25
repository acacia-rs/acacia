//! Containers whose slots are the open window's own: chests, barrels, ender chests and shulker
//! boxes (`ContainerScreen`, `ShulkerBoxScreen`), dispensers and droppers, hoppers, furnaces with
//! their recipe book, brewing stands and crafters.

use acacia_bot::items::SlotRef;
use acacia_ui::inventory::Station;
use acacia_ui::java::container::brewing::Brewing;
use acacia_ui::java::container::crafter::Crafter;
use acacia_ui::java::container::recipe_book::Kind as BookKind;
use acacia_ui::java::container::recipe_book::screens::Window;
use acacia_ui::java::container::simple::{Kind, Simple};

use super::build::{Kit, fill, player_targets};
use super::{Open, Target, player};
use crate::stations::{brewing_data, furnace_data};

fn container(slots: impl IntoIterator<Item = u8>) -> Vec<Target> {
    slots.into_iter().map(|i| Target::Slot(SlotRef::Container(i))).collect()
}

/// The container's slots, then the player's.
fn targets(own: impl IntoIterator<Item = u8>) -> Vec<Target> {
    let mut targets = container(own);
    targets.extend(player_targets());
    targets
}

/// A container of rows of nine, by the block it was opened on.
pub(super) fn chest(kit: &mut Kit, slots: usize) -> (Open, Vec<Target>) {
    let rows = (slots / 9).clamp(1, 6) as i32;
    let block = kit.view.inventory.block.unwrap_or_default();
    let shulker = block.ends_with("shulker_box") && rows == 3;
    let title = match block {
        _ if shulker => "container.shulkerBox",
        "minecraft:barrel" => "container.barrel",
        "minecraft:ender_chest" => "container.enderchest",
        _ if rows == 6 => "container.chestDouble",
        _ => "container.chest",
    };
    let kind = if shulker { Kind::Shulker } else { Kind::Chest { rows } };
    simple(kit, kind, title, targets(0..(rows * 9) as u8))
}

fn simple(kit: &mut Kit, kind: Kind, title: &str, targets: Vec<Target>) -> (Open, Vec<Target>) {
    let [w, h] = kit.size();
    let mut s = Simple::new(kind, w, h, kit.title(title), kit.inventory_title(), kit.env.font);
    fill(kit, &mut s.base, &targets);
    (Open::Simple(s), targets)
}

/// A workstation whose slots are the container's: Bedrock's slot order mapped to the Java menu's.
pub(super) fn station(kit: &mut Kit, station: Station) -> (Open, Vec<Target>) {
    let [w, h] = kit.size();
    let view = kit.view;
    let data = &view.inventory.data;
    match station {
        Station::Hopper => simple(kit, Kind::Hopper, "container.hopper", targets(0..5)),
        Station::Dispenser => simple(kit, Kind::Dispenser, "container.dispenser", targets(0..9)),
        Station::Dropper => simple(kit, Kind::Dispenser, "container.dropper", targets(0..9)),
        Station::Brewing => {
            // Java's bottles, ingredient, fuel; Bedrock's ingredient, bottles, fuel.
            let targets = targets([1, 2, 3, 0, 4]);
            let mut s = Brewing::new(w, h, kit.title("container.brewing"), kit.inventory_title(), brewing_data(data), kit.env.font);
            fill(kit, &mut s.base, &targets);
            (Open::Brewing(s), targets)
        }
        Station::Furnace | Station::BlastFurnace | Station::Smoker => {
            let (kind, title) = match station {
                Station::BlastFurnace => (BookKind::BlastFurnace, "container.blast_furnace"),
                Station::Smoker => (BookKind::Smoker, "container.smoker"),
                _ => (BookKind::Furnace, "container.furnace"),
            };
            let (window, title) = (Window::Furnace(furnace_data(station, data)), kit.title(title));
            player::recipe_screen(kit, kind, window, title, targets(0..3))
        }
    }
}

/// `CrafterScreen`: the grid, the inventory, the result.
pub(super) fn crafter(kit: &mut Kit) -> (Open, Vec<Target>) {
    let [w, h] = kit.size();
    let mut targets = targets(0..9);
    targets.push(Target::None);
    // Which slots are disabled and whether it is powered come in its block entity, not tracked: none, unpowered.
    let mut s = Crafter::new(w, h, kit.title("container.crafter"), kit.inventory_title(), [0; 10], kit.env.font);
    fill(kit, &mut s.base, &targets);
    (Open::Crafter(s), targets)
}
