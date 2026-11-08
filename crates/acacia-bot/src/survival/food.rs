//! Vanilla food and drink values: hunger and saturation as Java 1.21 and Dragonfly
//! (`server/item/*.go`), use durations from Dragonfly (`DefaultConsumeDuration` 1.61 s).

use acacia_world::neutral::Item;

/// What eating one item restores.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Food {
    pub hunger: u8,
    pub saturation: f32,
    /// Edible with a full hunger bar.
    pub always_edible: bool,
    /// A bad or random side effect (poison, hunger, nausea, teleport): auto-eat skips it.
    pub harmful: bool,
    /// Worth more than its food value (golden apples): auto-eat skips it.
    pub precious: bool,
}

/// Which food auto-eat picks from the hotbar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FoodChoice {
    /// The most saturation.
    #[default]
    Saturation,
    /// The most hunger points that still fit in the bar, else the fewest.
    NoWaste,
}

const fn plain(hunger: u8, saturation: f32) -> Food {
    Food { hunger, saturation, always_edible: false, harmful: false, precious: false }
}

const fn harmful(hunger: u8, saturation: f32) -> Food {
    Food { harmful: true, ..plain(hunger, saturation) }
}

/// What eating `item` restores; `None` if it is not food.
pub fn food(item: Item) -> Option<Food> {
    use Item::*;
    Some(match item {
        Apple => plain(4, 2.4),
        BakedPotato | Bread | CookedCod | CookedRabbit => plain(5, 6.0),
        Beef | Porkchop | Rabbit => plain(3, 1.8),
        CookedBeef | CookedPorkchop => plain(8, 12.8),
        Beetroot => plain(1, 1.2),
        BeetrootSoup | MushroomStew | CookedChicken => plain(6, 7.2),
        Carrot => plain(3, 3.6),
        Cod | Salmon | Cookie | SweetBerries | GlowBerries => plain(2, 0.4),
        CookedMutton | CookedSalmon => plain(6, 9.6),
        DriedKelp | TropicalFish => plain(1, 0.2),
        GoldenCarrot => plain(6, 14.4),
        HoneyBottle => Food { always_edible: true, ..plain(6, 1.2) },
        MelonSlice | Mutton => plain(2, 1.2),
        Potato => plain(1, 0.6),
        PumpkinPie => plain(8, 4.8),
        RabbitStew => plain(10, 12.0),
        GoldenApple | EnchantedGoldenApple => Food { always_edible: true, precious: true, ..plain(4, 9.6) },
        Chicken | PoisonousPotato => harmful(2, 1.2),
        Pufferfish => harmful(1, 0.2),
        RottenFlesh => harmful(4, 0.8),
        SpiderEye => harmful(2, 3.2),
        SuspiciousStew => Food { always_edible: true, ..harmful(6, 7.2) },
        ChorusFruit => Food { always_edible: true, ..harmful(4, 2.4) },
        _ => return None,
    })
}

/// Ticks "use" is held before `item` is consumed; `None` for anything that is not a food or drink.
pub fn use_ticks(item: Item) -> Option<u32> {
    match item {
        Item::DriedKelp => Some(16),
        Item::HoneyBottle => Some(40),
        Item::Potion | Item::MilkBucket | Item::OminousBottle => Some(32),
        other => food(other).map(|_| 32),
    }
}

/// Consumable with a full hunger bar (drinks and `always_edible` food).
pub(crate) fn always_consumable(item: Item) -> bool {
    matches!(item, Item::Potion | Item::MilkBucket | Item::OminousBottle) || food(item).is_some_and(|f| f.always_edible)
}

/// The slot of the food to eat among `candidates` (`(slot, item)`), skipping harmful and precious food;
/// `missing` is how many hunger points the bar lacks. Ties go to the earlier slot.
pub(crate) fn choose_food(candidates: impl Iterator<Item = (u8, Item)>, missing: f32, choice: FoodChoice) -> Option<u8> {
    let edible: Vec<(u8, Food)> = candidates
        .filter_map(|(slot, item)| food(item).map(|f| (slot, f)))
        .filter(|(_, f)| !f.harmful && !f.precious)
        .collect();
    let better = |a: &Food, b: &Food| match choice {
        FoodChoice::Saturation => (a.saturation, a.hunger) > (b.saturation, b.hunger),
        FoodChoice::NoWaste => {
            let fits = |f: &Food| f32::from(f.hunger) <= missing;
            match (fits(a), fits(b)) {
                (true, false) => true,
                (false, true) => false,
                (true, true) => a.hunger > b.hunger,
                (false, false) => a.hunger < b.hunger,
            }
        }
    };
    let mut best: Option<(u8, Food)> = None;
    for (slot, f) in edible {
        if best.as_ref().is_none_or(|(_, b)| better(&f, b)) {
            best = Some((slot, f));
        }
    }
    best.map(|(slot, _)| slot)
}
