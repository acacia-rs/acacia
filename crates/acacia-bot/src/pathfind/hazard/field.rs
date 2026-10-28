//! The search's view of the threats: cost spheres (search score only: `g`, never the planned ticks) and the cells
//! solid entities fill (never entered). Built once per search from a [`Threats`] snapshot; motion enters only
//! through each sphere's growth by the threat's speed.

use acacia_physics::{BlockPos, Vec3};

use super::{Class, Threat, Threats};
use crate::pathfind::profile::MovementProfile;

/// Ticks of a threat's approach a sphere covers ahead.
const PREDICT_TICKS: f32 = 10.0;

/// One cost sphere: `weight · (1 − d/radius)²` ticks at distance `d` from `centre`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    pub centre: Vec3,
    pub radius: f32,
    pub weight: f32,
}

impl Sphere {
    fn cost(&self, p: Vec3) -> f32 {
        let d2: f32 = (0..3).map(|i| (p[i] - self.centre[i]).powi(2)).sum();
        if d2 >= self.radius * self.radius {
            return 0.0;
        }
        let k = 1.0 - d2.sqrt() / self.radius;
        self.weight * k * k
    }
}

/// (radius, weight) of a class's sphere before the speed growth; `None` for no sphere.
fn shape(t: &Threat, profile: MovementProfile) -> Option<(f32, f32)> {
    let human = profile == MovementProfile::Human;
    let wide = |optimal: f32, human_r: f32| if human { human_r } else { optimal };
    match t.class {
        Class::Melee => Some((wide(3.0, 4.0), 40.0)),
        Class::Neutral if t.angry => Some((wide(3.0, 4.0), 40.0)),
        Class::Neutral => Some((wide(1.5, 2.0), 5.0)),
        Class::Creeper if t.lit => Some((wide(7.0, 9.0), 120.0)),
        Class::Creeper => Some((wide(4.0, 6.0), 60.0)),
        Class::Ranged => Some((wide(8.0, 12.0), 15.0)),
        Class::Player => Some((1.0, 5.0)),
        Class::Passive => Some((1.0, 8.0)),
        Class::Solid | Class::Projectile => None,
    }
}

/// The threats as costs and closed cells for one search.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Hazards {
    pub spheres: Vec<Sphere>,
    /// Boxes (min, max) of the solid entities: the cells they reach into are closed.
    pub solids: Vec<(Vec3, Vec3)>,
    /// Lit creepers (box bottom centre): to get away from now (the `Evade` process).
    pub blasts: Vec<Vec3>,
}

impl Hazards {
    pub fn new(threats: &Threats, profile: MovementProfile) -> Hazards {
        let mut h = Hazards::default();
        for t in &threats.list {
            if t.class == Class::Solid {
                h.solids.push(t.bounds());
                continue;
            }
            if t.class == Class::Creeper && t.lit {
                h.blasts.push(t.feet);
            }
            let Some((radius, weight)) = shape(t, profile) else { continue };
            let speed = t.speed.max((t.motion[0].powi(2) + t.motion[2].powi(2)).sqrt());
            let mid = [t.feet[0], t.feet[1] + t.size.1 * 0.5, t.feet[2]];
            h.spheres.push(Sphere { centre: mid, radius: radius + speed * PREDICT_TICKS, weight });
        }
        h
    }

    pub fn is_empty(&self) -> bool {
        self.spheres.is_empty() && self.solids.is_empty()
    }

    /// Extra score (ticks) for a body standing with its feet cell at `p`, measured at the body's middle.
    pub fn cost(&self, p: BlockPos) -> f32 {
        let mid = [p[0] as f32 + 0.5, p[1] as f32 + 0.9, p[2] as f32 + 0.5];
        self.spheres.iter().map(|s| s.cost(mid)).sum()
    }

    /// The score of a move from `from` to `to`: every cell its straight line passes, the end included.
    pub fn edge(&self, from: BlockPos, to: BlockPos) -> f32 {
        if self.spheres.is_empty() {
            return 0.0;
        }
        let d = [0, 1, 2].map(|i| to[i] - from[i]);
        let n = d.iter().map(|v| v.abs()).max().unwrap_or(0).max(1);
        let at = |k: i32| [0, 1, 2].map(|i| from[i] + (d[i] as f32 * k as f32 / n as f32).round() as i32);
        (1..=n).map(|k| self.cost(at(k))).sum()
    }

    /// A solid entity's box reaches into cell `p`.
    pub fn closed(&self, p: BlockPos) -> bool {
        let lo = p.map(|v| v as f32);
        self.solids.iter().any(|(min, max)| (0..3).all(|i| max[i] > lo[i] + 1e-3 && min[i] < lo[i] + 1.0 - 1e-3))
    }
}
