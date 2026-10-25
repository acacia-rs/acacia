//! Per entity type facts the Java look reads: where the name tag hangs (26.3's
//! `EntityAttachment.NAME_TAG`: the top of the entity's box in its pose, the sniffer's own offset).
//! Checked by the parity harness `name_tags`.

use crate::shadows::radius::java_name;

/// A name tag: blocks over the feet (before the renderer's half block), and the distance it shows
/// within (`NAME_TAG_DISTANCE`; 64 for every type's default).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NameTag {
    pub height: f32,
    pub distance: f32,
}

/// (kind, adult height, baby height, living); boats, rafts and minecarts are in [`name_tag`].
const HEIGHTS: &[(&str, f32, f32, bool)] = &[
    ("minecraft:allay", 0.6, 0.6, true),
    ("minecraft:area_effect_cloud", 0.5, 0.5, false),
    ("minecraft:armadillo", 0.65, 0.39, true),
    ("minecraft:armor_stand", 1.975, 1.975, true),
    ("minecraft:arrow", 0.5, 0.5, false),
    ("minecraft:axolotl", 0.42, 0.21, true),
    ("minecraft:bat", 0.9, 0.9, true),
    ("minecraft:bee", 0.5, 0.25, true),
    ("minecraft:blaze", 1.8, 1.8, true),
    ("minecraft:block_display", 0.0, 0.0, false),
    ("minecraft:bogged", 1.99, 1.99, true),
    ("minecraft:breeze", 1.77, 1.77, true),
    ("minecraft:breeze_wind_charge", 0.3125, 0.3125, false),
    ("minecraft:camel", 2.375, 1.4, true),
    ("minecraft:camel_husk", 2.375, 2.375, true),
    ("minecraft:cat", 0.7, 0.35, true),
    ("minecraft:cave_spider", 0.5, 0.5, true),
    ("minecraft:chicken", 0.7, 0.4, true),
    ("minecraft:cod", 0.3, 0.3, true),
    ("minecraft:copper_golem", 0.98, 0.98, true),
    ("minecraft:cow", 1.4, 0.7, true),
    ("minecraft:creaking", 2.7, 2.7, true),
    ("minecraft:creeper", 1.7, 1.7, true),
    ("minecraft:cushion", 0.25, 0.25, false),
    ("minecraft:dolphin", 0.6, 0.39, true),
    ("minecraft:donkey", 1.5, 0.75, true),
    ("minecraft:dragon_fireball", 1.0, 1.0, false),
    ("minecraft:drowned", 1.95, 0.98, true),
    ("minecraft:egg", 0.25, 0.25, false),
    ("minecraft:elder_guardian", 1.9975, 1.9975, true),
    ("minecraft:end_crystal", 2.0, 2.0, false),
    ("minecraft:ender_dragon", 8.0, 8.0, true),
    ("minecraft:ender_pearl", 0.25, 0.25, false),
    ("minecraft:enderman", 2.9, 2.9, true),
    ("minecraft:endermite", 0.3, 0.3, true),
    ("minecraft:evoker", 1.95, 1.95, true),
    ("minecraft:evoker_fangs", 0.8, 0.8, false),
    ("minecraft:experience_bottle", 0.25, 0.25, false),
    ("minecraft:experience_orb", 0.5, 0.5, false),
    ("minecraft:eye_of_ender", 0.25, 0.25, false),
    ("minecraft:falling_block", 0.98, 0.98, false),
    ("minecraft:fireball", 1.0, 1.0, false),
    ("minecraft:firework_rocket", 0.25, 0.25, false),
    ("minecraft:fishing_bobber", 0.25, 0.25, false),
    ("minecraft:fox", 0.7, 0.42, true),
    ("minecraft:frog", 0.5, 0.5, true),
    ("minecraft:ghast", 4.0, 4.0, true),
    ("minecraft:giant", 12.0, 12.0, true),
    ("minecraft:glow_item_frame", 0.5, 0.5, false),
    ("minecraft:glow_squid", 0.8, 0.5, true),
    ("minecraft:goat", 1.3, 0.65, true),
    ("minecraft:guardian", 0.85, 0.85, true),
    ("minecraft:happy_ghast", 4.0, 0.95, true),
    ("minecraft:hoglin", 1.4, 0.85, true),
    ("minecraft:horse", 1.6, 1.12, true),
    ("minecraft:husk", 1.95, 0.98, true),
    ("minecraft:illusioner", 1.95, 1.95, true),
    ("minecraft:interaction", 1.0, 1.0, false),
    ("minecraft:iron_golem", 2.7, 2.7, true),
    ("minecraft:item", 0.25, 0.25, false),
    ("minecraft:item_display", 0.0, 0.0, false),
    ("minecraft:item_frame", 0.5, 0.5, false),
    ("minecraft:leash_knot", 0.5, 0.5, false),
    ("minecraft:lightning_bolt", 0.0, 0.0, false),
    ("minecraft:lingering_potion", 0.25, 0.25, false),
    ("minecraft:llama", 1.87, 0.935, true),
    ("minecraft:llama_spit", 0.25, 0.25, false),
    ("minecraft:magma_cube", 0.52, 0.52, true),
    ("minecraft:mannequin", 1.8, 1.8, true),
    ("minecraft:marker", 0.0, 0.0, false),
    ("minecraft:mooshroom", 1.4, 0.7, true),
    ("minecraft:mule", 1.6, 0.8, true),
    ("minecraft:nautilus", 0.95, 0.475, true),
    ("minecraft:ocelot", 0.7, 0.35, true),
    ("minecraft:ominous_item_spawner", 0.25, 0.25, false),
    ("minecraft:painting", 0.5, 0.5, false),
    ("minecraft:panda", 1.25, 0.625, true),
    ("minecraft:parched", 1.99, 1.99, true),
    ("minecraft:parrot", 0.9, 0.9, true),
    ("minecraft:phantom", 0.5, 0.5, true),
    ("minecraft:pig", 0.9, 0.45, true),
    ("minecraft:piglin", 1.95, 0.98, true),
    ("minecraft:piglin_brute", 1.95, 1.95, true),
    ("minecraft:pillager", 1.95, 1.95, true),
    // The mannequin's: both are avatars (the oracle cannot make a player).
    ("minecraft:player", 1.8, 1.8, true),
    ("minecraft:polar_bear", 1.4, 0.7, true),
    ("minecraft:pufferfish", 0.35, 0.35, true),
    ("minecraft:rabbit", 0.6, 0.4, true),
    ("minecraft:ravager", 2.2, 2.2, true),
    ("minecraft:salmon", 0.4, 0.4, true),
    ("minecraft:sheep", 1.3, 0.65, true),
    ("minecraft:shulker", 1.0, 1.0, true),
    ("minecraft:shulker_bullet", 0.3125, 0.3125, false),
    ("minecraft:silverfish", 0.3, 0.3, true),
    ("minecraft:skeleton", 1.99, 1.99, true),
    ("minecraft:skeleton_horse", 1.6, 1.12, true),
    ("minecraft:slime", 0.52, 0.52, true),
    ("minecraft:small_fireball", 0.3125, 0.3125, false),
    ("minecraft:sniffer", 2.05, 1.025, true),
    ("minecraft:snow_golem", 1.9, 1.9, true),
    ("minecraft:snowball", 0.25, 0.25, false),
    ("minecraft:spectral_arrow", 0.5, 0.5, false),
    ("minecraft:spider", 0.9, 0.9, true),
    ("minecraft:splash_potion", 0.25, 0.25, false),
    ("minecraft:squid", 0.8, 0.5, true),
    ("minecraft:stray", 1.99, 1.99, true),
    ("minecraft:strider", 1.7, 0.85, true),
    ("minecraft:sulfur_cube", 0.49, 0.49, true),
    ("minecraft:tadpole", 0.3, 0.3, true),
    ("minecraft:text_display", 0.0, 0.0, false),
    ("minecraft:tnt", 0.98, 0.98, false),
    ("minecraft:trader_llama", 1.87, 0.935, true),
    ("minecraft:trident", 0.5, 0.5, false),
    ("minecraft:tropical_fish", 0.4, 0.4, true),
    ("minecraft:turtle", 0.4, 0.12, true),
    ("minecraft:vex", 0.8, 0.8, true),
    ("minecraft:villager", 1.95, 0.98, true),
    ("minecraft:vindicator", 1.95, 1.95, true),
    ("minecraft:wandering_trader", 1.95, 0.975, true),
    ("minecraft:warden", 2.9, 2.9, true),
    ("minecraft:wind_charge", 0.3125, 0.3125, false),
    ("minecraft:witch", 1.95, 1.95, true),
    ("minecraft:wither", 3.5, 3.5, true),
    ("minecraft:wither_skeleton", 2.4, 2.4, true),
    ("minecraft:wither_skull", 0.3125, 0.3125, false),
    ("minecraft:wolf", 0.85, 0.425, true),
    ("minecraft:zoglin", 1.4, 0.85, true),
    ("minecraft:zombie", 1.95, 0.98, true),
    ("minecraft:zombie_horse", 1.6, 1.12, true),
    ("minecraft:zombie_nautilus", 0.95, 0.95, true),
    ("minecraft:zombie_villager", 1.95, 0.98, true),
    ("minecraft:zombified_piglin", 1.95, 0.98, true),
];

/// The height of a living entity's box in a pose other than standing, where it differs
/// (`getDefaultDimensions` overrides; Java's pose names, lower case).
fn posed(kind: &str, baby: bool, pose: &str) -> Option<f32> {
    let avatar = matches!(kind, "minecraft:player" | "minecraft:mannequin");
    Some(match pose {
        "sleeping" => 0.2,
        "dying" if avatar => 0.2,
        "crouching" if avatar => 1.5,
        "swimming" | "fall_flying" | "spin_attack" if avatar => 0.6,
        "sitting" if matches!(kind, "minecraft:camel" | "minecraft:camel_husk") => {
            if baby { 0.425 } else { 0.945 }
        }
        "long_jumping" if kind == "minecraft:goat" => {
            if baby { 0.455 } else { 0.91 }
        }
        "emerging" | "digging" if kind == "minecraft:warden" => 1.0,
        _ => return None,
    })
}

/// Where `kind`'s (Bedrock or Java identifier) name tag hangs, a baby's or an adult's, in `pose`.
pub fn name_tag(kind: &str, baby: bool, pose: &str) -> Option<NameTag> {
    let kind = java_name(kind);
    let height = if kind.ends_with("_boat") || kind.ends_with("_raft") {
        0.5625
    } else if kind.ends_with("minecart") {
        0.7
    } else {
        let &(_, adult, young, living) = HEIGHTS.iter().find(|(k, ..)| *k == kind)?;
        let standing = if baby { young } else { adult };
        if living { posed(kind, baby, pose).unwrap_or(standing) } else { standing }
    };
    Some(NameTag { height, distance: 64.0 })
}
