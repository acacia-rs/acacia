//! Picks the process in control each tick and keeps the one pilot pointed at its goal. Network-free: the
//! caller drives the pilot or does the act it returns, then reports back.

use std::collections::HashMap;
use std::sync::Arc;

use acacia_physics::{BlockPos, Vec3};

use super::{Act, Ctx, Event, Look, ProcId, Process, Want};
use crate::pathfind::{Goal, NavStatus, Pilot, RolloutCache};
use crate::ActionError;

struct Entry {
    id: ProcId,
    priority: i32,
    paused: bool,
    proc: Box<dyn Process>,
    /// Last tick another process had control.
    other_at: u64,
}

/// The pilot and whose goal it drives to.
struct Route {
    owner: ProcId,
    goal: Goal,
    pilot: Pilot,
    arrived: bool,
    driven_at: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// Drive [`Arbiter::pilot_mut`] this tick, then report with [`Arbiter::drove`].
    Go,
    /// Stand still.
    Hold,
    /// Do the act, then report with [`Arbiter::acted`].
    Act(ProcId, Act),
    /// Every process yields.
    Idle,
    /// No processes left.
    Empty,
}

#[derive(Default)]
pub struct Arbiter {
    /// By priority, highest first; equal ones in the order added.
    entries: Vec<Entry>,
    next: u32,
    tick: u64,
    active: Option<ProcId>,
    route: Option<Route>,
    cache: Arc<RolloutCache>,
    outcomes: HashMap<ProcId, Result<(), ActionError>>,
}

impl Arbiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Higher `priority` wins control.
    pub fn add(&mut self, proc: impl Process + 'static, priority: i32) -> ProcId {
        let id = ProcId(self.next);
        self.next += 1;
        let at = self.entries.iter().position(|e| e.priority < priority).unwrap_or(self.entries.len());
        self.entries.insert(at, Entry { id, priority, paused: false, proc: Box::new(proc), other_at: self.tick });
        id
    }

    pub fn pause(&mut self, id: ProcId) -> bool {
        let Some(e) = self.entries.iter_mut().find(|e| e.id == id) else { return false };
        e.paused = true;
        if self.active == Some(id) {
            e.proc.lost_control();
            self.active = None;
        }
        true
    }

    pub fn resume(&mut self, id: ProcId) -> bool {
        self.entries.iter_mut().find(|e| e.id == id).map(|e| e.paused = false).is_some()
    }

    pub fn remove(&mut self, id: ProcId) -> Option<Box<dyn Process>> {
        let at = self.entries.iter().position(|e| e.id == id)?;
        let mut e = self.entries.remove(at);
        if self.active == Some(id) {
            e.proc.lost_control();
        }
        self.forget(id);
        Some(e.proc)
    }

    /// The outcome of a finished process.
    pub fn outcome(&self, id: ProcId) -> Option<&Result<(), ActionError>> {
        self.outcomes.get(&id)
    }

    pub fn take_outcome(&mut self, id: ProcId) -> Option<Result<(), ActionError>> {
        self.outcomes.remove(&id)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn active(&self) -> Option<ProcId> {
        self.active
    }

    pub fn get(&self, id: ProcId) -> Option<&dyn Process> {
        self.entries.iter().find(|e| e.id == id).map(|e| e.proc.as_ref())
    }

    pub fn pilot(&self) -> Option<&Pilot> {
        self.route.as_ref().map(|r| &r.pilot)
    }

    pub fn pilot_mut(&mut self) -> Option<&mut Pilot> {
        self.route.as_mut().map(|r| &mut r.pilot)
    }

    /// Asks the processes, highest priority first, until one wants control; points the pilot at its goal.
    pub fn decide(&mut self, pos: Vec3, on_ground: bool, look: &dyn Look) -> Decision {
        self.tick += 1;
        let tick = self.tick;
        let path = self.route.as_ref().and_then(|r| r.pilot.follower()).map_or(&[][..], |f| f.ahead());
        let hazards = self.route.as_ref().and_then(|r| r.pilot.field());
        let (mut winner, mut finished) = (None, Vec::new());
        for e in self.entries.iter_mut().filter(|e| !e.paused) {
            let cx = Ctx { tick, pos, on_ground, path, quiet: tick - e.other_at, look, hazards };
            match e.proc.want(&cx) {
                Want::Yield => {}
                Want::Done(result) => finished.push((e.id, result)),
                want => {
                    winner = Some((e.id, want));
                    break;
                }
            }
        }
        for (id, result) in finished {
            self.finish(id, result);
        }
        let Some((id, want)) = winner else {
            if let Some(old) = self.active.take() {
                self.with(old, |p| p.lost_control());
            }
            return if self.entries.is_empty() { Decision::Empty } else { Decision::Idle };
        };
        self.entries.iter_mut().filter(|e| e.id != id).for_each(|e| e.other_at = tick);
        if self.active != Some(id) {
            if let Some(old) = self.active.replace(id) {
                self.with(old, |p| p.lost_control());
            }
            tracing::debug!(proc = self.get(id).map(|p| p.name()), "process takes control");
            self.with(id, |p| p.regained());
        }
        match want {
            Want::Go(goal) => self.route_to(id, goal),
            Want::Act(act) => Decision::Act(id, act),
            _ => Decision::Hold,
        }
    }

    fn route_to(&mut self, id: ProcId, goal: Goal) -> Decision {
        let tick = self.tick;
        if self.route.as_ref().is_some_and(|r| r.owner == id && r.goal == goal && r.arrived) {
            // Told again, not held silently: a process asking anew may have forgotten (the builder after an act at
            // the stand held there forever, live).
            if self.with(id, |p| p.arrived()) == Some(true) {
                self.finish(id, Ok(()));
            }
            return Decision::Hold;
        }
        match &mut self.route {
            Some(r) if r.owner == id && r.goal == goal => {
                // Not driven last tick: the path no longer starts where the bot is.
                if r.driven_at + 1 < tick {
                    r.pilot.interrupt();
                }
                r.driven_at = tick;
            }
            _ => {
                let opts = self.get(id).map(|p| p.opts()).unwrap_or_default();
                let mut pilot = Pilot::with_cache(goal.clone(), opts, self.cache.clone());
                if let Some(old) = self.route.take() {
                    pilot.inherit(old.pilot);
                }
                self.route = Some(Route { owner: id, goal, pilot, arrived: false, driven_at: tick });
            }
        }
        Decision::Go
    }

    /// How driving went this tick (after [`Decision::Go`]); `dug`: blocks its work broke, with their identifiers.
    pub fn drove(&mut self, result: Result<NavStatus, ActionError>, dug: Vec<(BlockPos, String)>) {
        for (pos, block) in dug {
            let event = Event::Dug { pos, block };
            self.entries.iter_mut().for_each(|e| e.proc.observe(&event));
        }
        let Some(owner) = self.route.as_ref().map(|r| r.owner) else { return };
        match result {
            Ok(NavStatus::Arrived) => {
                self.route.as_mut().into_iter().for_each(|r| r.arrived = true);
                if self.with(owner, |p| p.arrived()) == Some(true) {
                    self.finish(owner, Ok(()));
                }
            }
            Ok(_) => {}
            Err(e) => {
                self.route = None;
                if self.with(owner, |p| p.failed(&e)) == Some(true) {
                    self.finish(owner, Err(e));
                }
            }
        }
    }

    /// Cells a path's work placed scaffold into this tick (after [`Decision::Go`]).
    pub fn placed(&mut self, cells: Vec<BlockPos>) {
        for pos in cells {
            let event = Event::Placed { pos };
            self.entries.iter_mut().for_each(|e| e.proc.observe(&event));
        }
    }

    /// The placement the process in control would make while driving on ([`Process::en_route`]).
    pub fn en_route(&mut self) -> Option<(ProcId, Act)> {
        let id = self.active?;
        self.with(id, |p| p.en_route()).flatten().map(|a| (id, a))
    }

    pub fn acted(&mut self, id: ProcId, act: &Act, result: &Result<(), ActionError>) {
        self.with(id, |p| p.acted(act, result));
    }

    fn with<T>(&mut self, id: ProcId, f: impl FnOnce(&mut dyn Process) -> T) -> Option<T> {
        self.entries.iter_mut().find(|e| e.id == id).map(|e| f(e.proc.as_mut()))
    }

    fn finish(&mut self, id: ProcId, result: Result<(), ActionError>) {
        tracing::debug!(proc = self.get(id).map(|p| p.name()), ?result, "process finished");
        self.entries.retain(|e| e.id != id);
        self.outcomes.insert(id, result);
        self.forget(id);
    }

    fn forget(&mut self, id: ProcId) {
        if self.active == Some(id) {
            self.active = None;
        }
        if self.route.as_ref().is_some_and(|r| r.owner == id) {
            self.route = None;
        }
    }
}
