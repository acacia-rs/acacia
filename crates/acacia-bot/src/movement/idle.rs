//! Light idle mode for bots without physics: no collision, but the `PlayerAuthInput` a vanilla client
//! sends while standing still on the ground, every tick. The server is the source of position: its
//! teleports and corrections are adopted as they come (knockback is left to the server's correction);
//! the only own move is stepping off a vehicle ([`Idle::stepped_to`]). The bot may turn its head
//! ([`Idle::look_at`]) until the server moves it. Vanilla sends an input every tick even
//! when still; walking needs `BotConfig::physics`.

use acacia_client::proto::packets::PlayerAuthInput;
use acacia_physics::{Input, Outcome, TickOutput, Vec3};

use super::auth_input::{self, Edges};
use super::Controls;
use crate::state::PlayerState;

/// Velocity a player standing on the ground reports: one tick of gravity, cancelled by the floor.
const STANDING_DELTA: [f32; 3] = [0.0, -0.0784, 0.0];
/// The vanilla client's first `PlayerAuthInput` carries tick 4.
const FIRST_TICK: u64 = 4;

pub struct Idle {
    tick: u64,
    last_feet: Option<[f32; 3]>,
    /// `PlayerState::teleports` as of the last input; `None` adopts the count unacknowledged.
    last_teleports: Option<u32>,
    /// Own (yaw, pitch) replacing the server-known rotation until the server moves the player.
    look: Option<(f32, f32)>,
}

impl Default for Idle {
    fn default() -> Self {
        Self { tick: FIRST_TICK - 1, last_feet: None, last_teleports: Some(0), look: None }
    }
}

impl Idle {
    /// Continues the input tick count of a physics bot whose last input carried `tick`.
    pub(crate) fn continuing(tick: u64) -> Self {
        Self { tick, last_feet: None, last_teleports: None, look: None }
    }

    /// The tick of the last input built.
    pub(crate) fn last_tick(&self) -> u64 {
        self.tick
    }

    /// The (yaw, pitch) the next input reports, unless the server moves the player first.
    pub(crate) fn facing(&self, player: &PlayerState) -> (f32, f32) {
        self.look.unwrap_or((player.yaw, player.pitch))
    }

    /// Turns to face `target` from `eye` from the next input on.
    pub(crate) fn look_at(&mut self, eye: Vec3, target: Vec3) {
        let mut c = Controls::default();
        c.look_at(eye, target);
        self.look = Some((c.yaw, c.pitch));
    }

    /// The player moved itself to `feet` (left a vehicle): not a server move to acknowledge.
    pub(crate) fn stepped_to(&mut self, feet: [f32; 3]) {
        self.last_feet = Some(feet);
    }

    /// Advances one client tick; `None` while dead (the tick still counts, as in vanilla).
    /// `loaded`: past the loading screen.
    pub fn tick(&mut self, player: &PlayerState, loaded: bool) -> Option<PlayerAuthInput> {
        self.build(player, None, loaded)
    }

    /// [`Idle::tick`] looking in the given direction instead of its own.
    pub(crate) fn tick_facing(&mut self, player: &PlayerState, yaw: f32, pitch: f32, loaded: bool) -> Option<PlayerAuthInput> {
        self.build(player, Some((yaw, pitch)), loaded)
    }

    fn build(&mut self, player: &PlayerState, facing: Option<(f32, f32)>, loaded: bool) -> Option<PlayerAuthInput> {
        self.tick += 1;
        if !player.alive {
            return None;
        }
        let feet = [player.position.x, player.position.y, player.position.z];
        // Every own MovePlayer needs the ack even when it does not move us: until then BDS drops all
        // queued inputs and item-use transactions. Corrections are acked too.
        let teleported = self.last_feet.is_some_and(|last| last != feet)
            || self.last_teleports.is_some_and(|last| last != player.teleports);
        self.last_feet = Some(feet);
        self.last_teleports = Some(player.teleports);
        if teleported {
            tracing::debug!(?feet, teleports = player.teleports, "idle: acknowledging a server move");
            self.look = None;
        }
        let (yaw, pitch) = facing.unwrap_or_else(|| self.facing(player));
        let input = Input { yaw, pitch, ..Input::default() };
        let out = TickOutput {
            position: feet,
            eye_position: [feet[0], feet[1] + super::EYE_HEIGHT, feet[2]],
            delta: STANDING_DELTA,
            on_ground: true,
            vertical_collision: true,
            horizontal_collision: false,
            jumped: false,
            teleported,
            move_vector: [0.0, 0.0],
            outcome: if teleported { Outcome::Teleport } else { Outcome::Normal },
        };
        let edges = Edges { sprint: (false, false), sneak: (false, false), jump: (false, false), swim: (false, false), sneaking: false, sprinting: false, sprint_key: false };
        Some(auth_input::build(&input, &out, &edges, self.tick, loaded))
    }
}

#[cfg(test)]
mod tests;
