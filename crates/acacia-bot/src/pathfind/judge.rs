//! Judging a solved leap under the profile's perturbations (`docs/pathfind-plan.md` "Macro moves").

use super::control::centre;
use super::leap::{JumpAt, LeapCtl};
use super::profile::Perturbation;
use super::solve::{Attempt, Solved, Solver};
use crate::movement::{Sim, StepIn};

impl<W: ?Sized> Solver<'_, W> {
    /// The nominal landing `a` under the profile's perturbations: worst ticks, least margin, or `None`
    /// if one of them fails.
    pub(super) fn judge<S: StepIn<W>>(&mut self, start: &S, a: Attempt) -> Option<Solved> {
        let (mut ticks, mut least) = (a.ticks(), self.margin(&a.last(), a.ctl.target));
        let perturbations = if self.perturb { self.profile.perturbations() } else { &[] };
        // The perturbation that failed last goes first: neighbouring solves tend to fail alike.
        let first = self.fails_first.min(perturbations.len());
        let rest = perturbations[..first].iter().chain(perturbations.get(first + 1..).unwrap_or_default());
        for p in perturbations[first..].iter().take(1).chain(rest).filter(|p| **p != Perturbation::NOMINAL) {
            let (from, ctl) = perturbed(start, a.ctl, a.takeoff, p);
            let b = self.attempt(&from, ctl);
            if !b.clean {
                self.fails_first = perturbations.iter().position(|q| q == p).unwrap_or(0);
                return None;
            }
            (ticks, least) = (ticks.max(b.ticks()), least.min(self.margin(&b.last(), a.ctl.target)));
        }
        Some(Solved { ctl: a.ctl, ticks, margin: least, end: a.end })
    }
}

/// The start and controller of one perturbed execution.
fn perturbed<S: Sim>(start: &S, mut ctl: LeapCtl, takeoff: Option<u16>, p: &Perturbation) -> (S, LeapCtl) {
    if let Some(k) = takeoff.filter(|_| p.jump != 0) {
        ctl.jump = JumpAt::Tick((i32::from(k) + i32::from(p.jump)).clamp(0, i32::from(u8::MAX)) as u8);
    }
    ctl.aim.bias = p.yaw;
    let mut from = start.clone();
    if p.start != 0.0 {
        let [tx, tz] = centre(ctl.target);
        let b = from.body();
        let mut pos = b.pos;
        let (dx, dz) = (tx - pos[0], tz - pos[2]);
        let len = dx.hypot(dz).max(1e-3);
        pos[0] -= dx / len * p.start;
        pos[2] -= dz / len * p.start;
        from.set_motion(pos, b.vel, b.yaw);
    }
    (from, ctl)
}
