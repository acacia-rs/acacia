//! The Java particle types drawn here, by registry name, with their sprite sets as the 26.3
//! client's `particles/<name>.json` lists them (order matters: providers index into it).

macro_rules! types {
    ($($variant:ident = $name:literal [$($sprite:literal),*]),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Type { $($variant),* }

        impl Type {
            pub const ALL: &'static [Type] = &[$(Type::$variant),*];

            pub fn name(self) -> &'static str {
                match self { $(Type::$variant => $name),* }
            }

            pub fn from_name(name: &str) -> Option<Type> {
                match name.strip_prefix("minecraft:").unwrap_or(name) { $($name => Some(Type::$variant),)* _ => None }
            }

            /// `textures/particle/` names of the sprite set.
            pub fn sprites(self) -> &'static [&'static str] {
                match self { $(Type::$variant => &[$($sprite),*]),* }
            }
        }
    };
}

const GENERIC_DOWN: [&str; 8] = ["generic_7", "generic_6", "generic_5", "generic_4", "generic_3", "generic_2", "generic_1", "generic_0"];

types! {
    Smoke = "smoke" ["generic_7", "generic_6", "generic_5", "generic_4", "generic_3", "generic_2", "generic_1", "generic_0"],
    LargeSmoke = "large_smoke" ["generic_7", "generic_6", "generic_5", "generic_4", "generic_3", "generic_2", "generic_1", "generic_0"],
    WhiteSmoke = "white_smoke" ["generic_7", "generic_6", "generic_5", "generic_4", "generic_3", "generic_2", "generic_1", "generic_0"],
    Flame = "flame" ["flame"],
    SoulFireFlame = "soul_fire_flame" ["soul_fire_flame"],
    CopperFireFlame = "copper_fire_flame" ["copper_fire_flame"],
    SmallFlame = "small_flame" ["flame"],
    Lava = "lava" ["lava"],
    DrippingWater = "dripping_water" ["drip_hang"],
    FallingWater = "falling_water" ["drip_fall"],
    DrippingLava = "dripping_lava" ["drip_hang"],
    FallingLava = "falling_lava" ["drip_fall"],
    LandingLava = "landing_lava" ["drip_land"],
    DrippingHoney = "dripping_honey" ["drip_hang"],
    FallingHoney = "falling_honey" ["drip_fall"],
    LandingHoney = "landing_honey" ["drip_land"],
    FallingNectar = "falling_nectar" ["drip_fall"],
    FallingSporeBlossom = "falling_spore_blossom" ["drip_fall"],
    SporeBlossomAir = "spore_blossom_air" ["drip_fall"],
    DrippingObsidianTear = "dripping_obsidian_tear" ["drip_hang"],
    FallingObsidianTear = "falling_obsidian_tear" ["drip_fall"],
    LandingObsidianTear = "landing_obsidian_tear" ["drip_land"],
    DrippingDripstoneWater = "dripping_dripstone_water" ["drip_hang"],
    FallingDripstoneWater = "falling_dripstone_water" ["drip_fall"],
    DrippingDripstoneLava = "dripping_dripstone_lava" ["drip_hang"],
    FallingDripstoneLava = "falling_dripstone_lava" ["drip_fall"],
    Splash = "splash" ["splash_0", "splash_1", "splash_2", "splash_3"],
    Rain = "rain" ["splash_0", "splash_1", "splash_2", "splash_3"],
    Bubble = "bubble" ["bubble"],
    BubbleColumnUp = "bubble_column_up" ["bubble"],
    SulfurBubbles = "sulfur_bubbles" ["bubble_white"],
    CurrentDown = "current_down" ["bubble"],
    Underwater = "underwater" ["generic_0"],
    CrimsonSpore = "crimson_spore" ["generic_0"],
    WarpedSpore = "warped_spore" ["generic_0"],
    Ash = "ash" ["generic_0"],
    WhiteAsh = "white_ash" ["generic_0"],
    Portal = "portal" ["generic_0", "generic_1", "generic_2", "generic_3", "generic_4", "generic_5", "generic_6", "generic_7"],
    ReversePortal = "reverse_portal" ["generic_0", "generic_1", "generic_2", "generic_3", "generic_4", "generic_5", "generic_6", "generic_7"],
    Enchant = "enchant" ["sga_a", "sga_b", "sga_c", "sga_d", "sga_e", "sga_f", "sga_g", "sga_h", "sga_i", "sga_j", "sga_k", "sga_l", "sga_m", "sga_n", "sga_o", "sga_p", "sga_q", "sga_r", "sga_s", "sga_t", "sga_u", "sga_v", "sga_w", "sga_x", "sga_y", "sga_z"],
    Crit = "crit" ["critical_hit"],
    EnchantedHit = "enchanted_hit" ["enchanted_hit"],
    Heart = "heart" ["heart"],
    AngryVillager = "angry_villager" ["angry"],
    HappyVillager = "happy_villager" ["glint"],
    Mycelium = "mycelium" ["generic_0"],
    Poof = "poof" ["generic_7", "generic_6", "generic_5", "generic_4", "generic_3", "generic_2", "generic_1", "generic_0"],
    Explosion = "explosion" ["explosion_0", "explosion_1", "explosion_2", "explosion_3", "explosion_4", "explosion_5", "explosion_6", "explosion_7", "explosion_8", "explosion_9", "explosion_10", "explosion_11", "explosion_12", "explosion_13", "explosion_14", "explosion_15"],
    ExplosionEmitter = "explosion_emitter" [],
    CampfireCosySmoke = "campfire_cosy_smoke" ["big_smoke_0", "big_smoke_1", "big_smoke_2", "big_smoke_3", "big_smoke_4", "big_smoke_5", "big_smoke_6", "big_smoke_7", "big_smoke_8", "big_smoke_9", "big_smoke_10", "big_smoke_11"],
    CampfireSignalSmoke = "campfire_signal_smoke" ["big_smoke_0", "big_smoke_1", "big_smoke_2", "big_smoke_3", "big_smoke_4", "big_smoke_5", "big_smoke_6", "big_smoke_7", "big_smoke_8", "big_smoke_9", "big_smoke_10", "big_smoke_11"],
    Note = "note" ["note"],
    Dust = "dust" ["generic_7", "generic_6", "generic_5", "generic_4", "generic_3", "generic_2", "generic_1", "generic_0"],
    DustColorTransition = "dust_color_transition" ["generic_7", "generic_6", "generic_5", "generic_4", "generic_3", "generic_2", "generic_1", "generic_0"],
    EndRod = "end_rod" ["glitter_7", "glitter_6", "glitter_5", "glitter_4", "glitter_3", "glitter_2", "glitter_1", "glitter_0"],
    TotemOfUndying = "totem_of_undying" ["glitter_7", "glitter_6", "glitter_5", "glitter_4", "glitter_3", "glitter_2", "glitter_1", "glitter_0"],
    CherryLeaves = "cherry_leaves" ["cherry_0", "cherry_1", "cherry_2", "cherry_3", "cherry_4", "cherry_5", "cherry_6", "cherry_7", "cherry_8", "cherry_9", "cherry_10", "cherry_11"],
    PaleOakLeaves = "pale_oak_leaves" ["pale_oak_0", "pale_oak_1", "pale_oak_2", "pale_oak_3", "pale_oak_4", "pale_oak_5", "pale_oak_6", "pale_oak_7", "pale_oak_8", "pale_oak_9", "pale_oak_10", "pale_oak_11"],
    TintedLeaves = "tinted_leaves" ["leaf_0", "leaf_1", "leaf_2", "leaf_3", "leaf_4", "leaf_5", "leaf_6", "leaf_7", "leaf_8", "leaf_9", "leaf_10", "leaf_11"],
    FallingDust = "falling_dust" ["generic_7", "generic_6", "generic_5", "generic_4", "generic_3", "generic_2", "generic_1", "generic_0"],
}

impl Type {
    /// `ParticleType.getOverrideLimiter`: spawned however far from the camera.
    pub fn override_limiter(self) -> bool {
        matches!(self, Type::ExplosionEmitter | Type::Explosion | Type::Poof | Type::CampfireCosySmoke | Type::CampfireSignalSmoke)
    }
}

/// Every sprite name some type draws, each once: what a Java sheet packs.
pub fn all_sprites() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = GENERIC_DOWN.to_vec();
    for t in Type::ALL {
        for s in t.sprites() {
            if !names.contains(s) {
                names.push(s);
            }
        }
    }
    names
}

#[test]
fn names_round_trip() {
    assert!(Type::ALL.iter().all(|&t| Type::from_name(t.name()) == Some(t)));
    assert_eq!(Type::from_name("minecraft:campfire_cosy_smoke"), Some(Type::CampfireCosySmoke));
    assert_eq!(all_sprites().iter().filter(|s| s.starts_with("generic")).count(), 8);
}
