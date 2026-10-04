//! Client-side movement for server-authoritative servers: runs the physics simulation every 50 ms
//! and reports the result in `PlayerAuthInput`, obeying teleports, corrections and knockback.

mod auth_input;
pub(crate) mod equipment;
mod glide;
mod idle;
mod rewind;
#[cfg(test)]
mod tests;

pub use idle::Idle;

use acacia_client::proto::packets::{
    CorrectPlayerMovePrediction, CorrectPlayerMovePredictionPredictionType, MobEffect, MobEffectEventId, MovePlayer, MovementEffect, PlayerAuthInput,
    PlayerAuthInputBlockActionItem, Respawn, SetEntityMotion, UpdateAttributes,
};
use acacia_client::proto::types::{InputData, MovementEffectType};
use acacia_client::proto::{DecodeError, Packet, RawPacket};
use acacia_physics::constants::FREEZE_SPEED_MODIFIER;
use acacia_physics::{self as physics, Effects, Equipment, Input, PlayerState, Vec3, WorldView};

use crate::state::Me;
use rewind::{Correction, History};

/// Players' wire positions are eye positions this far above the feet.
pub(crate) const EYE_HEIGHT: f32 = crate::state::PlayerState::EYE_HEIGHT;
/// `Respawn` state carrying the position the player respawns at.
const RESPAWN_READY: u8 = 1;
/// `MobEffect` ids of the effects that change movement (Speed and Slowness come through the movement attribute).
const EFFECT_JUMP_BOOST: i32 = 8;
const EFFECT_LEVITATION: i32 = 24;
const EFFECT_SLOW_FALLING: i32 = 27;
const EFFECT_WEAVING: i32 = 33;
/// Window for double-tapping forward to sprint.
const DOUBLE_TAP_TICKS: u32 = 7;

/// The server's state of a vehicle the bot drives (`CorrectPlayerMovePrediction` type Vehicle).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct VehicleCorrection {
    pub feet: Vec3,
    pub delta: Vec3,
    pub on_ground: bool,
    pub pitch_yaw: [f32; 2],
}

/// What the bot is trying to do this tick; persists until changed.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Controls {
    /// +1 = walk forward, -1 = back. Sent as a keyboard key, so any nonzero value counts as ±1:
    /// BDS moves keyboard players at full speed whatever the move vector says.
    pub forward: f32,
    /// +1 = strafe left, -1 = right; ±1 like `forward`.
    pub strafe: f32,
    pub jump: bool,
    pub sneak: bool,
    pub sprint: bool,
    /// Keep an elytra open. Set in the air to start a glide; the simulation clears it when the glide ends
    /// (landing, water) or cannot start.
    pub glide: bool,
    /// Degrees; 0 faces +Z, wrapped to -180..=180 when sent.
    pub yaw: f32,
    /// Degrees; -90 looks up, clamped to -90..=90 when sent.
    pub pitch: f32,
}

impl Controls {
    pub fn stop(&mut self) {
        (self.forward, self.strafe, self.jump, self.sprint) = (0.0, 0.0, false, false);
    }

    /// Turns to face `target` from `eye` (both world positions).
    pub fn look_at(&mut self, eye: Vec3, target: Vec3) {
        let [dx, dy, dz] = [target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]];
        self.yaw = (-dx).atan2(dz).to_degrees();
        self.pitch = (-dy).atan2((dx * dx + dz * dz).sqrt()).to_degrees();
    }
}

/// Adds queued block actions (server-authoritative breaking) to a tick's input.
pub(crate) fn attach_actions(input: &mut PlayerAuthInput, actions: Vec<PlayerAuthInputBlockActionItem>) {
    if !actions.is_empty() {
        input.input_data.push(InputData::BlockAction);
        input.block_action = Some(actions);
    }
}

pub struct Movement {
    pub controls: Controls,
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
    /// Ticks left in which pressing forward again starts a sprint (double tap).
    sprint_trigger: u32,
    prev_impulse: bool,
    /// The sprint was started by a double tap, so it outlives the (unheld) sprint key.
    tapped_sprint: bool,
    effects: Effects,
    equipment: Equipment,
    history: History,
    /// Replay only: the recorded `WantDown` (bot traces before 2026-10-02 never sent it), else it follows sneak.
    pub(crate) recorded_want_down: Option<bool>,
}

impl Movement {
    pub const PACKETS: &'static [u32] = &[
        MovePlayer::ID,
        CorrectPlayerMovePrediction::ID,
        SetEntityMotion::ID,
        Respawn::ID,
        MobEffect::ID,
        UpdateAttributes::ID,
        MovementEffect::ID,
    ];

    pub fn new() -> Self {
        Self {
            controls: Controls::default(),
            pending_actions: Vec::new(),
            corrections: 0,
            teleports: 0,
            vehicle_corrections: 0,
            vehicle_correction: None,
            last_vehicle_delta: None,
            spawn_wait: 0,
            using_item: false,
            elytra: false,
            physics: None,
            tick: 0,
            prev_jump: false,
            ack_teleport: false,
            current_on_landing: false,
            sprint_trigger: 0,
            recorded_want_down: None,
            prev_impulse: false,
            tapped_sprint: false,
            effects: Effects::default(),
            equipment: Equipment::default(),
            history: History::default(),
        }
    }

    /// Starts simulating from the spawn position (feet).
    pub fn start(&mut self, feet: Vec3, yaw: f32, pitch: f32) {
        let mut st = PlayerState::new(feet);
        st.effects = self.effects;
        self.physics = Some(st);
        (self.controls.yaw, self.controls.pitch) = (yaw, pitch);
    }

    /// Replaces the simulated position and velocity as of the end of input `tick`.
    pub(crate) fn resync(&mut self, tick: u64, feet: Vec3, delta: Vec3) {
        if let Some(st) = &mut self.physics {
            st.apply_correction(feet, delta, st.on_ground);
            self.tick = tick;
        }
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

    /// Numbers the next simulated tick `tick`: a real client's tick counter can skip (replay only).
    pub(crate) fn align_tick(&mut self, tick: u64) {
        self.tick = tick.saturating_sub(1);
    }

    /// Tick of the last `PlayerAuthInput` built.
    pub(crate) fn input_tick(&self) -> u64 {
        self.tick
    }

    pub fn is_started(&self) -> bool {
        self.physics.is_some()
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

    /// Tracks the effects that change movement; they also arrive at login, before movement starts.
    fn apply_effect(&mut self, p: &MobEffect) {
        let level = (p.event_id != MobEffectEventId::Remove).then_some(p.amplifier);
        tracing::trace!(our_tick = self.tick, server_tick = p.tick, id = p.effect_id, event = ?p.event_id, amplifier = p.amplifier, "effect");
        match p.effect_id {
            EFFECT_JUMP_BOOST => self.effects.jump_boost = level,
            EFFECT_LEVITATION => self.effects.levitation = level,
            EFFECT_SLOW_FALLING => self.effects.slow_falling = level.is_some(),
            EFFECT_WEAVING => self.effects.weaving = level.is_some(),
            _ => return,
        }
        tracing::debug!(our_tick = self.tick, server_tick = p.tick, effects = ?self.effects, "movement effects");
        if let Some(st) = &mut self.physics {
            st.effects = self.effects;
            self.history.effects(p.tick, self.effects);
        }
    }

    pub fn apply(&mut self, packet: &RawPacket, me: &Me) -> Result<(), DecodeError> {
        if packet.id == MobEffect::ID {
            let p: MobEffect = packet.decode()?;
            if p.runtime_entity_id == me.runtime_entity_id {
                self.apply_effect(&p);
            }
            return Ok(());
        }
        let Some(st) = &mut self.physics else {
            // A teleport before movement starts still needs HandledTeleport, or BDS drops our inputs.
            self.ack_teleport |= packet.id == MovePlayer::ID && u64::from(packet.decode::<MovePlayer>()?.runtime_id) == me.runtime_entity_id;
            return Ok(());
        };
        match packet.id {
            MovePlayer::ID => {
                let m: MovePlayer = packet.decode()?;
                if u64::from(m.runtime_id) == me.runtime_entity_id {
                    tracing::debug!(our_tick = self.tick, server_tick = m.tick, mode = ?m.mode, pos = ?feet(&m.position), "teleport");
                    let feet = feet(&m.position);
                    // BDS holds the player at the target through tick T+1 and simulates on from T+2
                    // (docs/DESIGN.md "Movement vs the servers"); Geyser stamps no tick, and Boar
                    // holds the player still on the acknowledging tick.
                    if m.tick == 0 || m.tick >= self.tick {
                        st.queue_teleport(feet);
                        // Boar makes a teleported player airborne until it lands again; BDS keeps on_ground.
                        if m.tick == 0 {
                            st.on_ground = false;
                        }
                        self.current_on_landing = m.tick != 0;
                    } else {
                        let on_ground = st.on_ground;
                        self.history.schedule(m.tick + 1, Correction { feet, delta: [0.0; 3], on_ground, teleport: true });
                        self.ack_teleport = true;
                    }
                    self.teleports += 1;
                    (self.controls.yaw, self.controls.pitch) = (m.yaw, m.pitch);
                }
            }
            CorrectPlayerMovePrediction::ID => {
                let c: CorrectPlayerMovePrediction = packet.decode()?;
                // Corrections of a driven boat or horse go to the vehicle simulation (riding/horse.rs);
                // applied to the player they sank it 1.62 below the vehicle.
                if c.prediction_type == CorrectPlayerMovePredictionPredictionType::Vehicle {
                    tracing::debug!(
                        server_tick = c.tick, server = ?[c.position.x, c.position.y, c.position.z], delta = ?[c.delta.x, c.delta.y, c.delta.z],
                        rotation = ?[c.rotation.x, c.rotation.z], on_ground = c.on_ground, "vehicle correction"
                    );
                    self.vehicle_correction = Some(VehicleCorrection {
                        feet: [c.position.x, c.position.y, c.position.z],
                        delta: [c.delta.x, c.delta.y, c.delta.z],
                        on_ground: c.on_ground,
                        pitch_yaw: [c.rotation.x, c.rotation.z],
                    });
                    self.vehicle_corrections += 1;
                    self.last_vehicle_delta = Some([c.delta.x, c.delta.y, c.delta.z]);
                    return Ok(());
                }
                tracing::debug!(
                    our_tick = self.tick, server_tick = c.tick, ours = ?st.pos, server = ?feet(&c.position),
                    delta = ?[c.delta.x, c.delta.y, c.delta.z], on_ground = c.on_ground, "movement correction"
                );
                let (feet, delta, on_ground) = (feet(&c.position), [c.delta.x, c.delta.y, c.delta.z], c.on_ground);
                if c.tick == 0 {
                    st.apply_correction(feet, delta, on_ground);
                } else {
                    self.history.schedule(c.tick, Correction { feet, delta, on_ground, teleport: false });
                }
                self.corrections += 1;
            }
            UpdateAttributes::ID => {
                let p: UpdateAttributes = packet.decode()?;
                if p.runtime_entity_id == me.runtime_entity_id {
                    // Speed and Slowness arrive as modifiers on the movement attribute, and so do the sprint
                    // boost (multiplying) and freezing (adding), which the simulation applies itself.
                    for a in p.attributes.iter().filter(|a| a.name == "minecraft:movement") {
                        let amount = |name: &str| a.modifiers.iter().find(|m| m.name == name).map_or(0.0, |m| m.amount);
                        let value = a.current /(1.0 + amount("Sprinting speed boost")) - amount("Freeze effect");
                        let freeze = amount("Freeze effect") / FREEZE_SPEED_MODIFIER;
                        tracing::debug!(our_tick = self.tick, server_tick = p.tick, value, freeze, "movement attribute");
                        st.set_movement_attribute(value);
                        self.history.movement_attribute(p.tick, value);
                        // Our freeze steps per input tick, the server's per world tick: follow the server's.
                        let change = self.history.freeze(p.tick, freeze);
                        st.set_freeze((st.freeze + change).clamp(0.0, 1.0));
                        if p.tick >= self.tick {
                            st.server_freeze = Some(freeze);
                        }
                    }
                }
            }
            Respawn::ID => {
                let r: Respawn = packet.decode()?;
                if r.state == RESPAWN_READY {
                    st.queue_teleport(feet(&r.position));
                    self.teleports += 1;
                }
            }
            MovementEffect::ID => {
                let e: MovementEffect = packet.decode()?;
                if e.runtime_id == me.runtime_entity_id && e.effect_type == MovementEffectType::GLIDEBOOST {
                    tracing::debug!(our_tick = self.tick, server_tick = e.tick, duration = e.effect_duration, "glide boost");
                    self.server_glide_boost(e.effect_duration, e.tick);
                }
            }
            SetEntityMotion::ID => {
                let m: SetEntityMotion = packet.decode()?;
                if m.runtime_entity_id == me.runtime_entity_id {
                    tracing::debug!(our_tick = self.tick, server_tick = m.tick, velocity = ?m.velocity, "knockback");
                    // Knockback stamped with tick K moves the player on input tick K+1.
                    let velocity = [m.velocity.x, m.velocity.y, m.velocity.z];
                    if m.tick == 0 || m.tick >= self.tick || !self.history.knockback(m.tick + 1, velocity) {
                        st.queue_knockback(velocity);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Simulates one tick and returns the `PlayerAuthInput` describing it.
    pub fn tick(&mut self, world: &impl WorldView) -> Option<PlayerAuthInput> {
        let st = self.physics.as_mut()?;
        let c = self.controls;
        // Simulated with the rotation as sent, so a replay of the packet reproduces the tick exactly.
        let (yaw, pitch) = auth_input::wire_rotation(c.yaw, c.pitch);
        let replayed = self.history.apply(st, world);
        if replayed > 0 {
            tracing::debug!(tick = self.tick, replayed, "rewound");
        }
        st.equipment = self.equipment;
        // Pressing forward again within DOUBLE_TAP_TICKS of the last press sprints: BDS runs this client
        // rule on the raw key flags, so the simulation must too. A press is the forward impulse reaching
        // full strength: sneaking scales it down on land (not in water, where it means sink), so releasing
        // sneak there with forward held counts. Presses while sneaking never count, and only presses on
        // the ground or in water do (vanilla client captures, strict BDS fuzz). A press with the sprint key
        // held starts no tap window (as in Java's `aiStep`).
        let forward = key(c.forward) > 0.0;
        let in_water = physics::touching_water(st, world);
        let sneaking = c.sneak || st.sneaking;
        let impulse = forward && !(sneaking && !in_water);
        self.sprint_trigger = self.sprint_trigger.saturating_sub(1);
        let mut double_tap = false;
        let grounded = st.on_ground || in_water;
        if impulse && !self.prev_impulse && !sneaking && grounded {
            if self.sprint_trigger > 0 {
                double_tap = true;
            } else if !c.sprint {
                self.sprint_trigger = DOUBLE_TAP_TICKS;
            }
        }
        self.prev_impulse = impulse;
        self.tapped_sprint = (self.tapped_sprint && st.sprinting || double_tap) && !c.sprint;
        let input = Input {
            move_vector: keys(c.strafe, c.forward),
            yaw,
            pitch,
            jump: c.jump,
            sneak: c.sneak,
            want_down: self.recorded_want_down.unwrap_or(c.sneak),
            // Strict BDS 1.26.52 ends a key sprint when the key is released (fuzz: 94% of such ticks were
            // corrected); a double-tap sprint has no key and lasts while forward is held.
            sprint: c.sprint || double_tap || (self.tapped_sprint && st.sprinting && forward),
            using_item: self.using_item,
            glide: c.glide,
            ..Input::default()
        };
        let (was_sprinting, was_sneaking, was_swimming, was_gliding) = (st.sprinting, st.sneaking, st.swimming, st.gliding);
        let knockback = st.knockback;
        st.equipment.elytra = self.elytra;
        let mut out = physics::tick(st, &input, world);
        if !st.gliding {
            self.controls.glide = false;
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
            jump: (c.jump && !self.prev_jump, !c.jump && self.prev_jump),
            swim: (st.swimming && !was_swimming, !st.swimming && was_swimming),
            glide: (st.gliding && !was_gliding, !st.gliding && was_gliding),
            sneaking: st.sneaking,
            sprinting: st.sprinting,
            sprint_key: c.sprint,
        };
        self.prev_jump = c.jump;
        self.tick += 1;
        self.history.record(self.tick, input, knockback, st);
        // Movement starts only after the bot has left the loading screen (bot.rs).
        let packet = auth_input::build(&input, &out, &edges, self.tick, true);
        tracing::trace!(tick = self.tick, forward = c.forward, sprint = c.sprint, sprinting = st.sprinting, swimming = st.swimming, speed = st.movement_speed, freeze = st.freeze, yaw = c.yaw, pitch = c.pitch,
            pos = ?out.position, delta = ?out.delta, teleported = out.teleported, flags = ?packet.input_data, "auth input");
        Some(packet)
    }
}

impl Default for Movement {
    fn default() -> Self { Self::new() }
}

/// The move vector of held keys: the client normalizes diagonals, so they move at the same 0.98 of full
/// speed as one key does (the simulation's own cap would let them reach 1.0).
fn keys(strafe: f32, forward: f32) -> [f32; 2] {
    let [s, f] = [key(strafe), key(forward)];
    if s != 0.0 && f != 0.0 { [s * std::f32::consts::FRAC_1_SQRT_2, f * std::f32::consts::FRAC_1_SQRT_2] } else { [s, f] }
}

/// A movement axis as a key press: -1, 0 or 1.
fn key(axis: f32) -> f32 {
    if axis > 0.0 { 1.0 } else if axis < 0.0 { -1.0 } else { 0.0 }
}

fn feet(eye: &acacia_client::proto::types::Vec3f) -> Vec3 {
    [eye.x, eye.y - EYE_HEIGHT, eye.z]
}
