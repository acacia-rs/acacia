//! Ports of Java's `EntityModel.setupAnim`, one per model family, applied to a model's
//! [`Poses`] from its [`RenderState`]. After Horizon's `mobs/anim.rs` (MIT), whose ports its
//! harness proves against the 26.3 client; ours are proven by `tools/parity/entity_poses`.

mod arms;
mod humanoid;
mod quadruped;

pub use arms::ArmPose;
pub use humanoid::Humanoid;
pub use quadruped::sheep_eat_scales;

use super::mth::{cos, sin};
use super::poses::Poses;
use super::state::RenderState;

/// The `setupAnim` a model runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// `QuadrupedModel`: cows, pigs.
    Quadruped,
    Sheep,
    Chicken,
    Creeper,
    Spider,
    /// `ZombieModel`, `DrownedModel`.
    Zombie,
    /// `SkeletonModel`.
    Skeleton,
}

impl Family {
    /// The family of a layer (`minecraft:cow#main`, `minecraft:zombie_baby#main`); `None` for
    /// one not ported yet.
    pub fn of_layer(layer: &str) -> Option<Family> {
        let (name, part) = layer.trim_start_matches("minecraft:").split_once('#').unwrap_or((layer, "main"));
        if part != "main" {
            return None;
        }
        Some(match name.trim_end_matches("_baby") {
            "cow" | "cold_cow" | "warm_cow" | "mooshroom" | "pig" | "cold_pig" | "warm_pig" => Family::Quadruped,
            "sheep" => Family::Sheep,
            "chicken" | "cold_chicken" | "warm_chicken" => Family::Chicken,
            "creeper" => Family::Creeper,
            "spider" | "cave_spider" => Family::Spider,
            "zombie" | "husk" | "drowned" | "giant" => Family::Zombie,
            "skeleton" | "stray" | "wither_skeleton" | "bogged" | "parched" => Family::Skeleton,
            _ => return None,
        })
    }
}

/// Runs `family`'s `setupAnim` on poses fresh from the model (every part at its initial pose).
pub fn setup(family: Family, s: &RenderState, p: &mut Poses) {
    match family {
        Family::Quadruped => quadruped::quadruped(p, s),
        Family::Sheep => quadruped::sheep(p, s),
        Family::Chicken => quadruped::chicken(p, s),
        Family::Creeper => quadruped::creeper(p, s),
        Family::Spider => quadruped::spider(p, s),
        Family::Zombie => {
            let h = Humanoid { zombie: Some(s.flag("isAggressive")), ..Humanoid::of(s) };
            humanoid::humanoid(p, s, &h);
        }
        Family::Skeleton => {
            humanoid::humanoid(p, s, &Humanoid::of(s));
            if s.flag("isAggressive") && !s.flag("isHoldingBow") {
                humanoid::skeleton_melee(p, s);
            }
        }
    }
}

/// The head follows the look: `xRot`, `yRot` in degrees.
fn head(p: &mut Poses, s: &RenderState) {
    p.x_rot("head", s.x_rot().to_radians());
    p.y_rot("head", s.y_rot().to_radians());
}

/// `AnimationUtils.bobModelPart`.
fn bob(p: &mut Poses, name: &str, age: f32, scale: f32) {
    let part = p.get(name);
    part.z_rot += scale * (cos(age * 0.09) * 0.05 + 0.05);
    part.x_rot += scale * (sin(age * 0.067) * 0.05);
}

#[cfg(test)]
mod tests;
