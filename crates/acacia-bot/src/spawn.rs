//! What the vanilla client sends between spawning and settling in, with its timing (docs/research/
//! vanilla-actions-2026-10-02.md "Join / spawn"): loading screen start; ~0.6 s later mouse-over-nothing, its emote
//! list and its first input; ~1.75 s of inputs later loading screen end with
//! SetLocalPlayerAsInitialised; ~1.3 s after that the "back in world" packets.

/// Ticks from spawn to the first input (vanilla: 0.5-0.7 s).
const FIRST_INPUT_TICK: u32 = 12;
/// Inputs sent before leaving the loading screen (vanilla: 17-19 over 1.6 s, hitches included).
const LOADING_INPUTS: u32 = 18;
/// Inputs reporting PlayMode Normal before Screen (vanilla switches 1-2 inputs before the loading end).
const NORMAL_MODE_INPUTS: u32 = 16;
/// Inputs from SetLocalPlayerAsInitialised to the "back in world" packets (vanilla: 17, over 1.2-1.4 s).
const SETTLE_TICKS: u32 = 17;
/// Frame hitches while the world loads: (after input n, pause range in ms), each followed by two
/// inputs in one batch (capture gaps: 404/325 and 700/187 ms after inputs 9-12; settle time implies one more).
const STALLS: [(u32, u64, u64); 3] = [(9, 400, 700), (12, 190, 330), (LOADING_INPUTS + 6, 300, 450)];

use std::time::Duration;

/// `ServerboundLoadingScreen` types.
pub(crate) const LOADING_SCREEN_START: i32 = 1;
pub(crate) const LOADING_SCREEN_END: i32 = 2;

/// The emotes a default character has equipped (vanilla capture).
pub(crate) const DEFAULT_EMOTES: [&str; 4] = [
    "17428c4c-3813-4ea1-b3a9-d6a32f83afca",
    "ce5c0300-7f03-455d-aaf1-352e4927b54d",
    "9a469a61-c83b-4ba9-b507-bdbe64430582",
    "4c8ae710-df2e-47cd-814d-cc7bf21a3d67",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpawnPacket {
    LoadingScreenStart,
    /// `Interact(MouseOverEntity, 0)`: the cursor over nothing.
    MouseOverNothing,
    EmoteList,
    LoadingScreenEnd,
    Initialized,
    /// `PlayerAction(Respawn)` + cleared aim assist (`Client::respawn_done`).
    RespawnDone,
}

/// One tick of the sequence: packets before this tick's input, whether to send an input, packets after.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct SpawnTick {
    pub before: Vec<SpawnPacket>,
    pub input: bool,
    pub after: Vec<SpawnPacket>,
    /// Hold the next tick back this long (`Ticker::stall`).
    pub stall: Option<Duration>,
}

pub(crate) struct SpawnSequence {
    ticks: u32,
    inputs: u32,
    initialized_at: Option<u32>,
    rng: u64,
}

impl Default for SpawnSequence {
    fn default() -> Self {
        Self::new(crate::cadence::seed())
    }
}

impl SpawnSequence {
    pub fn new(seed: u64) -> Self {
        Self { ticks: 0, inputs: 0, initialized_at: None, rng: seed }
    }

    /// Inputs report PlayMode Screen from here on (before the loading screen ends, as vanilla does).
    pub fn screen_mode(&self) -> bool {
        self.inputs > NORMAL_MODE_INPUTS
    }

    pub fn tick(&mut self) -> SpawnTick {
        use SpawnPacket::*;
        self.ticks += 1;
        let mut t = SpawnTick::default();
        if self.ticks == 1 {
            t.before.push(LoadingScreenStart);
        }
        if self.ticks < FIRST_INPUT_TICK {
            return t;
        }
        if self.ticks == FIRST_INPUT_TICK {
            t.before.extend([MouseOverNothing, EmoteList]);
        }
        t.input = true;
        self.inputs += 1;
        if self.inputs == LOADING_INPUTS {
            t.after.extend([LoadingScreenEnd, Initialized]);
            self.initialized_at = Some(self.ticks);
        }
        if self.initialized_at.is_some_and(|at| self.ticks == at + SETTLE_TICKS) {
            t.after.push(RespawnDone);
        }
        if let Some(&(_, lo, hi)) = STALLS.iter().find(|s| s.0 == self.inputs) {
            t.stall = Some(Duration::from_millis(lo + crate::cadence::splitmix(&mut self.rng) % (hi - lo + 1)));
        }
        t
    }
}

/// Sends one step of the spawn sequence for the player `runtime_entity_id`.
pub(crate) fn send(client: &acacia_client::Client, runtime_entity_id: u64, packet: SpawnPacket) {
    use acacia_client::proto::manual::Uuid;
    use acacia_client::proto::packets::{EmoteList, ServerboundLoadingScreen, SetLocalPlayerAsInitialized};
    let screen = |r#type| ServerboundLoadingScreen { r#type, loading_screen_id: None };
    match packet {
        SpawnPacket::LoadingScreenStart => client.send(&screen(LOADING_SCREEN_START)),
        SpawnPacket::LoadingScreenEnd => client.send(&screen(LOADING_SCREEN_END)),
        SpawnPacket::MouseOverNothing => client.send(&crate::interact::wire::mouse_over_nothing()),
        SpawnPacket::EmoteList => client.send(&EmoteList {
            player_id: runtime_entity_id,
            emote_pieces: DEFAULT_EMOTES.iter().map(|id| id.parse::<Uuid>().expect("constant UUIDs")).collect(),
        }),
        SpawnPacket::Initialized => client.send(&SetLocalPlayerAsInitialized { runtime_entity_id }),
        SpawnPacket::RespawnDone => client.respawn_done(),
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use SpawnPacket::*;

    #[test]
    fn follows_the_vanilla_order() {
        let mut seq = SpawnSequence::default();
        let total = FIRST_INPUT_TICK + LOADING_INPUTS + SETTLE_TICKS + 5;
        let ticks: Vec<SpawnTick> = (0..total).map(|_| seq.tick()).collect();
        let sent: Vec<SpawnPacket> = ticks.iter().flat_map(|t| t.before.iter().chain(&t.after).copied()).collect();
        assert_eq!(sent, [LoadingScreenStart, MouseOverNothing, EmoteList, LoadingScreenEnd, Initialized, RespawnDone]);
        let first_input = ticks.iter().position(|t| t.input).unwrap() as u32 + 1;
        assert_eq!(first_input, FIRST_INPUT_TICK);
        assert_eq!(ticks.iter().filter(|t| t.input).count() as u32, total - (FIRST_INPUT_TICK - 1), "an input every tick from the first");
        assert!(seq.screen_mode());
        let stalls: Vec<u64> = ticks.iter().filter_map(|t| t.stall).map(|d| d.as_millis() as u64).collect();
        assert_eq!(stalls.len(), STALLS.len());
        assert!(stalls.iter().zip(STALLS).all(|(&ms, (_, lo, hi))| (lo..=hi).contains(&ms)), "{stalls:?}");
    }
}
