//! Executing a macro move live: the follower hands the move's controller the live body every tick and
//! checks it against the prediction made when the move started.

use acacia_physics::{BlockPos, Vec3};

use super::PathNode;
use super::node::Entry;
use super::control::{Controller, Phase};
use super::mac::Macro;
use super::rollout::{self, Body, Spend};
use super::shortcut::ShortcutCtl;
use super::solve::{STRAIGHT, Solver, WIDE};
use crate::cadence::splitmix;
use crate::movement::{Controls, SimWorld, StepIn};

/// Prediction horizon; controllers fail long before it.
const MAX_TICKS: u32 = 120;

/// How a finished macro move went, for tests and logs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MacroReport {
    /// Ticks the first prediction took.
    pub predicted: usize,
    pub executed: usize,
    /// Largest live-vs-predicted difference seen within tolerance.
    pub max_error: f32,
    /// Times the live body left the tolerance and the move was predicted (or solved) again from it.
    pub repredicted: u32,
    /// The planned controller did not finish from the live start and was solved again.
    pub resolved: bool,
}

pub(super) enum MacroStep {
    /// The current node is no macro move (or has no live state to run one): steer classically.
    Idle,
    Going,
    /// The move is due but this tick's rollout budget is spent: start it on the next tick.
    Wait,
    /// The move finished this tick, on the path node of this index.
    Done(usize),
    Diverged,
}

/// The follower's macro side: the move running now, if any.
#[derive(Default)]
pub(super) struct MacroRunner {
    exec: Option<Exec>,
    /// Path index whose macro move has finished.
    done: Option<usize>,
    report: Option<MacroReport>,
    /// Jitter streams of the moves run live.
    pub rng: u64,
}

impl MacroRunner {
    pub fn active(&self) -> bool {
        self.exec.is_some()
    }

    /// The macro move of path index `index` has run.
    pub fn finished(&self, index: usize) -> bool {
        self.done == Some(index)
    }

    pub fn take_report(&mut self) -> Option<MacroReport> {
        self.report.take()
    }

    /// Runs a shortcut the follower found, predicted as `predicted`, ending on path index `last`.
    pub fn begin(&mut self, ctl: ShortcutCtl, predicted: Vec<Body>, last: usize) {
        let report = MacroReport { predicted: predicted.len() - 1, executed: 0, max_error: 0.0, repredicted: 0, resolved: false };
        let tolerance = ctl.aim.profile.tolerance();
        self.exec = Some(Exec { ctl: Macro::Shortcut(ctl), predicted, t: 0, tolerance, want: None, last, report });
    }

    /// Runs the move under way, else starts the macro move of `path[index]` if due, setting `c` for the tick
    /// after `live`.
    pub fn tick<S: StepIn<W>, W: SimWorld + ?Sized>(&mut self, path: &[PathNode], index: usize, live: Option<&S>, w: &W, c: &mut Controls, spend: &mut Spend) -> MacroStep {
        let Some(live) = live else { return MacroStep::Idle };
        let exec = match &mut self.exec {
            Some(e) => e,
            None => {
                let Some(mac) = path[index].mac.filter(|_| self.done != Some(index)) else { return MacroStep::Idle };
                // Planned from standing on the node before: a classic jump onto it still in the air (an Ascend counts as
                // reached mid-air) lands first, steered classically.
                if !live.body().on_ground {
                    return MacroStep::Idle;
                }
                let mac = mac.reseeded(splitmix(&mut self.rng));
                // A node entered at rest (water) plans no arrival velocity.
                let want = (path[index].entry != Entry::Rest).then(|| path[index].mom.velocity());
                // Rollouts already spent this tick went on the shortcut search, which runs once per node.
                let starved = spend.ticks > 0;
                match Exec::start(mac, want, index, live, *c, w, spend) {
                    Some(e) => self.exec.insert(e),
                    None if starved && spend.short => return MacroStep::Wait,
                    None => return MacroStep::Diverged,
                }
            }
        };
        match exec.tick(live, w, c, spend) {
            Phase::Going => MacroStep::Going,
            Phase::Done => {
                let last = exec.last;
                (self.report, self.done) = (Some(exec.report), Some(last));
                self.exec = None;
                MacroStep::Done(last)
            }
            Phase::Failed => {
                self.exec = None;
                MacroStep::Diverged
            }
        }
    }
}

struct Exec {
    ctl: Macro,
    predicted: Vec<Body>,
    /// Index into `predicted` of the body expected now.
    t: usize,
    tolerance: f32,
    /// The planned arrival velocity, if any.
    want: Option<Vec3>,
    /// The path index the move ends on.
    last: usize,
    report: MacroReport,
}

/// Arriving this much faster or slower (blocks/tick) than planned starts the next move off its plan.
const ARRIVAL_SLOP: f32 = 0.03;

impl Exec {
    /// Predicts `ctl` from the live state; solves it again if it does not finish from there, or arrives
    /// too far from the planned velocity `want`.
    fn start<S: StepIn<W>, W: SimWorld + ?Sized>(ctl: Macro, want: Option<Vec3>, last: usize, live: &S, c: Controls, w: &W, spend: &mut Spend) -> Option<Self> {
        let off = |p: &(Macro, Vec<Body>)| {
            let end = p.1[p.1.len() - 1].vel;
            want.map_or(0.0, |v| (end[0] - v[0]).hypot(end[2] - v[2]))
        };
        let planned = predict(ctl, live, c, w, spend).filter(|p| off(p) <= ARRIVAL_SLOP);
        let ((ctl, predicted), resolved) = match planned {
            Some(p) => (p, false),
            None => (resolve(&ctl, want, live, c, w, spend).or_else(|| predict(ctl, live, c, w, spend))?, true),
        };
        let report = MacroReport { predicted: predicted.len() - 1, executed: 0, max_error: 0.0, repredicted: 0, resolved };
        Some(Self { ctl, predicted, t: 0, tolerance: ctl.profile().tolerance(), want, last, report })
    }

    /// Steps the controller on the live body, predicting again from it when it left the tolerance.
    /// `Failed` when nothing from the live body finishes the move.
    fn tick<S: StepIn<W>, W: SimWorld + ?Sized>(&mut self, live: &S, w: &W, c: &mut Controls, spend: &mut Spend) -> Phase {
        let b = live.body();
        let err = self.predicted.get(self.t).map_or(f32::INFINITY, |p| p.error(&b));
        if err > self.tolerance {
            let again = predict(self.ctl, live, *c, w, spend).or_else(|| resolve(&self.ctl, self.want, live, *c, w, spend));
            let Some((ctl, again)) = again else { return Phase::Failed };
            tracing::debug!(t = self.t, err, "macro move off its prediction; predicted again");
            (self.ctl, self.predicted, self.t) = (ctl, again, 0);
            self.report.repredicted += 1;
        } else {
            self.report.max_error = self.report.max_error.max(err);
        }
        let phase = self.ctl.step(&b, &mut |pc: &Controls| rollout::peek(live, pc, w), c);
        if phase == Phase::Going {
            self.t += 1;
            self.report.executed += 1;
        }
        phase
    }
}

/// `ctl` run on from the live state, if it finishes (and the budget allows the run).
fn predict<S: StepIn<W>, W: ?Sized>(mut ctl: Macro, live: &S, c: Controls, w: &W, spend: &mut Spend) -> Option<(Macro, Vec<Body>)> {
    if !spend.affords(u64::from(MAX_TICKS)) {
        return None;
    }
    let start = ctl;
    let trace = rollout::run(&mut live.clone(), &mut ctl, c, MAX_TICKS, w);
    spend.ticks += u64::from(trace.ticks);
    trace.landed.then_some((start, trace.bodies))
}

/// Another way to finish `ctl`'s move from the live state, arriving near velocity `want`: a leap's other
/// timings or coasts.
fn resolve<S: StepIn<W>, W: SimWorld + ?Sized>(ctl: &Macro, want: Option<Vec3>, live: &S, held: Controls, w: &W, spend: &mut Spend) -> Option<(Macro, Vec<Body>)> {
    let avoid = |p: BlockPos| w.hazard(p);
    match ctl {
        Macro::Leap(l) => {
            let profile = l.aim.profile;
            let mut s = Solver::new(w, &avoid, &|_| false, profile, false, held);
            s.budget = spend.left();
            let found = s.retry(l, live, STRAIGHT, want).or_else(|| (!l.airborne() && !s.starved).then(|| s.retry(l, live, WIDE, want)).flatten());
            spend.charge(&s);
            found.map(|(l, b)| (Macro::Leap(l), b))
        }
        Macro::Run(_) | Macro::Shortcut(_) => None,
    }
}
