//! Vanilla food and drink values: hunger and saturation as Java 1.21 and Dragonfly
//! (`server/item/*.go`), use durations from Dragonfly (`DefaultConsumeDuration` 1.61 s).

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

/// Food values of an item identifier (`minecraft:bread`); `None` if it is not food.
pub fn food(name: &str) -> Option<Food> {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    Some(match name {
        "apple" => plain(4, 2.4),
        "baked_potato" | "bread" | "cooked_cod" | "cooked_rabbit" => plain(5, 6.0),
        "beef" | "porkchop" | "rabbit" => plain(3, 1.8),
        "cooked_beef" | "cooked_porkchop" => plain(8, 12.8),
        "beetroot" => plain(1, 1.2),
        "beetroot_soup" | "mushroom_stew" | "cooked_chicken" => plain(6, 7.2),
        "carrot" => plain(3, 3.6),
        "cod" | "salmon" | "cookie" | "sweet_berries" | "glow_berries" => plain(2, 0.4),
        "cooked_mutton" | "cooked_salmon" => plain(6, 9.6),
        "dried_kelp" | "tropical_fish" => plain(1, 0.2),
        "golden_carrot" => plain(6, 14.4),
        "honey_bottle" => Food { always_edible: true, ..plain(6, 1.2) },
        "melon_slice" | "mutton" => plain(2, 1.2),
        "potato" => plain(1, 0.6),
        "pumpkin_pie" => plain(8, 4.8),
        "rabbit_stew" => plain(10, 12.0),
        "golden_apple" | "enchanted_golden_apple" => Food { always_edible: true, precious: true, ..plain(4, 9.6) },
        "chicken" | "poisonous_potato" => harmful(2, 1.2),
        "pufferfish" => harmful(1, 0.2),
        "rotten_flesh" => harmful(4, 0.8),
        "spider_eye" => harmful(2, 3.2),
        "suspicious_stew" => Food { always_edible: true, ..harmful(6, 7.2) },
        "chorus_fruit" => Food { always_edible: true, ..harmful(4, 2.4) },
        _ => return None,
    })
}

/// Ticks "use" is held before the item is consumed; `None` for anything that is not a food or drink.
pub fn use_ticks(name: &str) -> Option<u32> {
    match name.strip_prefix("minecraft:").unwrap_or(name) {
        "dried_kelp" => Some(16),
        "honey_bottle" => Some(40),
        "potion" | "milk_bucket" | "ominous_bottle" => Some(32),
        other => food(other).map(|_| 32),
    }
}

/// Consumable with a full hunger bar (drinks and `always_edible` food).
pub(crate) fn always_consumable(name: &str) -> bool {
    let bare = name.strip_prefix("minecraft:").unwrap_or(name);
    matches!(bare, "potion" | "milk_bucket" | "ominous_bottle") || food(bare).is_some_and(|f| f.always_edible)
}

/// The slot of the food to eat among `candidates` (`(slot, identifier)`), skipping harmful and
/// precious food; `missing` is how many hunger points the bar lacks. Ties go to the earlier slot.
pub(crate) fn choose_food<'a>(candidates: impl Iterator<Item = (u8, &'a str)>, missing: f32, choice: FoodChoice) -> Option<u8> {
    let edible: Vec<(u8, Food)> = candidates
        .filter_map(|(slot, name)| food(name).map(|f| (slot, f)))
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
