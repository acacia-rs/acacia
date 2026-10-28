//! Client-side movement for server-authoritative servers: runs the physics simulation every 50 ms
//! and reports the result in `PlayerAuthInput`, obeying teleports, corrections and knockback.

mod auth_input;
mod controls;
pub(crate) mod equipment;
mod flight;
mod fork;
mod glide;
mod idle;
mod keys;
mod lifecycle;
mod packets;
mod rewind;
mod rotation_noise;
#[cfg(test)]
mod tests;

pub use controls::Controls;
pub(crate) use controls::{attach_actions, VehicleCorrection};
pub use fork::Rollout;
pub use idle::Idle;

use acacia_client::proto::packets::{PlayerAuthInput, PlayerAuthInputBlockActionItem, PlayerAuthInputInputMode, PlayerAuthInputInteractionModel};
use acacia_physics::{self as physics, Effects, Equipment, PlayerState, Vec3, WorldView};

use keys::KeyLatch;
use rewind::History;
pub(crate) use rotation_noise::RotationNoise;

/// Players' wire positions are eye positions this far above the feet.
pub(crate) const EYE_HEIGHT: f32 = crate::state::PlayerState::EYE_HEIGHT;

pub struct Movement {
    pub controls: Controls,
    /// [`crate::BotConfig::mouse_input`].
    pub(crate) mouse_input: bool,
    /// Block actions to send with the next tick's `PlayerAuthInput`.
    pub(crate) pending_actions: Vec<PlayerAuthInputBlockActionItem>,
    /// Server corrections received: each one means the simulation disagreed with the server.
    pub corrections: u32,
    /// Server teleports received (including setbacks).
    pub teleports: u32,
    /// Corrections of a vehicle the bot drives: each one means the vehicle simulation disagreed.
    pub vehicle_corrections: u32,
    /// The latest vehicle correction, for the vehicle simulation to take.
    pub(crate) vehicle_correction: Option<VehicleCorrection>,
    /// The server's vehicle motion in the latest vehicle correction.
    pub last_vehicle_delta: Option<Vec3>,
    /// Ticks spent with the spawn chunk loaded but movement not yet started.
    pub(crate) spawn_wait: u32,
    /// The movement attribute of the simulation a dimension change stopped: the next one arrives with it.
    carried_attribute: Option<f32>,
    /// Holding "use" on an item (eating, drinking), which slows movement.
    pub(crate) using_item: bool,
    /// An elytra is worn (set by the bot every tick).
    pub(crate) elytra: bool,
    physics: Option<PlayerState>,
    tick: u64,
    prev_jump: bool,
    /// A teleport superseded by its correction still needs `HandledTeleport` sent.
    ack_teleport: bool,
    /// The queued teleport is a BDS one, which applies liquid currents on its hold tick.
    current_on_landing: bool,
    keys: KeyLatch,
    effects: Effects,
    equipment: Equipment,
    /// The flying flag of the server's last `UpdateAbilities` (see `set_abilities`).
    server_flying: Option<bool>,
    history: History,
    /// Solid entity boxes for the next tick and the ones seen last ([`Movement::set_solids`]).
    solids: Vec<physics::Aabb>,
    seen_solids: Vec<physics::Aabb>,
    /// Replay only: the recorded `WantDown` (bot traces before 2026-10-02 never sent it), else it follows sneak.
    pub(crate) recorded_want_down: Option<bool>,
    last_setback: Option<u64>,
    /// Live bots only: sims, tests and replays send `controls` rotations exactly.
    pub(crate) rotation_noise: Option<RotationNoise>,
    /// The next inputs carry no movement keys: Oomph bans moving on both sides of an `ItemStackRequest`.
    pub(crate) still_ticks: u8,
}

/// A correction moving the bot further than this (blocks) is a setback: paths and actions pause.
pub const SETBACK: f32 = 0.05;

impl Movement {
    pub fn new() -> Self {
        Self {
            controls: Controls::default(),
            mouse_input: false,
            pending_actions: Vec::new(),
            corrections: 0,
            teleports: 0,
            vehicle_corrections: 0,
            vehicle_correction: None,
            last_vehicle_delta: None,
            spawn_wait: 0,
            carried_attribute: None,
            using_item: false,
            elytra: false,
            physics: None,
            tick: 0,
            prev_jump: false,
            ack_teleport: false,
            current_on_landing: false,
            recorded_want_down: None,
            keys: KeyLatch::default(),
            effects: Effects::default(),
            equipment: Equipment::default(),
            server_flying: None,
            history: History::default(),
            solids: Vec::new(),
            seen_solids: Vec::new(),
            last_setback: None,
            rotation_noise: None,
            still_ticks: 0,
        }
    }

    /// Ticks since a correction moved the bot more than [`SETBACK`] (ulp-sized ones are not setbacks).
    pub fn setback_age(&self) -> Option<u64> {
        self.last_setback.map(|t| self.tick.saturating_sub(t))
    }

    /// The worn armour for the ticks from here on (see `equipment::worn`); true when it changed.
    pub(crate) fn set_equipment(&mut self, equipment: Equipment) -> bool {
        let changed = equipment != self.equipment;
        if changed {
            tracing::debug!(our_tick = self.tick, ?equipment, "equipment");
            self.equipment = equipment;
        }
        changed
    }

    /// The worn armour the simulation uses.
    pub fn equipment(&self) -> Equipment {
        self.equipment
    }

    /// The freeze the simulation holds, and the one the server has reported for input `tick`.
    pub(crate) fn freeze_at(&self, tick: u64) -> (f32, Option<f32>) {
        (self.physics.as_ref().map_or(0.0, |p| p.freeze), self.history.server_freeze(tick))
    }

    /// Simulated feet position.
    pub fn position(&self) -> Option<Vec3> {
        self.physics.as_ref().map(|p| p.pos)
    }

    /// The simulation's on-ground flag (false before movement starts).
    pub fn on_ground(&self) -> bool {
        self.physics.as_ref().is_some_and(|p| p.on_ground)
    }

    pub fn eye_position(&self) -> Option<Vec3> {
        self.physics.as_ref().map(PlayerState::eye_position)
    }

    /// The simulation is sprinting.
    pub fn sprinting(&self) -> bool {
        self.physics.as_ref().is_some_and(|p| p.sprinting)
    }

    /// The simulation is sneaking.
    pub fn sneaking(&self) -> bool {
        self.physics.as_ref().is_some_and(|p| p.sneaking)
    }

    /// Collision boxes of the solid entities as tracked now (world space; see [`physics::WithEntities`]). They
    /// take effect a tick late: strict BDS collides a tick with the shapes it fetched on the tick before.
    pub fn set_solids(&mut self, boxes: Vec<physics::Aabb>) {
        self.solids = std::mem::replace(&mut self.seen_solids, boxes);
    }

    /// Simulates one tick and returns the `PlayerAuthInput` describing it.
    pub fn tick(&mut self, world: &impl WorldView) -> Option<PlayerAuthInput> {
        let st = self.physics.as_mut()?;
        let mut c = self.controls;
        if let Some(noise) = self.rotation_noise.as_mut() {
            (c.yaw, c.pitch) = noise.apply(c.yaw, c.pitch);
        }
        if self.still_ticks > 0 {
            self.still_ticks -= 1;
            (c.forward, c.strafe) = (0.0, 0.0);
        }
        let replayed = self.history.apply(st, world);
        if replayed > 0 {
            tracing::debug!(tick = self.tick, replayed, "rewound");
        }
        let solids = std::mem::take(&mut self.solids);
        let world = &physics::WithEntities { world, boxes: &solids };
        st.equipment = self.equipment;
        self.keys.mouse_input = self.mouse_input;
        let input = self.keys.advance(&c, st, world, self.using_item, self.recorded_want_down.unwrap_or(c.sneak));
        let jump = input.jump;
        let (was_sprinting, was_sneaking, was_swimming, was_gliding) = (st.sprinting, st.sneaking, st.swimming, st.gliding);
        let was_flying = st.flight.flying;
        let knockback = st.knockback;
        st.equipment.elytra = self.elytra;
        let mut out = physics::tick(st, &input, world);
        if !st.gliding {
            self.controls.glide = false;
        }
        if was_flying != st.flight.flying {
            self.controls.fly = st.flight.flying;
        }
        if out.teleported && std::mem::take(&mut self.current_on_landing) {
            physics::apply_current(st, world);
            out.delta = st.vel;
        }
        out.teleported |= std::mem::take(&mut self.ack_teleport);
        let edges = auth_input::Edges {
            sprint: (
                st.sprinting && !was_sprinting || st.sprint_start_cancelled,
                !st.sprinting && was_sprinting || st.sprint_start_cancelled,
            ),
            sneak: (st.sneaking && !was_sneaking, !st.sneaking && was_sneaking),
            jump: (jump && !self.prev_jump, !jump && self.prev_jump),
            swim: (st.swimming && !was_swimming, !st.swimming && was_swimming),
            glide: (st.gliding && !was_gliding, !st.gliding && was_gliding),
            fly: (st.flight.flying && !was_flying, !st.flight.flying && was_flying),
            sneaking: st.sneaking,
            sprinting: st.sprinting,
            sprint_key: c.sprint,
        };
        self.prev_jump = jump;
        self.tick += 1;
        self.history.record(self.tick, input, knockback, st, solids);
        // Movement starts only after the bot has left the loading screen (bot.rs).
        let mut packet = auth_input::build(&input, &out, &edges, self.tick, true);
        if self.mouse_input {
            packet.input_mode = PlayerAuthInputInputMode::Mouse;
            packet.interaction_model = PlayerAuthInputInteractionModel::Crosshair;
        }
        tracing::trace!(tick = self.tick, forward = c.forward, sprint = c.sprint, sprinting = st.sprinting, swimming = st.swimming, speed = st.movement_speed, freeze = st.freeze, yaw = c.yaw, pitch = c.pitch,
            pos = ?out.position, delta = ?out.delta, teleported = out.teleported, flags = ?packet.input_data, "auth input");
        Some(packet)
    }
}

impl Default for Movement {
    fn default() -> Self { Self::new() }
}
