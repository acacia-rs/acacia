//! What the bot knows of the threats (design decision 4). `Optimal` knows every one at once. `Human` knows one only
//! after perceiving it (in the view cone with a clear line, within earshot, or close when hurt) and a reaction delay
//! later; once known it is tracked until out of mind for [`human::THREAT_MEMORY`] ticks.

use std::collections::HashMap;

use acacia_physics::Vec3;

use super::Threats;
use crate::cadence::splitmix;
use crate::human;
use crate::pathfind::MovementProfile;
use crate::pathfind::terrain::{Blocks, Terrain};

/// Distance (blocks) within which whatever hurt the bot is taken to be the culprit.
const HURT_RADIUS: f32 = 3.0;
/// Farthest an entity is seen.
const VIEW_RANGE: f32 = 48.0;

#[derive(Debug, Clone, Copy)]
struct Known {
    /// Tick from which the bot reacts to it.
    from: u64,
    /// Tick it was last perceived.
    seen: u64,
}

/// What the bot perceives where it stands now.
pub struct View<'a> {
    pub eye: Vec3,
    /// Degrees; Bedrock yaw 0 faces +Z.
    pub yaw: f32,
    pub health: f32,
    /// A clear line between two points.
    pub sees: &'a dyn Fn(Vec3, Vec3) -> bool,
}

#[derive(Debug, Default)]
pub struct Senses {
    known: HashMap<u64, Known>,
    rng: u64,
    health: Option<f32>,
}

impl Senses {
    pub fn new(seed: u64) -> Self {
        Senses { rng: seed, ..Senses::default() }
    }

    /// The part of `threats` the bot reacts to now.
    pub fn filter(&mut self, mut threats: Threats, view: &View, profile: MovementProfile) -> Threats {
        let hurt = self.health.replace(view.health).is_some_and(|h| view.health < h);
        if profile == MovementProfile::Optimal {
            return threats;
        }
        let now = threats.at;
        for t in &threats.list {
            let mid = [t.feet[0], t.feet[1] + t.size.1 * 0.5, t.feet[2]];
            let earshot = if t.lit { human::FUSE_EARSHOT } else { human::EARSHOT };
            let Some(sense) = perceives(view, hurt, mid, earshot) else { continue };
            let (lo, hi) = match sense {
                Sense::Heard => human::THREAT_NOTICE_HEARD,
                Sense::Seen => human::THREAT_NOTICE_SEEN,
            };
            let delay = lo + splitmix(&mut self.rng) % (hi - lo + 1);
            self.known.entry(t.runtime_id).or_insert(Known { from: now + delay, seen: now }).seen = now;
        }
        self.known.retain(|_, k| now.saturating_sub(k.seen) <= human::THREAT_MEMORY);
        threats.list.retain(|t| self.known.get(&t.runtime_id).is_some_and(|k| now >= k.from));
        threats
    }
}

/// No block box on the straight line from `a` to `b` (sampled every quarter block).
pub fn clear_line<B: Blocks>(t: &Terrain<B>, a: Vec3, b: Vec3) -> bool {
    let n = ((0..3).map(|i| (b[i] - a[i]).powi(2)).sum::<f32>().sqrt() * 4.0).ceil() as i32;
    (1..n).all(|k| {
        let p = [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * k as f32 / n as f32);
        t.cell(p.map(|v| v.floor() as i32)).boxes.is_empty()
    })
}

enum Sense {
    /// In earshot (farther for a lit fuse's hiss), or close when the bot was hurt.
    Heard,
    Seen,
}

fn perceives(v: &View, hurt: bool, mid: Vec3, earshot: f32) -> Option<Sense> {
    let d = [0, 1, 2].map(|i| mid[i] - v.eye[i]);
    let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if dist <= earshot || hurt && dist <= HURT_RADIUS {
        return Some(Sense::Heard);
    }
    let yaw = v.yaw.to_radians();
    let flat = d[0].hypot(d[2]).max(1e-4);
    let cos = (-yaw.sin() * d[0] + yaw.cos() * d[2]) / flat;
    (dist <= VIEW_RANGE && cos >= human::VIEW_HALF_DEG.to_radians().cos() && (v.sees)(v.eye, mid)).then_some(Sense::Seen)
}
