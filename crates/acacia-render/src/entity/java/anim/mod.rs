//! Ports of Java's `EntityModel.setupAnim` (26.3 client), one per model family, applied to a
//! model's [`Poses`] from its [`RenderState`]. Horizon's `mobs/anim.rs` (MIT) was the starting
//! point; the harness `entity_poses` proves them.

mod arms;
mod humanoid;
mod player;
mod quadruped;
mod simple;
mod piglin;
mod monsters;
mod illager;
mod beasts;
mod tame;
mod feline;
mod flyers;
mod equine;
mod critters;
mod nether;
mod axolotl;
mod stand;

pub use arms::ArmPose;
pub use humanoid::{Humanoid, Model};
pub use quadruped::sheep_eat_scales;

use super::poses::Poses;
use super::state::RenderState;

/// The `setupAnim` a model runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// `QuadrupedModel`: cows, pigs.
    Quadruped,
    Sheep,
    /// `AdultChickenModel`.
    Chicken,
    BabyChicken,
    Creeper,
    Spider,
    /// `ZombieModel`.
    Zombie,
    /// `DrownedModel`.
    Drowned,
    /// `SkeletonModel`.
    Skeleton,
    /// `PlayerModel`, wide.
    Player,
    /// `PlayerModel`, slim: its `translateToHand` moves the arm half a pixel out.
    PlayerSlim,
    /// `PlayerCapeModel`.
    PlayerCape,
    /// `ElytraModel`.
    Elytra,
    Silverfish,
    Endermite,
    Blaze,
    Ghast,
    IronGolem,
    SnowGolem,
    /// `VillagerModel`: villagers, their babies, wandering traders.
    Villager,
    Piglin,
    /// `ZombifiedPiglinModel`.
    ZombifiedPiglin,
    Enderman,
    Witch,
    Vex,
    /// `IllagerModel`.
    Illager,
    /// `SlimeModel`: no animation of its own.
    Slime,
    MagmaCube,
    PolarBear,
    Goat,
    Turtle,
    Panda,
    /// `HoglinModel`: hoglins and zoglins.
    Hoglin,
    Wolf,
    Llama,
    Squid,
    Dolphin,
    /// `AdultFelineModel` / `BabyFelineModel`: cats and ocelots.
    Feline,
    /// `CodModel`, `TropicalFish*Model`.
    Fish,
    Salmon,
    Pufferfish,
    Guardian,
    Phantom,
    Allay,
    /// `HorseModel` / `BabyHorseModel`: horses, skeleton and zombie horses.
    Horse,
    /// `DonkeyModel` / `BabyDonkeyModel`: donkeys and mules.
    Donkey,
    Bee,
    Parrot,
    /// `AdultFoxModel`.
    Fox,
    Strider,
    Ravager,
    Shulker,
    /// `WitherBossModel`.
    Wither,
    /// `AdultAxolotlModel`.
    Axolotl,
    /// `ArmorStandModel` and its armour's `ArmorStandArmorModel`.
    ArmorStand,
    Tadpole,
}

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
            "fox" if !name.ends_with("_baby") => Family::Fox,
            "strider" => Family::Strider,
            "ravager" => Family::Ravager,
            "shulker" => Family::Shulker,
            "wither" => Family::Wither,
            "axolotl" if !name.ends_with("_baby") => Family::Axolotl,
            "armor_stand" | "armor_stand_small" => Family::ArmorStand,
            _ => return None,
        })
    }
}

/// Runs `family`'s `setupAnim` on poses fresh from the model (every part at its initial pose).
pub fn setup(family: Family, s: &RenderState, p: &mut Poses) {
    match family {
        Family::Quadruped => quadruped::quadruped(p, s),
        Family::Sheep => quadruped::sheep(p, s),
        Family::Chicken => quadruped::chicken(p, s, true),
        Family::BabyChicken => quadruped::chicken(p, s, false),
        Family::Creeper => quadruped::creeper(p, s),
        Family::Spider => quadruped::spider(p, s),
        Family::Zombie => humanoid::humanoid(p, s, &Humanoid::of(s), Model::Zombie),
        Family::Drowned => humanoid::humanoid(p, s, &Humanoid::of(s), Model::Drowned),
        Family::Skeleton => {
            humanoid::humanoid(p, s, &Humanoid::of(s), Model::Humanoid);
            p.hand_offset = 1.0;
            if s.flag("isAggressive") && !s.flag("isHoldingBow") {
                humanoid::skeleton_melee(p, s);
            }
        }
        Family::Player => player::player(p, s),
        Family::PlayerSlim => {
            player::player(p, s);
            p.hand_offset = 0.5;
        }
        Family::PlayerCape => player::cape(p, s),
        Family::Elytra => player::elytra(p, s),
        Family::Silverfish => simple::silverfish(p, s),
        Family::Endermite => simple::endermite(p, s),
        Family::Blaze => simple::blaze(p, s),
        Family::Ghast => simple::ghast(p, s),
        Family::IronGolem => simple::iron_golem(p, s),
        Family::SnowGolem => simple::snow_golem(p, s),
        Family::Villager => simple::villager(p, s),
        Family::Piglin => piglin::piglin(p, s),
        Family::ZombifiedPiglin => piglin::zombified(p, s),
        Family::Enderman => monsters::enderman(p, s),
        Family::Witch => monsters::witch(p, s),
        Family::Vex => monsters::vex(p, s),
        Family::Illager => illager::illager(p, s),
        Family::Slime => {}
        Family::MagmaCube => simple::magma_cube(p, s),
        Family::PolarBear => beasts::polar_bear(p, s),
        Family::Goat => beasts::goat(p, s),
        Family::Turtle => beasts::turtle(p, s),
        Family::Panda => beasts::panda(p, s),
        Family::Hoglin => beasts::hoglin(p, s),
        Family::Wolf => tame::wolf(p, s),
        Family::Llama => tame::llama(p, s),
        Family::Squid => tame::squid(p, s),
        Family::Dolphin => tame::dolphin(p, s),
        Family::Feline => feline::feline(p, s),
        Family::Fish => flyers::fish(p, s),
        Family::Salmon => flyers::salmon(p, s),
        Family::Pufferfish => flyers::pufferfish(p, s),
        Family::Guardian => flyers::guardian(p, s),
        Family::Phantom => flyers::phantom(p, s),
        Family::Allay => flyers::allay(p, s),
        Family::Horse => equine::equine(p, s, false),
        Family::Donkey => equine::equine(p, s, true),
        Family::Bee => critters::bee(p, s),
        Family::Parrot => critters::parrot(p, s),
        Family::Fox => critters::fox(p, s),
        Family::Strider => nether::strider(p, s),
        Family::Ravager => nether::ravager(p, s),
        Family::Shulker => nether::shulker(p, s),
        Family::Wither => nether::wither(p, s),
        Family::Axolotl => axolotl::axolotl(p, s),
        Family::ArmorStand => stand::armor_stand(p, s),
        Family::Tadpole => flyers::tadpole(p, s),
    }
}

/// The head follows the look: `xRot`, `yRot` in degrees.
fn head(p: &mut Poses, s: &RenderState) {
    p.x_rot("head", s.x_rot() * super::mth::pi_div(180.0));
    p.y_rot("head", s.y_rot() * super::mth::pi_div(180.0));
}

#[cfg(test)]
mod tests;
