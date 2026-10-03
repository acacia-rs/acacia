//! Holding "use" on a food or drink until the server consumes it, one client tick at a time.
//! Sequence and sources: docs/research/survival-signs-beds.md §1. No `ReleaseItem` is sent: BDS
//! completes the use itself and a Release before that cancels it.

use crate::ActionError;

/// How long to wait for the server to apply the consumption after the finishing click.
const CONFIRM_TICKS: u32 = 40;
/// Eating effects start this many ticks in and repeat every `EFFECT_EVERY` (Java's
/// `shouldTriggerItemUseEffects`; Bedrock's cadence is not known).
const EFFECTS_AFTER: u32 = 7;
const EFFECT_EVERY: u32 = 4;

/// What the server shows of the held item and the hunger bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Snapshot {
    pub network_id: i32,
    pub count: u16,
    pub hunger: f32,
    pub saturation: f32,
}

impl Snapshot {
    /// The server took the item or fed the player since `before`.
    fn consumed_since(&self, before: &Snapshot) -> bool {
        self.network_id != before.network_id
            || self.count < before.count
            || self.hunger > before.hunger
            || self.saturation > before.saturation
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Phase {
    Using,
    Confirming { left: u32 },
    Releasing { left: u32, result: Result<(), ActionError> },
}

/// What the bot sends this tick.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum UseTick {
    Nothing,
    /// `EntityEvent` EatingItem.
    Effect,
    /// The finishing ClickAir.
    Finish,
    /// "Use" let go (nothing sent); the use is over with this result.
    Done(Result<(), ActionError>),
}

#[derive(Debug)]
pub(crate) struct ItemUse {
    /// Hotbar slot of the item being used.
    pub slot: u8,
    /// Started by auto-eat rather than [`crate::Bot::consume`].
    pub auto: bool,
    before: Snapshot,
    use_ticks: u32,
    release_ticks: u32,
    elapsed: u32,
    phase: Phase,
}

impl ItemUse {
    /// `release_ticks`: how long "use" stays held after the item is consumed.
    pub fn new(slot: u8, before: Snapshot, use_ticks: u32, release_ticks: u32, auto: bool) -> Self {
        Self { slot, auto, before, use_ticks, release_ticks, elapsed: 0, phase: Phase::Using }
    }

    /// Advances one client tick. `selected`: the selected hotbar slot now.
    pub fn tick(&mut self, now: &Snapshot, selected: u8, alive: bool) -> UseTick {
        if !alive || selected != self.slot {
            return UseTick::Done(Err(ActionError::NotPossible("item use interrupted".into())));
        }
        match &mut self.phase {
            Phase::Using => {
                self.elapsed += 1;
                // Vanilla finishes on the tick after the use time ran out; Dragonfly rejects a finish
                // before 1.61 s, so finishing exactly at 32 ticks would be early there.
                if self.elapsed > self.use_ticks {
                    self.phase = Phase::Confirming { left: CONFIRM_TICKS };
                    // Already completed by the server (BDS): the finish would name the stack it replaced.
                    return if now.consumed_since(&self.before) { UseTick::Nothing } else { UseTick::Finish };
                }
                let left = self.use_ticks - self.elapsed;
                if self.elapsed >= EFFECTS_AFTER && left > 0 && left.is_multiple_of(EFFECT_EVERY) {
                    return UseTick::Effect;
                }
            }
            Phase::Confirming { left } => {
                if now.consumed_since(&self.before) {
                    self.phase = Phase::Releasing { left: self.release_ticks, result: Ok(()) };
                } else if *left == 0 {
                    let result = Err(ActionError::Rejected("the server did not consume the item".into()));
                    self.phase = Phase::Releasing { left: 0, result };
                } else {
                    *left -= 1;
                }
            }
            Phase::Releasing { left: 0, result } => return UseTick::Done(result.clone()),
            Phase::Releasing { left, .. } => *left -= 1,
        }
        UseTick::Nothing
    }
}

/// `EntityEvent` EatingItem data: the item's network id and aux value (PowerNukkitX checks it).
pub(crate) fn eating_data(network_id: i32, metadata: u32) -> i32 {
    (network_id << 16) | (metadata as i32 & 0xffff)
}
