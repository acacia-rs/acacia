//! Workstation windows as the inventory screen shows them: which station a window type is, and its
//! progress from the window's `ContainerSetData` properties.

use std::collections::HashMap;

use acacia_bot::proto::types::WindowType;
use acacia_bot::Bot;
use acacia_bot::state::{ItemStack, TradeItem};
use acacia_bot::workstation::BeaconEffect;
use acacia_ui::inventory::{Bench, Progress, Station};

use crate::control::{Stack, stack_of};

/// Furnace properties: ticks cooked, ticks of flame left, and what the burning fuel gave.
const COOK_TICKS: i32 = 0;
const LIT_TIME: i32 = 1;
const LIT_DURATION: i32 = 2;
/// Brewing stand properties: ticks left of 400, blaze powder charges left, and of how many.
const BREW_TIME: i32 = 0;
const BREW_FUEL: i32 = 1;
const BREW_FUEL_TOTAL: i32 = 2;
const BREW_TICKS: f32 = 400.0;

pub fn station(window: WindowType) -> Option<Station> {
    Some(match window {
        WindowType::Furnace => Station::Furnace,
        WindowType::BlastFurnace => Station::BlastFurnace,
        WindowType::Smoker => Station::Smoker,
        WindowType::Hopper => Station::Hopper,
        WindowType::Dispenser => Station::Dispenser,
        WindowType::Dropper => Station::Dropper,
        WindowType::BrewingStand => Station::Brewing,
        _ => return None,
    })
}

/// The open trading screen.
#[derive(Debug, Clone, PartialEq)]
pub struct Trade {
    /// The trader's name as the server sent it (may be a translation key).
    pub title: String,
    /// A villager's level; a wandering trader has none.
    pub level: Option<&'static str>,
    /// How far a villager is to its next level, 0 to 1; none for a master.
    pub progress: Option<f32>,
    pub offers: Vec<Offer>,
    /// The trader's level (0 = novice) and experience as the server sent them.
    pub tier: i32,
    pub xp: i32,
}

/// The experience at which a villager reaches each level.
const LEVEL_XP: [i32; 5] = [0, 10, 70, 150, 250];

/// The share of level `tier`'s experience a villager with `xp` has gathered.
fn level_progress(tier: i32, xp: i32) -> Option<f32> {
    let tier = usize::try_from(tier).ok()?;
    let (from, to) = (*LEVEL_XP.get(tier)?, *LEVEL_XP.get(tier + 1)?);
    Some(((xp - from) as f32 / (to - from) as f32).clamp(0.0, 1.0))
}

/// Java's `merchant.level.1` to `.5`.
const LEVELS: [&str; 5] = ["Novice", "Apprentice", "Journeyman", "Expert", "Master"];

/// The name of level `tier` (0 = novice) for a trader whose offers show it has levels: a
/// villager lists its later levels' offers from the start, a wandering trader has one level.
fn level(tier: i32, offer_tiers: impl Iterator<Item = i32>) -> Option<&'static str> {
    let levelled = offer_tiers.max().is_some_and(|highest| highest > 0);
    LEVELS.get(usize::try_from(tier).ok()?).copied().filter(|_| levelled)
}

#[cfg(test)]
#[test]
fn only_a_trader_with_later_offers_has_a_level() {
    assert_eq!((level(0, [0, 0, 1].into_iter()), level(4, [0, 4].into_iter())), (Some("Novice"), Some("Master")));
    assert_eq!((level(0, [0, 0].into_iter()), level(0, [].into_iter())), (None, None));
    assert_eq!((level_progress(0, 5), level_progress(1, 70), level_progress(2, 60), level_progress(4, 300)), (Some(0.5), Some(1.0), Some(0.0), None));
}

#[derive(Debug, Clone, PartialEq)]
pub struct Offer {
    pub price: Stack,
    pub second_price: Option<Stack>,
    pub goods: Stack,
    /// Not sold out and within the trader's level.
    pub open: bool,
}

pub fn trade(bot: &Bot) -> Option<Trade> {
    let window = bot.state().stations.trade.as_ref()?;
    // Prices say "any aux value" as 32767.
    let item = |t: &TradeItem| {
        let metadata = if t.metadata == i16::MAX { 0 } else { t.metadata.max(0) as u32 };
        stack_of(bot, &ItemStack { network_id: bot.state().items.id(&t.name)?, count: t.count, metadata, ..ItemStack::default() })
    };
    let offers = window.offers.iter().filter_map(|o| {
        Some(Offer { price: item(&o.buy_a)?, second_price: o.buy_b.as_ref().and_then(item), goods: item(&o.sell)?, open: !o.is_disabled() && o.tier <= window.tier })
    });
    let level = level(window.tier, window.offers.iter().map(|o| o.tier));
    let trader = window.ids.iter().find_map(|id| bot.state().entities.by_unique(*id).filter(|e| !e.is_player()));
    let xp = trader.map_or(0, |trader| trader.metadata.int(acacia_bot::proto::types::MetadataDictionaryItemKey::TradeExperience));
    tracing::debug!(target: "trade", xp, tier = window.tier, ids = ?window.ids, known = trader.is_some(), "trader");
    Some(Trade { title: window.display_name.clone(), level, progress: level.and_then(|_| level_progress(window.tier, xp)), offers: offers.collect(), tier: window.tier, xp })
}

/// The beacon screen's buttons: the five powers, the two second powers, and the confirm.
pub const BEACON: [&str; 8] = ["Speed", "Haste", "Resist", "Jump", "Strength", "Regen", "II", "OK"];
/// Each button's image: an effect's icon, or an atlas sprite by name. Level II has none.
pub const BEACON_ICONS: [&str; 8] = ["speed", "haste", "resistance", "jump_boost", "strength", "regeneration", "", "sprites/container/beacon/confirm"];
const POWERS: [BeaconEffect; 5] = [BeaconEffect::Speed, BeaconEffect::Haste, BeaconEffect::Resistance, BeaconEffect::JumpBoost, BeaconEffect::Strength];
const REGENERATION: usize = 5;
pub const BEACON_DONE: usize = 7;

/// Whether a pyramid of `levels` layers gives the button's power: two powers at one layer, two
/// more at two, strength at three, a second power at four.
pub fn beacon_allows(button: usize, levels: u8) -> bool {
    levels >= [1, 1, 2, 2, 3, 4, 4].get(button).copied().unwrap_or(0)
}

/// The powers the picked buttons stand for: the second is regeneration or the first again.
pub fn beacon_powers([first, second]: [Option<usize>; 2]) -> Option<(BeaconEffect, Option<BeaconEffect>)> {
    let primary = *POWERS.get(first?)?;
    Some((primary, second.map(|s| if s == REGENERATION { BeaconEffect::Regeneration } else { primary })))
}

pub fn bench(window: WindowType) -> Option<Bench> {
    Some(match window {
        WindowType::Workbench => Bench::Crafting,
        WindowType::Anvil => Bench::Anvil,
        WindowType::SmithingTable => Bench::Smithing,
        WindowType::Grindstone => Bench::Grindstone,
        WindowType::Stonecutter => Bench::Stonecutter,
        WindowType::Cartography => Bench::Cartography,
        WindowType::Enchantment => Bench::Enchanting,
        WindowType::Beacon => Bench::Beacon,
        WindowType::Loom => Bench::Loom,
        _ => return None,
    })
}

/// A furnace's properties as Java's `AbstractFurnaceMenu` holds them: lit time, lit duration,
/// cooking progress, cooking total.
pub fn furnace_data(station: Station, data: &HashMap<i32, i32>) -> [i32; 4] {
    let get = |key: i32| data.get(&key).copied().unwrap_or(0);
    let total = if station == Station::Furnace { 200 } else { 100 };
    [get(LIT_TIME), get(LIT_DURATION), get(COOK_TICKS), total]
}

/// A brewing stand's as Java's `BrewingStandMenu` holds them: brewing ticks left, fuel, total
/// brewing ticks, total fuel.
pub fn brewing_data(data: &HashMap<i32, i32>) -> [i32; 4] {
    let get = |key: i32| data.get(&key).copied().unwrap_or(0);
    [get(BREW_TIME), get(BREW_FUEL), BREW_TICKS as i32, get(BREW_FUEL_TOTAL)]
}

pub fn progress(station: Station, data: &HashMap<i32, i32>) -> Progress {
    let get = |key: i32| data.get(&key).copied().unwrap_or(0) as f32;
    let share = |part: f32, whole: f32| if whole > 0.0 { (part / whole).clamp(0.0, 1.0) } else { 0.0 };
    match station {
        // A furnace cooks in 200 ticks, a blast furnace or smoker in 100.
        Station::Furnace => Progress { work: share(get(COOK_TICKS), 200.0), fuel: share(get(LIT_TIME), get(LIT_DURATION)) },
        Station::BlastFurnace | Station::Smoker => Progress { work: share(get(COOK_TICKS), 100.0), fuel: share(get(LIT_TIME), get(LIT_DURATION)) },
        Station::Brewing => {
            let left = get(BREW_TIME);
            Progress { work: if left > 0.0 { 1.0 - share(left, BREW_TICKS) } else { 0.0 }, fuel: share(get(BREW_FUEL), get(BREW_FUEL_TOTAL)) }
        }
        Station::Hopper | Station::Dispenser | Station::Dropper => Progress::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beacon_buttons_name_their_powers() {
        assert_eq!(beacon_powers([Some(1), None]), Some((BeaconEffect::Haste, None)));
        assert_eq!(beacon_powers([Some(4), Some(5)]), Some((BeaconEffect::Strength, Some(BeaconEffect::Regeneration))));
        assert_eq!(beacon_powers([Some(0), Some(6)]), Some((BeaconEffect::Speed, Some(BeaconEffect::Speed))), "level II");
        assert_eq!(beacon_powers([None, Some(5)]), None);
        let allowed = |levels| (0..BEACON.len()).filter(|&b| beacon_allows(b, levels)).collect::<Vec<_>>();
        assert_eq!((allowed(0), allowed(2), allowed(4).len()), (vec![BEACON_DONE], vec![0, 1, 2, 3, BEACON_DONE], 8));
    }

    #[test]
    fn furnace_progress_comes_from_its_properties() {
        let data = HashMap::from([(COOK_TICKS, 50), (LIT_TIME, 400), (LIT_DURATION, 1600)]);
        assert_eq!(progress(Station::Furnace, &data), Progress { work: 0.25, fuel: 0.25 });
        assert_eq!(progress(Station::Smoker, &data).work, 0.5);
        assert_eq!(progress(Station::Furnace, &HashMap::new()), Progress::default(), "unlit and idle");
        let brewing = HashMap::from([(BREW_TIME, 300), (BREW_FUEL, 10), (BREW_FUEL_TOTAL, 20)]);
        assert_eq!(progress(Station::Brewing, &brewing), Progress { work: 0.25, fuel: 0.5 });
    }
}
