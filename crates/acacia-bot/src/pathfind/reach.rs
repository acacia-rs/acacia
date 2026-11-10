//! Floors on a leap's cost before it is solved (lazy leaps, docs/pathfind-plan.md "Core fixes"): how far the feet
//! can get in each tick from a node's momentum, simulated once per start speed on open flat ground.

use std::collections::HashMap;

use acacia_physics::{Aabb, BlockPhysics, BlockPos, WorldView};

use super::costs::{DANGER_NEAR, HUMAN_JUMP};
use super::leap::MAX_LEAP_TICKS;
use super::leap_moves::LeapCand;
use super::node::Momentum;
use super::profile::MovementProfile;
use super::terrain::{Blocks, Terrain};
use crate::movement::{Controls, Sim, SimWorld, StepIn};

/// A floor under y 0 of the given friction, air above.
pub struct Flat(f32);

impl WorldView for Flat {
    fn block_collisions(&self, pos: BlockPos, out: &mut Vec<Aabb>) {
        if pos[1] < 0 {
            out.push(Aabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0));
        }
    }

    fn block(&self, pos: BlockPos) -> BlockPhysics {
        if pos[1] < 0 { BlockPhysics::solid(self.0) } else { BlockPhysics::AIR }
    }
}

/// Ticks of sprinting before holding jump that the reach tries (`None`: never jump).
const JUMP_AFTER: [Option<usize>; 7] = [None, Some(0), Some(1), Some(2), Some(3), Some(5), Some(8)];

/// Farthest the feet get from the start by each tick, sprinting straight from a node's speed and jumping at any of
/// [`JUMP_AFTER`].
pub(crate) struct Reach {
    by_tick: Vec<f32>,
}

impl Reach {
    /// From momentum `mom` (attributes and effects from `base`), on default-friction ground.
    fn new<S: StepIn<Flat>>(mom: &Momentum, base: &S) -> Self {
        let mut start = mom.synth([0, 0, 0], base);
        let v = start.body().vel;
        let speed = v[0].hypot(v[2]);
        start.set_motion([0.5, 0.0, 0.5], [speed, v[1], 0.0], -90.0);
        let flat = Flat(S::envelope().friction);
        let mut by_tick = vec![0.0f32; usize::from(MAX_LEAP_TICKS) + 1];
        for after in JUMP_AFTER {
            let mut r = start.clone();
            for (k, d) in by_tick.iter_mut().enumerate().skip(1) {
                let jump = after.is_some_and(|a| k > a);
                r.step(&Controls { forward: 1.0, sprint: true, jump, yaw: -90.0, ..Controls::default() }, &flat);
                *d = d.max(r.body().pos[0] - 0.5);
            }
        }
        Self { by_tick }
    }

    /// Fewest ticks to cover `d` blocks (the last tick if never).
    fn ticks(&self, d: f32) -> f32 {
        self.by_tick.iter().position(|&x| x >= d).unwrap_or(self.by_tick.len() - 1) as f32
    }
}

/// A search's reaches by start speed and flags (rest nodes all share one).
#[derive(Default)]
pub(crate) struct Reaches(HashMap<(i16, i16, i16, u8), Reach>);

impl Reaches {
    pub fn get<S: StepIn<Flat>>(&mut self, mom: &Momentum, base: &S) -> &Reach {
        let v = mom.velocity();
        let speed = (v[0].hypot(v[2]) * 4096.0).round() as i16;
        self.0.entry((speed, mom.vel[1], mom.off[1], mom.flags)).or_insert_with(|| Reach::new(mom, base))
    }
}

/// Least cost the leap to `c` from `p` (moving as `mom`) can have: ticks for the feet to reach the target cell at
/// the fastest run (one tick of slack), `Human`'s hesitation when it must jump, and the hazard term.
pub(crate) fn leap_floor<B: Blocks, S: Sim>(t: &Terrain<B>, p: BlockPos, mom: &Momentum, c: &LeapCand, reach: &Reach, base: &S, profile: MovementProfile) -> f32 {
    let env = S::envelope();
    let [x, _, z] = mom.feet(p);
    let (lo, hi) = ([c.q[0] as f32, c.q[2] as f32], [c.q[0] as f32 + 1.0, c.q[2] as f32 + 1.0]);
    let d = (lo[0] - x).max(x - hi[0]).max(0.0).hypot((lo[1] - z).max(z - hi[1]).max(0.0));
    // Other floors (ice) carry more speed than the flat run assumes.
    let ticks = if t.friction([p[0], p[1] - 1, p[2]]) > env.friction {
        d / (env.leap_vmax * (base.traits().speed / env.movement_speed).max(1.0))
    } else {
        (reach.ticks(d) - 1.0).max(0.0)
    };
    let jumps = !c.to.wet && c.q[1] >= p[1];
    let hesitation = if profile == MovementProfile::Human && jumps { HUMAN_JUMP } else { 0.0 };
    let danger = if t.danger_near(c.q) { DANGER_NEAR } else { 0.0 };
    ticks + hesitation + danger
}
