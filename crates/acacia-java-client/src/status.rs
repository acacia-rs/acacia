//! The local player's server-set state that its physics reads (effects, food, the dimension's lava), and death and
//! respawn as `ClientPacketListener.handleRespawn` rebuilds the player.

use acacia_java_physics::{Effects, Player};
use acacia_java_proto::{Error as WireError, VarInt, Wire, ids, read_varint};

use crate::frame::Error;
use crate::join::Client;
use crate::movement::Movement;

use ids::play::{clientbound as from, serverbound as to};

/// `ServerboundClientCommandPacket.Action.PERFORM_RESPAWN`.
const PERFORM_RESPAWN: i32 = 0;
/// `ClientboundRespawnPacket.KEEP_ATTRIBUTE_MODIFIERS` and `KEEP_ENTITY_DATA`.
const KEEP_ATTRIBUTES: u8 = 1;
const KEEP_ENTITY_DATA: u8 = 2;
/// `FoodData.hasEnoughFood`: sprinting needs more than this.
const SPRINT_FOOD: i32 = 6;
/// `EnvironmentAttributes.FAST_LAVA` is the Nether dimension type's; named dimensions stand in for their types.
const FAST_LAVA_DIMENSION: &str = "minecraft:the_nether";

/// `mob_effect` registry ids of the effects that change movement.
mod effect {
    pub const SPEED: i32 = 0;
    pub const SLOWNESS: i32 = 1;
    pub const JUMP_BOOST: i32 = 7;
    pub const BLINDNESS: i32 = 14;
    pub const LEVITATION: i32 = 24;
    pub const SLOW_FALLING: i32 = 27;
    pub const DOLPHINS_GRACE: i32 = 29;
    pub const WEAVING: i32 = 36;
}

/// Sets (`Some(amplifier)`) or clears one effect; `false` for effects movement ignores.
fn set_effect(effects: &mut Effects, id: i32, amplifier: Option<u8>) -> bool {
    let on = amplifier.is_some();
    match id {
        effect::SPEED => effects.speed = amplifier,
        effect::SLOWNESS => effects.slowness = amplifier,
        effect::JUMP_BOOST => effects.jump_boost = amplifier,
        effect::LEVITATION => effects.levitation = amplifier,
        effect::BLINDNESS => effects.blindness = on,
        effect::SLOW_FALLING => effects.slow_falling = on,
        effect::DOLPHINS_GRACE => effects.dolphins_grace = on,
        effect::WEAVING => effects.weaving = on,
        _ => return false,
    }
    true
}

impl Movement {
    pub(crate) fn apply_status(&mut self, id: i32, mut input: &[u8], player_id: i32) -> Result<(), WireError> {
        let input = &mut input;
        match id {
            from::SET_HEALTH => {
                self.health = f32::read(input)?;
                self.food = read_varint(input)?;
            }
            from::UPDATE_MOB_EFFECT if read_varint(input)? == player_id => {
                let effect = read_varint(input)?;
                let amplifier = u8::try_from(read_varint(input)?).unwrap_or(u8::MAX);
                set_effect(&mut self.player.effects, effect, Some(amplifier));
            }
            from::REMOVE_MOB_EFFECT if read_varint(input)? == player_id => {
                set_effect(&mut self.player.effects, read_varint(input)?, None);
            }
            from::RESPAWN => self.respawned(input.last().copied().unwrap_or(0)),
            _ => {}
        }
        self.player.has_food = self.food > SPRINT_FOOD || self.player.may_fly;
        Ok(())
    }

    /// A new player object; `keep` is the packet's `dataToKeep`. The seat is the riding module's.
    fn respawned(&mut self, keep: u8) {
        let pos = self.player.pos;
        let old = std::mem::replace(&mut self.player, Player::at(pos));
        if keep & KEEP_ENTITY_DATA != 0 {
            (self.player.delta, self.player.yaw, self.player.pitch) = (old.delta, old.yaw, old.pitch);
        } else {
            self.player.yaw = -180.0;
        }
        if keep & KEEP_ATTRIBUTES != 0 {
            self.player.attributes = old.attributes.clone();
        } else {
            self.player.attributes.keep_bases(&old.attributes);
        }
        let kept = keep & KEEP_ENTITY_DATA != 0;
        self.reset_sent(kept.then_some((old.keys, old.sprinting)));
    }

    /// Dead players take no input (`LivingEntity.isImmobile`) but still fall and report where they are.
    pub fn dead(&self) -> bool {
        self.health <= 0.0
    }
}

impl Client {
    /// Presses "respawn" on the death screen.
    pub fn respawn(&self) -> Result<(), Error> {
        self.send(to::CLIENT_COMMAND, &VarInt(PERFORM_RESPAWN))
    }

    /// Before the player's tick: the world's lava speed.
    pub(crate) fn update_status(&mut self) {
        self.movement.player.fast_lava = self.world.dimension == FAST_LAVA_DIMENSION;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_on_and_off() {
        let mut m = Movement::default();
        let mut body = Vec::new();
        for v in [5, effect::JUMP_BOOST, 1, 200] {
            VarInt(v).write(&mut body);
        }
        0u8.write(&mut body);
        m.apply_status(from::UPDATE_MOB_EFFECT, &body, 5).unwrap();
        m.apply_status(from::UPDATE_MOB_EFFECT, &body, 6).unwrap();
        assert_eq!(m.player.effects.jump_boost, Some(1));
        let mut body = Vec::new();
        VarInt(5).write(&mut body);
        VarInt(effect::JUMP_BOOST).write(&mut body);
        m.apply_status(from::REMOVE_MOB_EFFECT, &body, 5).unwrap();
        assert_eq!(m.player.effects, Effects::default());
    }

    #[test]
    fn hungry_players_may_sprint_only_with_flight() {
        let mut m = Movement::default();
        let mut body = Vec::new();
        20.0f32.write(&mut body);
        VarInt(6).write(&mut body);
        0.0f32.write(&mut body);
        m.apply_status(from::SET_HEALTH, &body, 1).unwrap();
        assert!(!m.player.has_food);
        m.player.may_fly = true;
        m.apply_status(from::SET_HEALTH, &body, 1).unwrap();
        assert!(m.player.has_food);
    }

    #[test]
    fn respawn_without_kept_data_faces_south_at_rest() {
        let mut m = Movement::default();
        (m.player.delta, m.player.yaw) = ([0.3, -0.5, 0.1], 45.0);
        m.player.effects.speed = Some(1);
        m.apply_status(from::RESPAWN, &[0], 1).unwrap();
        assert_eq!((m.player.delta, m.player.yaw, m.player.effects.speed), ([0.0; 3], -180.0, None));
        m.player.delta = [0.3, 0.0, 0.0];
        m.apply_status(from::RESPAWN, &[KEEP_ENTITY_DATA], 1).unwrap();
        assert_eq!(m.player.delta, [0.3, 0.0, 0.0]);
    }
}
