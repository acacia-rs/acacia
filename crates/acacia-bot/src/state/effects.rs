//! Status effects from `MobEffect`, kept per entity (and for the local player).

use acacia_client::proto::packets::{MobEffect, MobEffectEventId};

/// Effect ids (gophertunnel `effect` package).
pub mod effect {
    pub const SPEED: i32 = 1;
    pub const SLOWNESS: i32 = 2;
    pub const HASTE: i32 = 3;
    pub const MINING_FATIGUE: i32 = 4;
    pub const STRENGTH: i32 = 5;
    pub const JUMP_BOOST: i32 = 8;
    pub const REGENERATION: i32 = 10;
    pub const RESISTANCE: i32 = 11;
    pub const FIRE_RESISTANCE: i32 = 12;
    pub const WATER_BREATHING: i32 = 13;
    pub const INVISIBILITY: i32 = 14;
    pub const BLINDNESS: i32 = 15;
    pub const NIGHT_VISION: i32 = 16;
    pub const HUNGER: i32 = 17;
    pub const WEAKNESS: i32 = 18;
    pub const POISON: i32 = 19;
    pub const WITHER: i32 = 20;
    pub const ABSORPTION: i32 = 22;
    pub const LEVITATION: i32 = 24;
    pub const SLOW_FALLING: i32 = 27;
    pub const DARKNESS: i32 = 30;
    pub const WEAVING: i32 = 33;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effect {
    pub id: i32,
    /// 0 for level I.
    pub amplifier: i32,
    /// Ticks left when the server sent it; -1 for infinite.
    pub duration: i32,
    /// Server tick of that packet (0 from servers that don't stamp it).
    pub tick: u64,
    pub ambient: bool,
    pub particles: bool,
}

/// Active effects of one entity, in arrival order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Effects(Vec<Effect>);

impl Effects {
    pub fn get(&self, id: i32) -> Option<&Effect> {
        self.0.iter().find(|e| e.id == id)
    }

    /// Amplifier + 1 (the level shown in game), 0 when not active.
    pub fn level(&self, id: i32) -> i32 {
        self.get(id).map_or(0, |e| e.amplifier + 1)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Effect> {
        self.0.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn apply(&mut self, p: &MobEffect) {
        self.0.retain(|e| e.id != p.effect_id);
        if p.event_id != MobEffectEventId::Remove {
            self.0.push(Effect {
                id: p.effect_id,
                amplifier: p.amplifier,
                duration: p.duration,
                tick: p.tick,
                ambient: p.ambient,
                particles: p.particles,
            });
        }
    }

    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(event_id: MobEffectEventId, effect_id: i32, amplifier: i32) -> MobEffect {
        MobEffect { runtime_entity_id: 1, event_id, effect_id, amplifier, particles: true, duration: 200, tick: 7, ambient: false }
    }

    #[test]
    fn add_update_remove() {
        let mut e = Effects::default();
        e.apply(&packet(MobEffectEventId::Add, effect::SPEED, 0));
        e.apply(&packet(MobEffectEventId::Add, effect::JUMP_BOOST, 1));
        assert_eq!((e.level(effect::SPEED), e.level(effect::JUMP_BOOST)), (1, 2));
        e.apply(&packet(MobEffectEventId::Update, effect::SPEED, 2));
        assert_eq!(e.level(effect::SPEED), 3);
        assert_eq!(e.iter().count(), 2, "an update replaces, never duplicates");
        e.apply(&packet(MobEffectEventId::Remove, effect::SPEED, 0));
        assert_eq!(e.level(effect::SPEED), 0);
        assert_eq!(e.get(effect::JUMP_BOOST).unwrap().tick, 7);
    }
}
