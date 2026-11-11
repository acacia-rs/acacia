//! Solving a leap by rollout: the jump timing, coast and takeoff yaw that land it in fewest ticks without
//! a hazard, ceiling or hard fall, and for `Human` under every perturbation of the profile. The planner
//! solves from a node's momentum, the follower again from the live state when a leap drifts.

use acacia_physics::BlockPos;

use super::control::{Controller, centre};
use super::leap::{JumpAt, LeapCtl};
use super::profile::{Aim, MovementProfile};
use super::rollout::{self, Body};
use crate::movement::{Controls, Motion, Sim, StepIn};

/// Backstop for a rollout; every controller gives up long before.
const ROLLOUT_CAP: u32 = 200;
/// Last tick a jump is tried on; later ones have run off any edge.
pub(super) const MAX_TAKEOFF: u8 = 12;
/// Latest coast tried on an overshoot (air ticks before letting go of forward).
pub(super) const MAX_COAST: u8 = 8;
/// Falls up to 3 blocks do no damage.
const MAX_SAFE_FALL: f32 = 3.0;
/// Most a fall of under 5 blocks drops in one tick: the landing tick's drop is not counted as fallen.
const LAST_TICK_FALL: f32 = 0.8;
/// Seed of the planned aim's jitter stream.
pub(crate) const AIM_SEED: u64 = 0x5eed;

/// Which leaps a solve tries.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Shape {
    /// Takeoff yaws (degrees off the target line).
    pub yaws: &'static [f32],
    pub air_aim: bool,
    /// Also try running off the edge without a jump.
    pub drop: bool,
}

pub(crate) const STRAIGHT: Shape = Shape { yaws: &[0.0], air_aim: false, drop: false };
/// Off-line takeoff yaws and steering in the air, for leaps that miss straight (diagonals).
pub(crate) const WIDE: Shape = Shape { yaws: &[0.0, 11.25, -11.25, 22.5, -22.5], air_aim: true, drop: false };

/// Why a solve found no leap: every jump fell short (so farther targets on the line will too), or not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Miss {
    Short,
    Other,
}

#[derive(Clone)]
pub(crate) struct Solved {
    /// Not yet started.
    pub ctl: LeapCtl,
    /// Worst ticks over the perturbations.
    pub ticks: u32,
    /// Least distance from the landing feet to an unsafe side of the target cell (0.5 if none is).
    pub margin: f32,
    /// Where the nominal run ends.
    pub end: Motion,
}

pub(crate) struct Solver<'a, W: ?Sized> {
    pub w: &'a W,
    /// Cells the body must not touch.
    pub avoid: &'a dyn Fn(BlockPos) -> bool,
    /// Clearance kept from avoided cells.
    pub pad: f32,
    /// Landing in this cell instead would do as well (a floor at the same level).
    pub safe: &'a dyn Fn(BlockPos) -> bool,
    pub profile: MovementProfile,
    /// Check the profile's perturbations (planning); off when re-solving live.
    pub perturb: bool,
    /// Controls held before the first tick (a leap leaves the yaw alone in the air).
    pub held: Controls,
    pub rollouts: u32,
    /// Ticks simulated.
    pub ticks: u64,
    /// Ticks it may simulate: callers check [`Solver::spent`] between moves.
    pub budget: u64,
    /// A rollout was skipped for want of budget ([`Solver::affords`]).
    pub starved: bool,
    /// Index of the perturbation that last failed a judgement.
    pub(super) fails_first: usize,
}

pub(super) struct Attempt {
    /// The controller as it started.
    pub ctl: LeapCtl,
    pub takeoff: Option<u16>,
    pub bodies: Vec<Body>,
    pub end: Motion,
    pub clean: bool,
}

impl Attempt {
    pub fn last(&self) -> Body {
        self.bodies[self.bodies.len() - 1]
    }

    pub fn ticks(&self) -> u32 {
        self.bodies.len() as u32 - 1
    }
}

impl<'a, W: ?Sized> Solver<'a, W> {
    pub fn new(w: &'a W, avoid: &'a dyn Fn(BlockPos) -> bool, safe: &'a dyn Fn(BlockPos) -> bool, profile: MovementProfile, perturb: bool, held: Controls) -> Self {
        Self { w, avoid, pad: profile.hazard_pad(), safe, profile, perturb, held, rollouts: 0, ticks: 0, budget: u64::MAX, starved: false, fails_first: 0 }
    }

    pub fn spent(&self) -> bool {
        self.ticks >= self.budget
    }

    /// A rollout of up to `n` more ticks stays within the budget (a hard cap, unlike [`Solver::spent`]).
    pub fn affords(&mut self, n: u64) -> bool {
        let fits = self.ticks + n <= self.budget;
        self.starved |= !fits;
        fits
    }
}

impl<W: ?Sized> Solver<'_, W> {
    /// Runs `ctl` from `start` (until it ends itself): its bodies, end motion, and whether it finished cleanly.
    pub fn run<S: StepIn<W>>(&mut self, start: &S, ctl: &mut impl Controller, wet: bool) -> (Vec<Body>, Motion, bool) {
        let mut r = start.clone();
        let trace = rollout::run(&mut r, ctl, self.held, ROLLOUT_CAP, self.w);
        (self.rollouts, self.ticks) = (self.rollouts + 1, self.ticks + u64::from(trace.ticks));
        let clean = trace.landed && self.clean(&trace.bodies, wet);
        (trace.bodies, r.motion(), clean)
    }

    pub(super) fn attempt<S: StepIn<W>>(&mut self, start: &S, ctl: LeapCtl) -> Attempt {
        let mut running = ctl;
        let (bodies, end, clean) = self.run(start, &mut running, ctl.wet);
        Attempt { ctl, takeoff: running.takeoff(), bodies, end, clean }
    }

    /// No wall bump on the ground or rising, no ceiling, no hard fall into a dry landing, nothing avoided touched.
    fn clean(&self, bodies: &[Body], wet: bool) -> bool {
        bodies.iter().all(|b| {
            !(b.h_collision && (b.on_ground || b.vel[1] >= 0.0)) && !b.bonk && (wet || b.fall <= MAX_SAFE_FALL) && !self.touches(b)
        })
    }

    /// Distance from the landing feet `b` to the nearest side of `target`'s cell with no safe cell beyond it.
    pub(super) fn margin(&self, b: &Body, target: BlockPos) -> f32 {
        let (x, z) = (b.pos[0] - target[0] as f32, b.pos[2] - target[2] as f32);
        let sides = [(x, -1, 0), (1.0 - x, 1, 0), (z, 0, -1), (1.0 - z, 0, 1)];
        let unsafe_sides = sides.into_iter().filter(|&(_, dx, dz)| !(self.safe)([target[0] + dx, target[1], target[2] + dz]));
        unsafe_sides.map(|(d, ..)| d).fold(0.5, f32::min)
    }

    fn touches(&self, b: &Body) -> bool {
        let r = 0.3 + self.pad;
        // No pad below the feet: a leap over lava level with the floor passes above it.
        let lo = [b.pos[0] - r, b.pos[1], b.pos[2] - r].map(|v| v.floor() as i32);
        let hi = [b.pos[0] + r, b.pos[1] + 1.8 + self.pad, b.pos[2] + r].map(|v| v.floor() as i32);
        (lo[0]..=hi[0]).any(|x| (lo[1]..=hi[1]).any(|y| (lo[2]..=hi[2]).any(|z| (self.avoid)([x, y, z]))))
    }

    /// The fastest clean leap from `start` to the cell `target` (feet `feet`, water if `wet`).
    pub fn solve<S: StepIn<W>>(&mut self, start: &S, target: BlockPos, feet: f32, wet: bool, shape: Shape) -> Result<Solved, Miss> {
        let (mut best, mut short): (Option<Solved>, bool) = (None, true);
        let mut settle = |this: &mut Self, a: Attempt| match this.land(start, a) {
            Ok(a) => {
                short = false;
                this.judge(start, a)
            }
            Err(s) => {
                short &= s;
                None
            }
        };
        let jumps = wet || !falls_too_far(start, feet);
        for &yaw in shape.yaws {
            let mut ctl = LeapCtl::new(target, feet, JumpAt::Never, None, Aim::new(self.profile, AIM_SEED));
            (ctl.wet, ctl.yaw_off, ctl.air_aim) = (wet, yaw, shape.air_aim);
            // The air time barely depends on the takeoff, so the earliest takeoff that lands is the fastest.
            let (mut jumped, mut last) = (None, None);
            let reaching = if jumps { self.first_reaching(start, ctl) } else { None };
            let (first, mut known) = reaching.unwrap_or((MAX_TAKEOFF + 1, None));
            for k in first..=MAX_TAKEOFF {
                let mut c = ctl;
                c.jump = JumpAt::Tick(k);
                let a = known.take().unwrap_or_else(|| self.attempt(start, c));
                // A jump due after the edge never comes: the later ones neither.
                let Some(takeoff) = a.takeoff else { break };
                if last.replace(takeoff) == Some(takeoff) {
                    continue;
                }
                jumped = settle(self, a);
                if jumped.is_some() {
                    break;
                }
            }
            let dropped = if shape.drop {
                let a = self.attempt(start, ctl);
                settle(self, a)
            } else {
                None
            };
            for s in [jumped, dropped].into_iter().flatten() {
                if best.as_ref().is_none_or(|b| (s.ticks, -s.margin) < (b.ticks, -b.margin)) {
                    best = Some(s);
                }
            }
        }
        best.ok_or(if short { Miss::Short } else { Miss::Other })
    }

    /// A sprint-jump from `start` at once toward `aim_at`: where it comes down (no cell is required), and when.
    pub fn hop_landing<S: StepIn<W>>(&mut self, start: &S, aim_at: BlockPos, feet: f32) -> (Body, u32) {
        let ctl = LeapCtl::new(aim_at, feet, JumpAt::Tick(0), None, Aim::new(self.profile, AIM_SEED));
        let a = self.attempt(start, ctl);
        (a.last(), a.ticks())
    }

    /// A sprint-jump from `start` at once into `target`'s cell (or beside it, on the level), judged like any leap.
    pub fn hop<S: StepIn<W>>(&mut self, start: &S, target: BlockPos, feet: f32) -> Option<Solved> {
        let mut ctl = LeapCtl::new(target, feet, JumpAt::Tick(0), None, Aim::new(self.profile, AIM_SEED));
        ctl.slack = true;
        let a = self.attempt(start, ctl);
        if a.clean { self.judge(start, a) } else { None }
    }
    /// The earliest takeoff tick whose jump is not short of the target, `None` if even the jump at the edge
    /// is. A later takeoff has more run-up nearer the gap, so it flies farther: bisection finds it.
    /// Also returns that takeoff's attempt when the bisection ran it.
    pub(super) fn first_reaching<S: StepIn<W>>(&mut self, start: &S, ctl: LeapCtl) -> Option<(u8, Option<Attempt>)> {
        let reaches = |this: &mut Self, jump: JumpAt| {
            let mut c = ctl;
            c.jump = jump;
            let a = this.attempt(start, c);
            let reached = a.clean || !short(start, &a);
            (a, reached)
        };
        let (edge, reached) = reaches(self, JumpAt::Edge);
        // No edge ahead (a wall or a long run-up): try every takeoff.
        let Some(edge) = edge.takeoff else { return Some((0, None)) };
        if !reached {
            return None;
        }
        let (mut lo, mut hi, mut at_hi) = (0, edge.min(u16::from(MAX_TAKEOFF)) as u8, None);
        while lo < hi {
            let mid = (lo + hi) / 2;
            match reaches(self, JumpAt::Tick(mid)) {
                (a, false) if a.takeoff.is_some() => lo = mid + 1,
                (a, _) => (hi, at_hi) = (mid, Some(a)),
            }
        }
        Some((lo, at_hi))
    }

    /// `a` if it landed, else the coast that lands it when it went long (binary search: a later coast
    /// flies farther). `Err(true)`: it fell short.
    pub(super) fn land<S: StepIn<W>>(&mut self, start: &S, a: Attempt) -> Result<Attempt, bool> {
        if a.clean {
            return Ok(a);
        }
        let ctl = a.ctl;
        let short = |b: &Attempt| short(start, b);
        if short(&a) {
            return Err(true);
        }
        let (mut lo, mut hi) = (0, i32::from(MAX_COAST));
        while lo <= hi {
            let mid = (lo + hi) / 2;
            let mut c = ctl;
            c.coast = Some(mid as u8);
            let b = self.attempt(start, c);
            if b.clean {
                return Ok(b);
            }
            if short(&b) { lo = mid + 1 } else { hi = mid - 1 }
        }
        Err(false)
    }
}

/// Any jump from `start` down to a dry floor at `feet` counts over [`MAX_SAFE_FALL`] fallen from its apex before
/// it lands (a jumped down-3: 4.25 less the landing tick's ≤ 0.8), so only running off the edge can land there.
/// Not under effects that change the fall.
fn falls_too_far<S: Sim>(start: &S, feet: f32) -> bool {
    !start.traits().altered_fall && start.body().pos[1] - feet + S::envelope().jump_rise - LAST_TICK_FALL > MAX_SAFE_FALL
}

/// The attempt came down over half a block short of its target's centre, along the line from `start`.
fn short<S: Sim>(start: &S, a: &Attempt) -> bool {
    let pos = start.body().pos;
    let from = [pos[0], pos[2]];
    let end = a.last();
    along(from, a.ctl.target, [end.pos[0], end.pos[2]]) < along(from, a.ctl.target, centre(a.ctl.target)) - 0.5
}

/// How far along the line from `from` to `target`'s centre `at` lies (blocks).
fn along(from: [f32; 2], target: BlockPos, at: [f32; 2]) -> f32 {
    let [tx, tz] = centre(target);
    let (sx, sz) = (tx - from[0], tz - from[1]);
    ((at[0] - from[0]) * sx + (at[1] - from[1]) * sz) / sx.hypot(sz).max(1e-3)
}
