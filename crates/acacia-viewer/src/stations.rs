//! Workstation windows as the inventory screen shows them: which station a window type is, and its
//! progress from the window's `ContainerSetData` properties.

use std::collections::HashMap;

use acacia_bot::proto::types::WindowType;
use acacia_ui::inventory::{Progress, Station};

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
    fn furnace_progress_comes_from_its_properties() {
        let data = HashMap::from([(COOK_TICKS, 50), (LIT_TIME, 400), (LIT_DURATION, 1600)]);
        assert_eq!(progress(Station::Furnace, &data), Progress { work: 0.25, fuel: 0.25 });
        assert_eq!(progress(Station::Smoker, &data).work, 0.5);
        assert_eq!(progress(Station::Furnace, &HashMap::new()), Progress::default(), "unlit and idle");
        let brewing = HashMap::from([(BREW_TIME, 300), (BREW_FUEL, 10), (BREW_FUEL_TOTAL, 20)]);
        assert_eq!(progress(Station::Brewing, &brewing), Progress { work: 0.25, fuel: 0.5 });
    }
}
