//! The server packets that steer the simulation: teleports, corrections, knockback, effects, attributes.

use acacia_client::proto::packets::{
    ChangeDimension, CorrectPlayerMovePrediction, CorrectPlayerMovePredictionPredictionType, MobEffect, MobEffectEventId, MovePlayer, MovementEffect, Respawn,
    SetEntityMotion, UpdateAttributes,
};
use acacia_client::proto::types::MovementEffectType;
use acacia_client::proto::{DecodeError, Packet, RawPacket};
use acacia_physics::constants::FREEZE_SPEED_MODIFIER;
use acacia_physics::Vec3;

use super::rewind::Correction;
use super::{Movement, VehicleCorrection, EYE_HEIGHT, SETBACK};
use crate::state::Me;

/// `Respawn` state carrying the position the player respawns at.
const RESPAWN_READY: u8 = 1;
/// `MobEffect` ids of the effects that change movement (Speed and Slowness come through the movement attribute).
const EFFECT_JUMP_BOOST: i32 = 8;
const EFFECT_LEVITATION: i32 = 24;
const EFFECT_SLOW_FALLING: i32 = 27;
const EFFECT_WEAVING: i32 = 33;

impl Movement {
    pub const PACKETS: &'static [u32] = &[
        MovePlayer::ID,
        CorrectPlayerMovePrediction::ID,
        SetEntityMotion::ID,
        Respawn::ID,
        MobEffect::ID,
        UpdateAttributes::ID,
        MovementEffect::ID,
        ChangeDimension::ID,
    ];

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
        if packet.id == ChangeDimension::ID {
            self.leave_dimension();
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
                        rotation = ?[c.rotation.x, c.rotation.z], yaw_velocity = ?c.angular_velocity, on_ground = c.on_ground, "vehicle correction"
                    );
                    self.vehicle_correction = Some(VehicleCorrection {
                        feet: [c.position.x, c.position.y, c.position.z],
                        delta: [c.delta.x, c.delta.y, c.delta.z],
                        on_ground: c.on_ground,
                        pitch_yaw: [c.rotation.x, c.rotation.z],
                        tick: c.tick,
                        yaw_velocity: c.angular_velocity,
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
                let ours = if c.tick == 0 { Some(st.pos) } else { self.history.position_at(c.tick) };
                if ours.is_none_or(|p| p.iter().zip(feet).map(|(a, b)| (a - b) * (a - b)).sum::<f32>() > SETBACK * SETBACK) {
                    self.last_setback = Some(self.tick);
                }
                if c.tick == 0 {
                    st.apply_correction(feet, delta, on_ground);
                } else {
                    self.history.schedule(c.tick, Correction { feet, delta, on_ground, teleport: false });
                }
                self.corrections += 1;
            }
            UpdateAttributes::ID if crate::state::leading_entity(packet) == Some(me.runtime_entity_id) => {
                let p: UpdateAttributes = packet.decode()?;
                {
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
}

fn feet(eye: &acacia_client::proto::types::Vec3f) -> Vec3 {
    [eye.x, eye.y - EYE_HEIGHT, eye.z]
}
