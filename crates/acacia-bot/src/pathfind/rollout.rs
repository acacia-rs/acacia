//! Running a controller forward on a fork of the simulation, and what a controller sees of the player
//! ([`Body`], in rollouts and live alike).

use super::control::{Controller, Phase};
use super::reach::Flat;
use super::terrain::Terrain;
pub use crate::movement::Body;
use crate::movement::{Controls, Sim, StepIn};

/// A simulation the planner steps on its own [`Terrain`] (and on the flat ground leap floors are measured on).
pub trait PlanSim<B>: Sim + StepIn<Flat> + for<'r> StepIn<Terrain<'r, B>> {}

impl<B, S: Sim + StepIn<Flat> + for<'r> StepIn<Terrain<'r, B>>> PlanSim<B> for S {}

/// A controller's run: `bodies[0]` is the start, one more per simulated tick.
#[derive(Debug, Clone)]
pub struct Trace {
    pub bodies: Vec<Body>,
    /// The controller finished the move (`Phase::Done`).
    pub landed: bool,
    pub ticks: u32,
}

/// Runs `ctl` on `r` from controls `c` until it finishes, fails or `max` ticks pass.
pub fn run<S: StepIn<W>, W: ?Sized>(r: &mut S, ctl: &mut impl Controller, mut c: Controls, max: u32, w: &W) -> Trace {
    let mut bodies = vec![r.body()];
    loop {
        let b = bodies[bodies.len() - 1];
        let phase = ctl.step(&b, &mut |pc: &Controls| peek(r, pc, w), &mut c);
        let ticks = bodies.len() as u32 - 1;
        if phase != Phase::Going || ticks >= max {
            return Trace { bodies, landed: phase == Phase::Done, ticks };
        }
        r.step(&c, w);
        bodies.push(r.body());
    }
}

/// Rollout ticks one follower tick may simulate, and those it has.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Spend {
    pub ticks: u64,
    pub limit: u64,
    /// Something was not simulated for want of budget.
    pub short: bool,
}

impl Spend {
    pub fn new(limit: u64) -> Self {
        Self { ticks: 0, limit, short: false }
    }

    pub fn left(&self) -> u64 {
        self.limit.saturating_sub(self.ticks)
    }

    /// A rollout of up to `n` ticks fits; marks the spend short if not.
    pub fn affords(&mut self, n: u64) -> bool {
        let fits = self.ticks + n <= self.limit;
        self.short |= !fits;
        fits
    }
}

/// The body one tick after `r` with `c` held; `r` is left as it was.
pub fn peek<S: StepIn<W>, W: ?Sized>(r: &S, c: &Controls, w: &W) -> Body {
    let mut f = r.clone();
    f.step(c, w);
    f.body()
}
