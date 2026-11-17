//! The tracked entities a player collides with (see `acacia_physics::WithEntities`).

use acacia_client::proto::types::{MetadataDictionaryItemKey as Key, MetadataDictionaryItemValueDefault as Plain};
use acacia_physics::{Aabb, Vec3, mc_sin};

use super::{Entities, Entity};

/// Kinds solid to a player on strict BDS, with the height their wire position sits above their box bottom.
/// Minecarts are not: a player walks through one with no correction (drill `entity_drill`).
const SOLID: &[(&str, f32)] = &[("minecraft:boat", 0.375), ("minecraft:chest_boat", 0.375)];

pub(crate) const SHULKER: &str = "minecraft:shulker";
/// BDS `Shulker::_peekAmountTick` moves the peek this far toward its target each tick.
const PEEK_STEP: f32 = 0.05;

impl Entity {
    /// The collision box a player meets, for the kinds solid to players.
    pub fn solid_box(&self) -> Option<Aabb> {
        if self.kind == SHULKER {
            return self.shulker_box();
        }
        let &(_, lift) = SOLID.iter().find(|(k, _)| *k == self.kind)?;
        let (w, h) = self.metadata.bounding_box()?;
        let (r, p) = (w * 0.5, &self.position);
        let y = p.y - lift;
        Some(Aabb::new(p.x - r, y, p.z - r, p.x + r, y + h, p.z + r))
    }

    /// The peek amount (0 closed .. 1 open) the shulker's metadata asks for (`ShulkerPeekId`, percent).
    fn peek_target(&self) -> f32 {
        match self.metadata.plain(Key::ShulkerPeekId) {
            Some(Plain::Int(v)) => *v as f32 * 0.01,
            Some(Plain::Byte(v)) => *v as f32 * 0.01,
            _ => 0.0,
        }
    }

    /// Steps a shulker's peek toward its target, as BDS does each tick.
    pub(crate) fn step_peek(&mut self) {
        let target = self.peek_target();
        let cur = self.peek.unwrap_or(target);
        self.peek = Some(if cur < target { (cur + PEEK_STEP).min(target) } else { (cur - PEEK_STEP).max(target) });
    }

    /// BDS `Shulker::_calculateBB`: a 0.98 shell on its attach block, its open lid extending the attached face, x/z
    /// inset 0.0001 (drill shulker_walk stopped at 0.4999; a player stood 0.98 and 1.13 up). See
    /// docs/research/shulker-peek-box.md.
    fn shulker_box(&self) -> Option<Aabb> {
        let [x, y, z] = match self.metadata.plain(Key::ShulkerAttachPos) {
            Some(Plain::Vec3i(v)) => [v.x as f32, v.y as f32, v.z as f32],
            _ => [self.position.x.floor(), self.position.y.floor(), self.position.z.floor()],
        };
        let face = match self.metadata.plain(Key::ShulkerAttachFace) {
            Some(Plain::Byte(f)) => *f,
            _ => 0,
        };
        let peek = self.peek.unwrap_or_else(|| self.peek_target());
        let open = 0.5 - mc_sin((peek + 0.5) * std::f32::consts::PI) * 0.5;
        let (px, pz) = (x + 0.5, z + 0.5);
        let (mut min, mut max) = ([px - 0.5, y, pz - 0.5], [px + 0.5, y + 0.98, pz + 0.5]);
        match face {
            0 => max[1] += open,
            1 => min[1] -= open,
            2 => max[2] += open,
            3 => min[2] -= open,
            4 => max[0] += open,
            5 => min[0] -= open,
            _ => {}
        }
        for i in [0, 2] {
            min[i] += 0.0001;
            max[i] -= 0.0001;
        }
        Some(Aabb { min, max })
    }
}

impl Entities {
    /// Solid entity boxes within `reach` blocks of `feet` (horizontally and vertically).
    pub fn solid_boxes(&self, feet: Vec3, reach: f32) -> Vec<Aabb> {
        let near = |b: &Aabb| (0..3).all(|i| b.min[i] - reach <= feet[i] && feet[i] <= b.max[i] + reach);
        self.iter().filter_map(Entity::solid_box).filter(near).collect()
    }
}
