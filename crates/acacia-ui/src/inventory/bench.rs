//! Workstations whose slots are the player's own UI slots (Bedrock's window 124, by offset) and
//! whose result is worked out, not stored: where Java's menus put them (`CraftingMenu`,
//! `AnvilMenu`, `SmithingMenu`, `GrindstoneMenu`, `StonecutterMenu`,
//! `CartographyTableMenu`, `EnchantmentMenu`, `LoomMenu`). The beacon is not Java's 230×219 screen: its
//! powers are a pick list over the payment slot's panel.

use super::{SLOT, Slot};

/// First UI offset of the player's 2×2 grid and of the crafting table's 3×3, row by row.
pub(super) const GRID_2X2: u8 = 28;
const GRID_3X3: u8 = 32;
const ANVIL: [u8; 2] = [1, 2];
const STONECUTTER: u8 = 3;
const CARTOGRAPHY: [u8; 2] = [12, 13];
/// The item, the lapis.
const ENCHANTING: [u8; 2] = [14, 15];
const BEACON: u8 = 27;
/// Banner, dye, pattern item.
const LOOM: [u8; 3] = [9, 10, 11];
const GRINDSTONE: [u8; 2] = [16, 17];
/// Template, base, addition.
const SMITHING: [u8; 3] = [53, 51, 52];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bench {
    Crafting,
    Anvil,
    Smithing,
    Grindstone,
    Stonecutter,
    Cartography,
    Enchanting,
    Beacon,
    Loom,
}

/// A bench's pick list: buttons in rows from `at`, each `cell` big.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Picks {
    pub at: [f32; 2],
    pub columns: usize,
    pub rows: usize,
    pub cell: [f32; 2],
    /// Where a button's text starts.
    pub text: f32,
}

impl Bench {
    pub fn title(self) -> &'static str {
        match self {
            Bench::Crafting => "Crafting",
            Bench::Anvil => "Repair & Name",
            Bench::Smithing => "Upgrade Gear",
            Bench::Grindstone => "Repair & Disenchant",
            Bench::Stonecutter => "Stonecutter",
            Bench::Cartography => "Cartography Table",
            Bench::Enchanting => "Enchant",
            Bench::Beacon => "Beacon",
            Bench::Loom => "Loom",
        }
    }

    /// Java's sheet under `textures/gui/container`.
    pub fn sheet(self) -> &'static str {
        match self {
            Bench::Crafting => "crafting_table",
            Bench::Anvil => "anvil",
            Bench::Smithing => "smithing",
            Bench::Grindstone => "grindstone",
            Bench::Stonecutter => "stonecutter",
            Bench::Cartography => "cartography_table",
            Bench::Enchanting => "enchanting_table",
            Bench::Loom => "loom",
            // No sheet of this size: the panel is drawn plain.
            Bench::Beacon => "",
        }
    }

    pub(super) fn title_at(self) -> [f32; 2] {
        match self {
            Bench::Crafting => [29.0, 6.0],
            Bench::Anvil => [60.0, 6.0],
            Bench::Smithing => [44.0, 15.0],
            Bench::Grindstone | Bench::Stonecutter | Bench::Beacon => [8.0, 6.0],
            Bench::Cartography | Bench::Loom => [8.0, 4.0],
            Bench::Enchanting => [12.0, 6.0],
        }
    }

    /// The results to pick from, for a bench that offers several.
    pub fn picks(self) -> Option<Picks> {
        match self {
            Bench::Stonecutter => Some(Picks { at: [52.0, 14.0], columns: 4, rows: 3, cell: [16.0, 18.0], text: 5.0 }),
            Bench::Enchanting => Some(Picks { at: [60.0, 14.0], columns: 1, rows: 3, cell: [108.0, 19.0], text: 5.0 }),
            Bench::Beacon => Some(Picks { at: [58.0, 12.0], columns: 2, rows: 4, cell: [55.0, 17.0], text: 4.0 }),
            Bench::Loom => Some(Picks { at: [60.0, 13.0], columns: 4, rows: 4, cell: [14.0, 14.0], text: 5.0 }),
            _ => None,
        }
    }

    /// Where the arrow to the result is, for looks whose panel has none printed on.
    pub(super) fn arrow(self) -> Option<[f32; 2]> {
        Some(match self {
            Bench::Stonecutter => [120.0, 33.0],
            Bench::Crafting => [90.0, 35.0],
            Bench::Anvil => [102.0, 47.0],
            Bench::Smithing => [68.0, 48.0],
            Bench::Grindstone => [95.0, 34.0],
            Bench::Cartography => [40.0, 35.0],
            Bench::Loom => [118.0, 40.0],
            Bench::Enchanting | Bench::Beacon => return None,
        })
    }

    /// Where the box for the result's new name is (110×16), for a bench that renames.
    pub fn name_box(self) -> Option<[f32; 2]> {
        (self == Bench::Anvil).then_some([59.0, 20.0])
    }

    /// The crafting table's result slot is the 26-pixel one.
    pub(super) fn big_result(self) -> bool {
        self == Bench::Crafting
    }

    /// The top-left of each slot's item.
    pub(super) fn slots(self) -> Vec<(Slot, [f32; 2])> {
        let ui = |offsets: &[u8], at: &[[f32; 2]], result: [f32; 2]| offsets.iter().zip(at).map(|(&o, &at)| (Slot::Ui(o), at)).chain([(Slot::Result, result)]).collect();
        match self {
            Bench::Crafting => {
                let cell = |i: u8| (Slot::Ui(GRID_3X3 + i), [30.0 + SLOT * f32::from(i % 3), 17.0 + SLOT * f32::from(i / 3)]);
                (0..9).map(cell).chain([(Slot::Result, [124.0, 35.0])]).collect()
            }
            Bench::Anvil => ui(&ANVIL, &[[27.0, 47.0], [76.0, 47.0]], [134.0, 47.0]),
            Bench::Smithing => ui(&SMITHING, &[[8.0, 48.0], [26.0, 48.0], [44.0, 48.0]], [98.0, 48.0]),
            Bench::Grindstone => ui(&GRINDSTONE, &[[49.0, 19.0], [49.0, 40.0]], [129.0, 34.0]),
            Bench::Stonecutter => ui(&[STONECUTTER], &[[20.0, 33.0]], [143.0, 33.0]),
            Bench::Cartography => ui(&CARTOGRAPHY, &[[15.0, 15.0], [15.0, 52.0]], [145.0, 39.0]),
            Bench::Enchanting => vec![(Slot::Ui(ENCHANTING[0]), [15.0, 47.0]), (Slot::Ui(ENCHANTING[1]), [35.0, 47.0])],
            Bench::Beacon => vec![(Slot::Ui(BEACON), [26.0, 38.0])],
            Bench::Loom => ui(&LOOM, &[[13.0, 26.0], [33.0, 26.0], [23.0, 45.0]], [143.0, 58.0]),
        }
    }
}
