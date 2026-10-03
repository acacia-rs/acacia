//! Human-looking delays: placeholders unless a capture sample count is given. Every caller
//! names its delay here, so refitting one from a capture touches one constant.

use std::time::Duration;

use crate::cadence::{seed, splitmix};

/// (min, max) in ms, uniform.
pub(crate) type Range = (u64, u64);

/// Form shown → reply: a base, per-character and per-input time, capped. Fitted to 4 forms of the
/// 2026-10-02 capture (1.7-4.6 s; research/forms.md "Timing").
pub(crate) const FORM_READ_BASE: Range = (1300, 3000);
pub(crate) const FORM_READ_PER_CHAR_MS: u64 = 15;
pub(crate) const FORM_READ_PER_INPUT_MS: u64 = 600;
pub(crate) const FORM_READ_MAX_MS: u64 = 8000;
/// Between two clicks in an open screen, from the previous response: container screens of the
/// 2026-10-02 capture, n 39, 230-1540 ms, median about 770 (research/workstations.md "Timing").
pub(crate) const CLICK: Range = (180, 1450);
/// Letting go of "use" once the food or drink is consumed.
pub(crate) const USE_RELEASE: Range = (60, 250);
/// Noticing the hunger bar before reaching for food (auto-eat).
pub(crate) const HUNGER_NOTICE: Range = (800, 3000);
/// Between selecting a hotbar slot and pressing use (capture: 0.6, 1.05, 2.6 s), and before switching back after eating.
pub(crate) const HOTBAR_SWITCH: Range = (500, 1100);
/// Typing on a sign or book page: per character after a TYPE_BASE, capped.
pub(crate) const TYPE_PER_CHAR_MS: u64 = 180;
pub(crate) const TYPE_MAX_MS: u64 = 40_000;
/// From a fish biting to reeling in (vanilla 518 ms, n=1). BDS's bite lasts 10-29 ticks, so reaction plus
/// round trip stays under ~450 ms to catch the shortest.
pub(crate) const FISH_REACTION: Range = (240, 350);
/// Picking an enchantment, trade or recipe, or taking a craft result, after the last input landed:
/// workstations of the 2026-10-02 capture, n 8, 550-1360 ms.
pub(crate) const CHOOSE: Range = (500, 1400);
/// Container screens, fitted to the 2026-10-02 vanilla capture (docs/research/workstations.md
/// "Timing"): from the screen opening to the first click, which adds a CLICK (n=15, median 1.5 s,
/// three hesitations of 4.5-10 s left out).
pub(crate) const SCREEN_OPEN_LOOK: Range = (600, 1700);
/// Beacon: from the payment landing to confirming a power (n=1).
pub(crate) const BEACON_CONFIRM: Range = (1500, 2500);
/// From the last click to closing the screen (n=13, 0.4-4 s, median 1 s).
pub(crate) const SCREEN_LINGER: Range = (400, 2500);

// Fitted to one vanilla capture (docs/research/vanilla-actions-2026-10-02.md); n = samples.
/// Sign editor or book open to close, beside per-character time (n=3: 2.15-3.0 s for 3-4 characters).
pub(crate) const TYPE_BASE: Range = (1400, 2600);
/// Extra time on the signing screen: title button, title, confirm (n=1: 4.1 s for 2 characters).
pub(crate) const BOOK_SIGN: Range = (1000, 2000);
/// Block click: StartItemUseOn to StopItemUseOn (n=25: 15-308 ms, most 30-140).
pub(crate) const STOP_USE_ON: Range = (30, 140);
/// Crosshair reported on an entity to clicking it (n=10: 75-1277 ms, most under 450).
pub(crate) const ENTITY_HOVER: Range = (75, 450);
/// Entity right-click: the transaction to the client's own swing (n=5: 124-203 ms).
pub(crate) const ENTITY_SWING: Range = (120, 210);
/// Held item changed by the server or a container move, to the client's MobEquipment (n=15: 43-247 ms, two near 700).
pub(crate) const EQUIP_RESYNC: Range = (60, 250);
/// Bed click to `PlayerAction StartSleeping` (n=1: 513 ms).
pub(crate) const START_SLEEP: Range = (400, 650);
/// Server `Animate WakeUp` to `PlayerAction StopSleeping` (n=1: 105 ms).
pub(crate) const WAKE_REPLY: Range = (80, 160);

pub(crate) struct Human {
    rng: u64,
}

impl Default for Human {
    fn default() -> Self {
        Self { rng: seed() }
    }
}

impl Human {
    pub(crate) fn between(&mut self, (lo, hi): Range) -> Duration {
        Duration::from_millis(lo + splitmix(&mut self.rng) % (hi - lo + 1))
    }

    /// Time to read `chars` characters of text, fill in `inputs` fields and decide.
    pub(crate) fn reading(&mut self, chars: usize, inputs: usize) -> Duration {
        let extra = Duration::from_millis(FORM_READ_PER_CHAR_MS * chars as u64 + FORM_READ_PER_INPUT_MS * inputs as u64);
        (self.between(FORM_READ_BASE) + extra).min(Duration::from_millis(FORM_READ_MAX_MS))
    }

    /// Time to type `chars` characters.
    pub(crate) fn typing(&mut self, chars: usize) -> Duration {
        let per_char = Duration::from_millis(TYPE_PER_CHAR_MS * chars as u64);
        (self.between(TYPE_BASE) + per_char).min(Duration::from_millis(TYPE_MAX_MS))
    }

    /// [`Self::between`] in 50 ms client ticks.
    pub(crate) fn ticks_between(&mut self, range: Range) -> u32 {
        (self.between(range).as_millis() / 50) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delays_stay_in_range() {
        let mut h = Human::default();
        for _ in 0..1000 {
            let d = h.between(CLICK).as_millis() as u64;
            assert!((CLICK.0..=CLICK.1).contains(&d));
        }
        assert!(h.reading(10_000, 3) <= Duration::from_millis(FORM_READ_MAX_MS));
        assert!(h.reading(0, 0) >= Duration::from_millis(FORM_READ_BASE.0));
        assert!(h.reading(0, 4) >= Duration::from_millis(FORM_READ_BASE.0 + 4 * FORM_READ_PER_INPUT_MS));
    }
}
