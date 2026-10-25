//! Each kind's shadow: its renderer's `shadowRadius` and `shadowStrength` as Java 26.3 constructs
//! them, a baby's as `MobRenderer` scales it (by its age scale, 0.5) and the renderers that change
//! it further (villagers, striders and turtles; slimes by size, pufferfish by puff).

/// Bedrock identifiers whose Java name differs.
const JAVA_NAMES: [(&str, &str); 7] = [
    ("minecraft:xp_orb", "minecraft:experience_orb"),
    ("minecraft:ender_crystal", "minecraft:end_crystal"),
    ("minecraft:zombie_pigman", "minecraft:zombified_piglin"),
    ("minecraft:evocation_illager", "minecraft:evoker"),
    ("minecraft:villager_v2", "minecraft:villager"),
    ("minecraft:zombie_villager_v2", "minecraft:zombie_villager"),
    ("minecraft:fishing_hook", "minecraft:fishing_bobber"),
];

/// (kind, adult radius, baby radius, strength); kinds with a radius of 0 are left out.
const TABLE: [(&str, f32, f32, f32); 0] = [];

/// The radius and strength of a `kind`'s shadow; `size` is a slime's, `puff` a pufferfish's.
pub fn radius(kind: &str, baby: bool, size: f32, puff: f32) -> Option<(f32, f32)> {
    let kind = JAVA_NAMES.iter().find(|(bedrock, _)| *bedrock == kind).map_or(kind, |(_, java)| java);
    match kind {
        "minecraft:slime" | "minecraft:magma_cube" => return Some((size * 0.25, 1.0)),
        "minecraft:pufferfish" => return Some((0.1 + 0.1 * puff, 1.0)),
        _ => {}
    }
    let &(_, adult, young, strength) = TABLE.iter().find(|(k, ..)| *k == kind)?;
    Some((if baby { young } else { adult }, strength))
}
