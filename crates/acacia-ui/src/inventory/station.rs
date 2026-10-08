//! Workstations whose slots are a container's own: where Java's menus put them (`FurnaceMenu`,
//! `HopperMenu`, `DispenserMenu`, `BrewingStandMenu`), in Bedrock's slot order.

use super::SLOT;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Station {
    Furnace,
    BlastFurnace,
    Smoker,
    Hopper,
    Dispenser,
    Dropper,
    Brewing,
}

/// How far a station's work has come, each 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Progress {
    /// The furnace's arrow, or the brewing stand's.
    pub work: f32,
    /// The furnace's flame, or the brewing stand's blaze powder bar.
    pub fuel: f32,
}

impl Station {
    pub fn title(self) -> &'static str {
        match self {
            Station::Furnace => "Furnace",
            Station::BlastFurnace => "Blast Furnace",
            Station::Smoker => "Smoker",
            Station::Hopper => "Item Hopper",
            Station::Dispenser => "Dispenser",
            Station::Dropper => "Dropper",
            Station::Brewing => "Brewing Stand",
        }
    }

    /// Java's sheet under `textures/gui/container`, and its progress sprites' folder.
    pub fn sheet(self) -> &'static str {
        match self {
            Station::Furnace => "furnace",
            Station::BlastFurnace => "blast_furnace",
            Station::Smoker => "smoker",
            Station::Hopper => "hopper",
            Station::Dispenser | Station::Dropper => "dispenser",
            Station::Brewing => "brewing_stand",
        }
    }

    pub fn is_furnace(self) -> bool {
        matches!(self, Station::Furnace | Station::BlastFurnace | Station::Smoker)
    }

    pub(super) fn height(self) -> f32 {
        if self == Station::Hopper { 133.0 } else { 166.0 }
    }

    /// Top of the player's three inventory rows.
    pub(super) fn player_rows(self) -> f32 {
        if self == Station::Hopper { 51.0 } else { 84.0 }
    }

    /// How many container slots the station has.
    pub fn slot_count(self) -> usize {
        self.slots().len()
    }

    /// The top-left of each container slot's item, by slot index.
    pub(super) fn slots(self) -> Vec<[f32; 2]> {
        match self {
            // Ingredient, fuel, result.
            Station::Furnace | Station::BlastFurnace | Station::Smoker => vec![[56.0, 17.0], [56.0, 53.0], [116.0, 35.0]],
            Station::Hopper => (0..5).map(|i| [44.0 + SLOT * i as f32, 20.0]).collect(),
            Station::Dispenser | Station::Dropper => (0..9).map(|i| [62.0 + SLOT * (i % 3) as f32, 17.0 + SLOT * (i / 3) as f32]).collect(),
            // Bedrock's order: the ingredient, three bottles, the blaze powder.
            Station::Brewing => vec![[79.0, 17.0], [56.0, 51.0], [79.0, 58.0], [102.0, 51.0], [17.0, 17.0]],
        }
    }
}
