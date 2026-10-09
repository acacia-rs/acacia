//! What a particle is, whichever look draws it, and the Bedrock identifiers that name each.

/// A particle or a burst of them. Velocities passed with one are Java's `addParticle` arguments
/// (blocks per tick); some kinds read them as parameters instead, as Java's providers do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Flame,
    /// A candle's.
    SmallFlame,
    SoulFlame,
    Smoke,
    LargeSmoke,
    CampfireSmoke,
    SignalSmoke,
    Crit,
    EnchantedHit,
    /// A critical hit's burst (Java's tracking emitter: 16 a tick for 3 ticks).
    CritBurst,
    EnchantedHitBurst,
    /// One explosion puff; the velocity's x shrinks it (Java's `HugeExplosionParticle`).
    Explosion,
    /// Puffs scattered over 8 ticks (Java's `HugeExplosionSeedParticle`).
    ExplosionEmitter,
    /// Java's `poof`, Bedrock's `explosion_particle`.
    Poof,
    Heart,
    AngryVillager,
    HappyVillager,
    Splash,
    Rain,
    Bubble,
    Lava,
    /// Redstone dust and the like, in this colour.
    Dust([f32; 3]),
    /// A note block's note, its pitch 0 to 1 picking the colour.
    Note(f32),
    Portal,
    DrippingWater,
    FallingWater,
    DrippingLava,
    FallingLava,
    LandingLava,
}

impl Kind {
    pub const REDSTONE: Kind = Kind::Dust([1.0, 0.0, 0.0]);

    /// The kind a Bedrock particle effect identifier (`SpawnParticleEffect`, `/particle`) shows;
    /// `None` for effects not drawn yet.
    pub fn from_identifier(name: &str) -> Option<Kind> {
        let name = name.strip_prefix("minecraft:").unwrap_or(name);
        Some(match name {
            "basic_flame_particle" | "mobflame_single" => Kind::Flame,
            "candle_flame_particle" | "small_flame_particle" => Kind::SmallFlame,
            "blue_flame_particle" | "small_soul_fire_flame" => Kind::SoulFlame,
            "basic_smoke_particle" => Kind::Smoke,
            "water_evaporation_manual" | "water_evaporation_bucket_emitter" => Kind::LargeSmoke,
            "campfire_smoke_particle" => Kind::CampfireSmoke,
            "campfire_tall_smoke_particle" => Kind::SignalSmoke,
            "basic_crit_particle" => Kind::Crit,
            "critical_hit_emitter" => Kind::CritBurst,
            "magic_critical_hit_emitter" => Kind::EnchantedHitBurst,
            "large_explosion" => Kind::Explosion,
            "huge_explosion_emitter" | "huge_explosion_lab_misc_emitter" => Kind::ExplosionEmitter,
            "explosion_particle" | "explosion_manual" | "egg_destroy_emitter" | "death_explosion_emitter" => Kind::Poof,
            "heart_particle" => Kind::Heart,
            "villager_angry" => Kind::AngryVillager,
            "villager_happy" | "crop_growth_emitter" => Kind::HappyVillager,
            "water_splash_particle" | "water_splash_particle_manual" => Kind::Splash,
            "rain_splash_particle" => Kind::Rain,
            "basic_bubble_particle" | "basic_bubble_particle_manual" => Kind::Bubble,
            "lava_particle" => Kind::Lava,
            "redstone_wire_dust_particle" | "redstone_torch_dust_particle" | "redstone_ore_dust_particle" | "redstone_repeater_dust_particle" => Kind::REDSTONE,
            "note_particle" => Kind::Note(0.0),
            "basic_portal_particle" | "portal_reverse_particle" | "portal_directional" | "portal_east_west" | "portal_north_south" => Kind::Portal,
            "water_drip_particle" | "stalactite_water_drip_particle" => Kind::DrippingWater,
            "lava_drip_particle" | "stalactite_lava_drip_particle" => Kind::DrippingLava,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_name_kinds_with_or_without_the_namespace() {
        assert_eq!(Kind::from_identifier("minecraft:basic_flame_particle"), Some(Kind::Flame));
        assert_eq!(Kind::from_identifier("critical_hit_emitter"), Some(Kind::CritBurst));
        assert_eq!(Kind::from_identifier("minecraft:no_such_particle"), None);
    }
}
