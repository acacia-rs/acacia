//! Entity avoidance (milestone M4): what the tracked entities mean to a path. [`Threats`] is a plain snapshot of the
//! entities near the bot, so the search and the pilot stay testable offline. Design:
//! research/pathfind-entities-design.md.

mod field;
mod kinds;
mod perceive;
#[cfg(test)]
mod tests;

pub use field::{Hazards, Sphere};
pub use kinds::{Class, Kind, of as kind};
pub use perceive::{Senses, View, clear_line};

use std::sync::Arc;

use acacia_client::proto::types::MetadataFlags1;
use acacia_physics::Vec3;

use super::profile::MovementProfile;
use crate::state::{Entities, Entity};

/// One entity as a path sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Threat {
    pub runtime_id: u64,
    pub class: Class,
    /// Bottom centre of its box.
    pub feet: Vec3,
    /// (width, height) of its box.
    pub size: (f32, f32),
    /// Blocks per tick, as last seen.
    pub motion: Vec3,
    /// How fast its kind closes in (blocks per tick).
    pub speed: f32,
    /// Ticks since its position was last updated.
    pub age: u64,
    /// A creeper about to blow.
    pub lit: bool,
    /// A neutral mob turned hostile.
    pub angry: bool,
}

impl Threat {
    /// `e` as a threat, `None` for kinds a path ignores. `now` is [`Entities::now`].
    pub fn of(e: &Entity, now: u64) -> Option<Threat> {
        let kind = kinds::of(&e.kind)?;
        let feet = e.feet();
        let scale = e.metadata.scale();
        let size = e.metadata.bounding_box().unwrap_or((kind.size.0 * scale, kind.size.1 * scale));
        let flags = e.metadata.flags();
        let m = &e.motion;
        Some(Threat {
            runtime_id: e.runtime_id,
            class: kind.class,
            feet: [feet.x, feet.y - kind.lift, feet.z],
            size,
            motion: [m.x, m.y, m.z],
            speed: kind.speed,
            age: now.saturating_sub(e.moved_at),
            lit: flags.contains(MetadataFlags1::IGNITED),
            angry: flags.contains(MetadataFlags1::ANGRY),
        })
    }

    /// Its box (min, max corners).
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let (r, h) = (self.size.0 * 0.5, self.size.1);
        let [x, y, z] = self.feet;
        ([x - r, y, z - r], [x + r, y + h, z + r])
    }

    /// Hostile now: melee, creepers, ranged, and neutrals while angry.
    pub fn hostile(&self) -> bool {
        matches!(self.class, Class::Melee | Class::Creeper | Class::Ranged) || self.class == Class::Neutral && self.angry
    }
}

/// The threats within some distance of the bot at one moment.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Threats {
    pub list: Vec<Threat>,
    /// [`Entities::now`] when taken.
    pub at: u64,
}

impl Threats {
    /// The tracked entities within `radius` of `centre` (feet) that a path cares about.
    pub fn near(entities: &Entities, centre: Vec3, radius: f32) -> Threats {
        let now = entities.now();
        let close = |t: &Threat| (0..3).map(|i| (t.feet[i] - centre[i]).powi(2)).sum::<f32>() <= radius * radius;
        let list = entities.iter().filter_map(|e| Threat::of(e, now)).filter(close).collect();
        Threats { list, at: now }
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
}

/// How far from the bot entities shape a search.
pub const FIELD_RADIUS: f32 = 32.0;

/// The field of `threats` for one tick's searches and checks, `None` when nothing in it matters.
pub fn field(threats: &Threats, profile: MovementProfile) -> Option<Arc<Hazards>> {
    let h = Hazards::new(threats, profile);
    (!h.is_empty()).then(|| Arc::new(h))
}

/// The collision boxes (min, max) of the solid ones among `threats`, for physics.
pub fn solid_boxes(threats: &Threats) -> Vec<acacia_physics::Aabb> {
    let solid = threats.list.iter().filter(|t| t.class == Class::Solid);
    solid.map(|t| t.bounds()).map(|(a, b)| acacia_physics::Aabb::new(a[0], a[1], a[2], b[0], b[1], b[2])).collect()
}
