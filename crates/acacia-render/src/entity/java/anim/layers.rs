//! Which `setupAnim` each model layer runs.

use super::Family;

impl Family {
    /// The family of a layer (`minecraft:cow#main`, `minecraft:zombie_baby#main`); `None` for
    /// one not ported yet.
    pub fn of_layer(layer: &str) -> Option<Family> {
        let (name, part) = layer.trim_start_matches("minecraft:").split_once('#').unwrap_or((layer, "main"));
        if (name, part) == ("player", "cape") {
            return Some(Family::PlayerCape);
        }
        if part != "main" {
            return None;
        }
        Some(match name.trim_end_matches("_baby") {
            "cow" | "cold_cow" | "warm_cow" | "mooshroom" | "pig" | "cold_pig" | "warm_pig" => Family::Quadruped,
            "sheep" => Family::Sheep,
            "chicken" | "cold_chicken" | "warm_chicken" if name.ends_with("_baby") => Family::BabyChicken,
            "chicken" | "cold_chicken" | "warm_chicken" => Family::Chicken,
            "creeper" => Family::Creeper,
            "spider" | "cave_spider" => Family::Spider,
            "zombie" | "husk" | "giant" | "zombie_villager" => Family::Zombie,
            "tadpole" => Family::Tadpole,
            "drowned" => Family::Drowned,
            "skeleton" | "stray" | "wither_skeleton" | "bogged" | "parched" => Family::Skeleton,
            "player" => Family::Player,
            "player_slim" => Family::PlayerSlim,
            "elytra" => Family::Elytra,
            "silverfish" => Family::Silverfish,
            "endermite" => Family::Endermite,
            "blaze" => Family::Blaze,
            "ghast" => Family::Ghast,
            "iron_golem" => Family::IronGolem,
            "snow_golem" => Family::SnowGolem,
            "villager" | "wandering_trader" => Family::Villager,
            "piglin" | "piglin_brute" => Family::Piglin,
            "zombified_piglin" => Family::ZombifiedPiglin,
            "enderman" => Family::Enderman,
            "witch" => Family::Witch,
            "vex" => Family::Vex,
            "vindicator" | "evoker" | "pillager" | "illusioner" => Family::Illager,
            "slime" => Family::Slime,
            "magma_cube" => Family::MagmaCube,
            "polar_bear" => Family::PolarBear,
            "goat" => Family::Goat,
            "turtle" => Family::Turtle,
            "panda" => Family::Panda,
            "hoglin" | "zoglin" => Family::Hoglin,
            "wolf" => Family::Wolf,
            "llama" | "trader_llama" => Family::Llama,
            "squid" | "glow_squid" => Family::Squid,
            "dolphin" => Family::Dolphin,
            "cat" | "ocelot" => Family::Feline,
            "cod" | "tropical_fish_large" | "tropical_fish_small" => Family::Fish,
            "salmon" | "salmon_small" | "salmon_large" => Family::Salmon,
            "pufferfish_big" | "pufferfish_medium" | "pufferfish_small" => Family::Pufferfish,
            "guardian" | "elder_guardian" => Family::Guardian,
            "phantom" => Family::Phantom,
            "allay" => Family::Allay,
            "horse" | "skeleton_horse" | "zombie_horse" => Family::Horse,
            "donkey" | "mule" => Family::Donkey,
            "bee" => Family::Bee,
            "parrot" => Family::Parrot,
            "fox" if name.ends_with("_baby") => Family::BabyFox,
            "fox" => Family::Fox,
            "axolotl" if name.ends_with("_baby") => Family::BabyAxolotl,
            "axolotl" => Family::Axolotl,
            "bat" => Family::Bat,
            "rabbit" => Family::Rabbit,
            "frog" => Family::Frog,
            "armadillo" => Family::Armadillo,
            "camel" | "camel_husk" => Family::Camel,
            "sniffer" => Family::Sniffer,
            "breeze" => Family::Breeze,
            "creaking" => Family::Creaking,
            "warden" => Family::Warden,
            "copper_golem" => Family::CopperGolem,
            "nautilus" | "zombie_nautilus" => Family::Nautilus,
            "ender_dragon" => Family::EnderDragon,
            "happy_ghast" => Family::HappyGhast,
            "strider" => Family::Strider,
            "ravager" => Family::Ravager,
            "shulker" => Family::Shulker,
            "wither" => Family::Wither,
            "armor_stand" | "armor_stand_small" => Family::ArmorStand,
            _ => return None,
        })
    }
}
