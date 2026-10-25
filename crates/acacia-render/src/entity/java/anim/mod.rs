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
mod keyframed;
mod layers;
mod dragon;

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
    BabyFox,
    BabyAxolotl,
    Bat,
    Rabbit,
    Frog,
    Armadillo,
    Camel,
    Sniffer,
    Breeze,
    Creaking,
    Warden,
    CopperGolem,
    Nautilus,
    EnderDragon,
    HappyGhast,
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
        Family::Fox => critters::fox(p, s, false),
        Family::BabyFox => critters::fox(p, s, true),
        Family::BabyAxolotl => keyframed::baby_axolotl(p, s),
        Family::Bat => keyframed::bat(p, s),
        Family::Rabbit => keyframed::rabbit(p, s),
        Family::Frog => keyframed::frog(p, s),
        Family::Armadillo => keyframed::armadillo(p, s),
        Family::Camel => keyframed::camel(p, s),
        Family::Sniffer => keyframed::sniffer(p, s),
        Family::Breeze => keyframed::breeze(p, s),
        Family::Creaking => keyframed::creaking(p, s),
        Family::Warden => keyframed::warden(p, s),
        Family::CopperGolem => keyframed::copper_golem(p, s),
        Family::Nautilus => keyframed::nautilus(p, s),
        Family::EnderDragon => dragon::ender_dragon(p, s),
        Family::HappyGhast => simple::happy_ghast(p, s),
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
