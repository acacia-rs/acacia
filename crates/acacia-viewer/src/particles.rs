//! Particles the server asks for: `SpawnParticleEffect` (by Bedrock identifier, what `/particle`
//! sends) and the particle level events, as the renderer's kinds.

use acacia_bot::proto::{Packet, RawPacket};
use acacia_bot::proto::packets::{LevelEvent, LevelEventEvent, SpawnParticleEffect};
use acacia_bot::proto::types::Vec3f;
use acacia_render::particles::Kind;
use glam::{DVec3, Vec3};

/// `count` particles of a kind, scattered over a box `spread` blocks to each side of `at`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spawn {
    pub kind: Kind,
    pub at: DVec3,
    pub velocity: Vec3,
    pub count: u32,
    pub spread: f32,
}

impl Spawn {
    fn one(kind: Kind, at: DVec3) -> Spawn {
        Spawn { kind, at, velocity: Vec3::ZERO, count: 1, spread: 0.0 }
    }

    fn many(kind: Kind, at: DVec3, count: u32, spread: f32) -> Spawn {
        Spawn { count, spread, ..Spawn::one(kind, at) }
    }
}

fn position(p: Vec3f) -> DVec3 {
    DVec3::new(p.x.into(), p.y.into(), p.z.into())
}

pub fn spawns(packet: &RawPacket) -> Vec<Spawn> {
    match packet.id {
        SpawnParticleEffect::ID => {
            let Ok(p) = packet.decode::<SpawnParticleEffect>() else { return Vec::new() };
            let kind = Kind::from_identifier(&p.particle_name);
            if kind.is_none() {
                tracing::debug!(name = p.particle_name, "particle effect not drawn");
            }
            kind.map(|k| Spawn::one(k, position(p.position))).into_iter().collect()
        }
        LevelEvent::ID => packet.decode::<LevelEvent>().ok().and_then(|p| level_event(p.event, position(p.position), p.data)).into_iter().collect(),
        _ => Vec::new(),
    }
}

/// The particles of a level event; bursts are Java's counts where it has the same effect.
fn level_event(event: LevelEventEvent, at: DVec3, data: i32) -> Option<Spawn> {
    use LevelEventEvent as E;
    let kind = match event {
        E::ParticleCritical | E::AddParticleCritical => Kind::CritBurst,
        E::ParticleExplosion | E::AddParticleHugeExplodeSeed => Kind::ExplosionEmitter,
        E::AddParticleHugeExplode => Kind::Explosion,
        E::ParticleDeathSmoke => return Some(Spawn::many(Kind::Poof, at, 20, 0.5)),
        E::ParticleSpawn => return Some(Spawn::many(Kind::Flame, at, 20, 0.5)),
        E::ParticleCropGrowth => return Some(Spawn::many(Kind::HappyVillager, at + 0.5, 15, 0.5)),
        E::ParticleEvaporate | E::ParticleEvaporateWater | E::ParticleFizzEffect => return Some(Spawn::many(Kind::LargeSmoke, at + 0.5, 8, 0.5)),
        E::ParticleSplash => return Some(Spawn::many(Kind::Splash, at, 16, 0.3)),
        E::ParticleBubble | E::AddParticleBubble | E::AddParticleBubbleManual => Kind::Bubble,
        E::AddParticleSmoke => Kind::Smoke,
        E::AddParticleLargeSmoke | E::AddParticleEvaporation => Kind::LargeSmoke,
        E::AddParticleExplode | E::AddParticleSnowballPoof => Kind::Poof,
        E::AddParticleFlame | E::AddParticleMobFlame => Kind::Flame,
        E::AddParticleCandleFlame => Kind::SmallFlame,
        E::AddParticleBlueFlame | E::AddParticleSoul => Kind::SoulFlame,
        E::AddParticleLava => Kind::Lava,
        E::AddParticleRedstone | E::AddParticleRisingRedDust | E::AddParticleFallingRedDust => Kind::REDSTONE,
        E::AddParticleHeart => Kind::Heart,
        E::AddParticleVillagerAngry => Kind::AngryVillager,
        E::AddParticleVillagerHappy | E::AddParticleTownAura => Kind::HappyVillager,
        E::AddParticlePortal | E::AddParticlePortalReverse => Kind::Portal,
        E::AddParticleWaterSplash | E::AddParticleWaterSplashManual => Kind::Splash,
        E::AddParticleRainSplash => Kind::Rain,
        E::AddParticleDripWater | E::AddParticleStalactiteDripWater => Kind::DrippingWater,
        E::AddParticleDripLava | E::AddParticleStalactiteDripLava => Kind::DrippingLava,
        E::AddParticleCampfireSmoke => Kind::CampfireSmoke,
        E::AddParticleTallCampfireSmoke => Kind::SignalSmoke,
        // Note blocks send the note, 0 to 24.
        E::AddParticleNote => Kind::Note(data.clamp(0, 24) as f32 / 24.0),
        _ => return None,
    };
    Some(Spawn::one(kind, at))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_events_map_to_kinds() {
        let at = DVec3::ZERO;
        assert_eq!(level_event(LevelEventEvent::AddParticleFlame, at, 0).map(|s| s.kind), Some(Kind::Flame));
        assert_eq!(level_event(LevelEventEvent::ParticleDeathSmoke, at, 0).map(|s| (s.kind, s.count)), Some((Kind::Poof, 20)));
        assert_eq!(level_event(LevelEventEvent::SoundClick, at, 0), None);
    }
}
