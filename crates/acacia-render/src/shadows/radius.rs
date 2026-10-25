//! Each kind's shadow: its renderer's `getShadowRadius` and `getShadowStrength` in Java 26.3, an
//! adult's and a baby's (`MobRenderer` scales by the age scale; villagers, striders, turtles and
//! others scale further). Kinds left out cast none. Checked by the parity harness `shadows`.

/// Bedrock identifiers whose Java name differs.
const JAVA_NAMES: [(&str, &str); 17] = [
    ("minecraft:xp_orb", "minecraft:experience_orb"),
    ("minecraft:ender_crystal", "minecraft:end_crystal"),
    ("minecraft:zombie_pigman", "minecraft:zombified_piglin"),
    ("minecraft:evocation_illager", "minecraft:evoker"),
    ("minecraft:villager_v2", "minecraft:villager"),
    ("minecraft:zombie_villager_v2", "minecraft:zombie_villager"),
    ("minecraft:fishing_hook", "minecraft:fishing_bobber"),
    ("minecraft:boat", "minecraft:oak_boat"),
    ("minecraft:chest_boat", "minecraft:oak_chest_boat"),
    ("minecraft:thrown_trident", "minecraft:trident"),
    ("minecraft:xp_bottle", "minecraft:experience_bottle"),
    ("minecraft:eye_of_ender_signal", "minecraft:eye_of_ender"),
    ("minecraft:fireworks_rocket", "minecraft:firework_rocket"),
    ("minecraft:wither_skull_dangerous", "minecraft:wither_skull"),
    ("minecraft:evocation_fang", "minecraft:evoker_fangs"),
    ("minecraft:wind_charge_projectile", "minecraft:wind_charge"),
    ("minecraft:breeze_wind_charge_projectile", "minecraft:breeze_wind_charge"),
];

/// A Bedrock kind's Java identifier.
pub fn java_name(kind: &str) -> &str {
    JAVA_NAMES.iter().find(|(bedrock, _)| *bedrock == kind).map_or(kind, |(_, java)| java)
}

/// (kind, adult radius, baby radius); boats, rafts, minecarts and the size-driven kinds are in
/// [`radius`].
const TABLE: &[(&str, f32, f32)] = &[
    ("minecraft:allay", 0.4, 0.4),
    ("minecraft:armadillo", 0.4, 0.24000001),
    ("minecraft:axolotl", 0.5, 0.25),
    ("minecraft:bat", 0.25, 0.25),
    ("minecraft:bee", 0.4, 0.2),
    ("minecraft:blaze", 0.5, 0.5),
    ("minecraft:bogged", 0.5, 0.5),
    ("minecraft:breeze", 0.5, 0.5),
    ("minecraft:camel", 0.7, 0.42000002),
    ("minecraft:camel_husk", 0.7, 0.7),
    ("minecraft:cat", 0.4, 0.2),
    ("minecraft:cave_spider", 0.56, 0.56),
    ("minecraft:chicken", 0.3, 0.15),
    ("minecraft:cod", 0.3, 0.3),
    ("minecraft:copper_golem", 0.5, 0.5),
    ("minecraft:cow", 0.7, 0.35),
    ("minecraft:creaking", 0.6, 0.6),
    ("minecraft:creeper", 0.5, 0.5),
    ("minecraft:dolphin", 0.7, 0.45499998),
    ("minecraft:donkey", 0.75, 0.375),
    ("minecraft:drowned", 0.5, 0.25),
    ("minecraft:elder_guardian", 1.2, 1.2),
    ("minecraft:end_crystal", 0.5, 0.5),
    ("minecraft:ender_dragon", 0.5, 0.5),
    ("minecraft:enderman", 0.5, 0.5),
    ("minecraft:endermite", 0.3, 0.3),
    ("minecraft:evoker", 0.5, 0.5),
    ("minecraft:experience_orb", 0.15, 0.15),
    ("minecraft:falling_block", 0.5, 0.5),
    ("minecraft:fox", 0.4, 0.2),
    ("minecraft:frog", 0.3, 0.3),
    ("minecraft:ghast", 1.5, 1.5),
    ("minecraft:giant", 3.0, 3.0),
    ("minecraft:glow_squid", 0.7, 0.35),
    ("minecraft:goat", 0.7, 0.385),
    ("minecraft:guardian", 0.5, 0.5),
    ("minecraft:happy_ghast", 2.0, 0.475),
    ("minecraft:hoglin", 0.7, 0.35),
    ("minecraft:horse", 0.75, 0.375),
    ("minecraft:husk", 0.5, 0.25),
    ("minecraft:illusioner", 0.5, 0.5),
    ("minecraft:iron_golem", 0.7, 0.7),
    ("minecraft:item", 0.15, 0.15),
    ("minecraft:llama", 0.7, 0.35),
    ("minecraft:mooshroom", 0.7, 0.35),
    ("minecraft:mule", 0.75, 0.375),
    ("minecraft:nautilus", 0.7, 0.35),
    ("minecraft:ocelot", 0.4, 0.2),
    ("minecraft:panda", 0.9, 0.45),
    ("minecraft:parched", 0.5, 0.5),
    ("minecraft:parrot", 0.3, 0.3),
    ("minecraft:phantom", 0.75, 0.75),
    ("minecraft:pig", 0.7, 0.35),
    ("minecraft:piglin", 0.5, 0.25),
    ("minecraft:piglin_brute", 0.5, 0.5),
    ("minecraft:pillager", 0.5, 0.5),
    // AvatarRenderer's; players are not in the harness (the oracle cannot make one).
    ("minecraft:player", 0.5, 0.5),
    ("minecraft:polar_bear", 0.9, 0.45),
    ("minecraft:rabbit", 0.3, 0.15),
    ("minecraft:ravager", 1.1, 1.1),
    ("minecraft:salmon", 0.4, 0.4),
    ("minecraft:sheep", 0.7, 0.35),
    ("minecraft:silverfish", 0.3, 0.3),
    ("minecraft:skeleton", 0.5, 0.5),
    ("minecraft:skeleton_horse", 0.75, 0.375),
    ("minecraft:sniffer", 1.1, 0.55),
    ("minecraft:snow_golem", 0.5, 0.5),
    ("minecraft:spider", 0.8, 0.8),
    ("minecraft:squid", 0.7, 0.35),
    ("minecraft:stray", 0.5, 0.5),
    ("minecraft:strider", 0.5, 0.125),
    ("minecraft:tadpole", 0.14, 0.14),
    ("minecraft:tnt", 0.5, 0.5),
    ("minecraft:trader_llama", 0.7, 0.35),
    ("minecraft:tropical_fish", 0.15, 0.15),
    ("minecraft:turtle", 0.7, 0.1743),
    ("minecraft:vex", 0.3, 0.3),
    ("minecraft:villager", 0.5, 0.125),
    ("minecraft:vindicator", 0.5, 0.5),
    ("minecraft:wandering_trader", 0.5, 0.25),
    ("minecraft:warden", 0.9, 0.9),
    ("minecraft:witch", 0.5, 0.5),
    ("minecraft:wither", 1.0, 1.0),
    ("minecraft:wither_skeleton", 0.5, 0.5),
    ("minecraft:wolf", 0.5, 0.25),
    ("minecraft:zoglin", 0.7, 0.35),
    ("minecraft:zombie", 0.5, 0.25),
    ("minecraft:zombie_horse", 0.75, 0.375),
    ("minecraft:zombie_nautilus", 0.7, 0.7),
    ("minecraft:zombie_villager", 0.5, 0.25),
    ("minecraft:zombified_piglin", 0.5, 0.25),
];

/// The radius and strength of a `kind`'s shadow (Bedrock or Java identifier); `size` is a cube
/// mob's, `puff` a pufferfish's. `None` for kinds without one.
pub fn radius(kind: &str, baby: bool, size: f32, puff: f32) -> Option<(f32, f32)> {
    let kind = java_name(kind);
    let strength = if matches!(kind, "minecraft:item" | "minecraft:experience_orb") { 0.75 } else { 1.0 };
    let radius = match kind {
        "minecraft:slime" | "minecraft:magma_cube" | "minecraft:sulfur_cube" => size * 0.25,
        "minecraft:pufferfish" => 0.1 + 0.1 * puff,
        _ if kind.ends_with("_boat") || kind.ends_with("_raft") => 0.8,
        _ if kind.ends_with("minecart") => 0.7,
        _ => {
            let &(_, adult, young) = TABLE.iter().find(|(k, ..)| *k == kind)?;
            if baby { young } else { adult }
        }
    };
    Some((radius, strength))
}
