//! A dropped item's copies for one frame, placed as Java's `ItemEntityRenderer` places them.

use std::f32::consts::TAU;

use glam::{DVec3, Mat3, Vec3};

use super::ItemModel;
use crate::blocks::placed::Draw;
use crate::entity::{EntityInstance, Pose};
use crate::look::Dropped;

/// Java draws a stack the thin way when the model is at most this deep.
const FLAT_DEPTH: f32 = 0.0625;

/// One dropped item entity.
pub struct Drop {
    pub feet: DVec3,
    pub count: u16,
    /// Seeds where the copies stray: Java's item id plus damage; here the network id plus aux.
    pub seed: i32,
    pub age_ticks: f32,
    /// From [`bob_offset`].
    pub bob_offset: f32,
}

/// Java picks it at random per entity; here it follows from the runtime id, so it stays put
/// across snapshots.
pub fn bob_offset(runtime_id: u64) -> f32 {
    Draw::new(runtime_id as i64).float() * TAU
}

pub fn copies(motion: &Dropped, count: u16) -> usize {
    1 + motion.copies.iter().filter(|&&past| count > past).count()
}

pub fn instances(model: &ItemModel, motion: &Dropped, drop: &Drop) -> Vec<EntityInstance> {
    let ground = if model.block { motion.block } else { motion.flat };
    let depth = if model.block { 1.0 } else { 1.0 / 16.0 } * ground.scale;
    // The model is a unit box around the origin before the ground transform.
    let lowest = ground.lift - 0.5 * ground.scale;
    let bob = (drop.age_ticks / motion.bob_ticks + drop.bob_offset).sin() * motion.bob + motion.bob;
    let spin = drop.age_ticks / motion.spin_ticks + drop.bob_offset;
    let base = drop.feet + DVec3::Y * f64::from(bob - lowest + motion.hover);
    let turn = Mat3::from_rotation_y(spin);

    let n = copies(motion, drop.count);
    let mut draw = Draw::new(i64::from(drop.seed));
    let mut stray = |reach: f32| (draw.float() * 2.0 - 1.0) * reach;
    let offsets: Vec<Vec3> = if depth > FLAT_DEPTH {
        let rest = (1..n).map(|_| Vec3::new(stray(motion.scatter), stray(motion.scatter), stray(motion.scatter)));
        std::iter::once(Vec3::ZERO).chain(rest).collect()
    } else {
        let step = depth * 1.5;
        let first = -(step * (n - 1) as f32 / 2.0);
        let rest = (1..n).map(|i| Vec3::new(stray(motion.scatter * 0.5), stray(motion.scatter * 0.5), first + step * i as f32));
        std::iter::once(Vec3::new(0.0, 0.0, first)).chain(rest).collect()
    };
    offsets
        .into_iter()
        .map(|offset| EntityInstance {
            layers: model.layers.clone(),
            skin: Some(model.skin.clone()),
            position: base + (turn * (offset + Vec3::Y * ground.lift)).as_dvec3(),
            // The instance turns by -yaw; Java's spin is a turn about +y.
            yaw: -spin.to_degrees(),
            scale: ground.scale,
            pose: Pose::default(),
            frame: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_follow_java_stack_sizes() {
        let counts = [1, 2, 16, 17, 32, 33, 48, 49, 64].map(|c| copies(&Dropped::JAVA, c));
        assert_eq!(counts, [1, 2, 2, 3, 3, 4, 4, 5, 5]);
    }

    #[test]
    fn java_float_matches_java_util_random() {
        // new java.util.Random(42).nextFloat()
        assert_eq!(Draw::new(42).float(), 0.7275637);
    }
}
