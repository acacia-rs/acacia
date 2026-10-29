//! The tracked entities a player collides with (see `acacia_physics::WithEntities`).

use acacia_physics::{Aabb, Vec3};

use super::{Entities, Entity};

/// Kinds solid to a player on strict BDS, with the height their wire position sits above their box bottom.
/// Minecarts are not: a player walks through one with no correction (drill `entity_drill`).
const SOLID: &[(&str, f32)] = &[("minecraft:boat", 0.375), ("minecraft:chest_boat", 0.375)];

impl Entity {
    /// The collision box a player meets, for the kinds solid to players.
    pub fn solid_box(&self) -> Option<Aabb> {
        let &(_, lift) = SOLID.iter().find(|(k, _)| *k == self.kind)?;
        let (w, h) = self.metadata.bounding_box()?;
        let (r, p) = (w * 0.5, &self.position);
        let y = p.y - lift;
        Some(Aabb::new(p.x - r, y, p.z - r, p.x + r, y + h, p.z + r))
    }
}

impl Entities {
    /// Solid entity boxes within `reach` blocks of `feet` (horizontally and vertically).
    pub fn solid_boxes(&self, feet: Vec3, reach: f32) -> Vec<Aabb> {
        let near = |b: &Aabb| (0..3).all(|i| b.min[i] - reach <= feet[i] && feet[i] <= b.max[i] + reach);
        self.iter().filter_map(Entity::solid_box).filter(near).collect()
    }
}
