//! [`Builder`] as a [`Process`]: survey, pick the next step (`plan`), drive or act, learn from the outcome.

use super::builder::Builder;
use super::journal::{self, Entry, Kind, WalkEnd};
use super::plan::{self, Next};
use super::status::{HoldWhy, Phase};
use super::survey::Need;
use crate::pathfind::process::{Act, Ctx, Event, Process, Want};
use crate::pathfind::{Goal, GotoOpts};
use crate::ActionError;

/// Ticks of waiting before stations are asked again (they may have been refilled).
const ASK_AGAIN: u64 = 600;
/// Failed walks in a row after which path work may break and fill built cells.
const ESCAPE_AFTER: u32 = 2;
/// Arrival tolerance at a stand beside its cell: under 0.5 minus the builder's body margin (`plan::BODY`).
const STAND_TOLERANCE: f32 = 0.1;

fn stands_of(goal: &Goal) -> Box<dyn Iterator<Item = acacia_physics::BlockPos> + '_> {
    match goal {
        Goal::Block(p) => Box::new(std::iter::once(*p)),
        Goal::Or(gs) => Box::new(gs.iter().flat_map(stands_of)),
        _ => Box::new(std::iter::empty()),
    }
}

/// The cell an act changes.
fn cell_of(act: &Act) -> Option<acacia_physics::BlockPos> {
    match act {
        Act::Place { against, face, .. } => Some(face.adjacent(*against)),
        Act::Break { pos } | Act::Fill { pos, .. } | Act::UseOn { cell: pos, .. } => Some(*pos),
        _ => None,
    }
}

/// `cell` is next to (or in) the body of a player standing at feet cell `feet`.
fn beside(cell: acacia_physics::BlockPos, feet: acacia_physics::BlockPos) -> bool {
    (cell[0] - feet[0]).abs() <= 1 && (cell[2] - feet[2]).abs() <= 1 && (feet[1]..=feet[1] + 1).contains(&cell[1])
}

impl Process for Builder {
    fn name(&self) -> &str {
        "builder"
    }

    fn want(&mut self, cx: &Ctx) -> Want {
        self.tick = cx.tick;
        self.check_restock(cx.look);
        let needs = self.survey(cx.look);
        self.sync(cx);
        let built = || self.cells.iter().zip(&needs).filter(|(c, n)| **n == Need::Done && !c.target.is_air()).map(|(c, _)| c.pos);
        if built().count() != self.keep.len() {
            self.keep = std::sync::Arc::new(built().collect());
        }
        self.scaffold.retain(|p| cx.look.solid(*p) != Some(false));
        // In the air: keep driving (a jump or pillar in progress), or wait to land before acting.
        let mut next = match &self.commit {
            Some((task, goal)) if !cx.on_ground => Next::Go(Phase::Building, *task, goal.clone()),
            _ => Next::Hold(Phase::Building, Vec::new(), HoldWhy::InAir),
        };
        // A task wanting the goal just reached could not act from there after all: let it rest and ask again.
        for _ in 0..8 {
            if !cx.on_ground {
                break;
            }
            next = plan::next(self, cx, &needs);
            match &next {
                Next::Go(_, task, goal) if self.arrived.as_ref() == Some(goal) => {
                    tracing::debug!(?task, ?goal, "build stand reached but no click from there");
                    self.rest(*task);
                    self.batch.clear();
                    next = Next::Hold(Phase::Building, Vec::new(), HoldWhy::ArrivedNoClick);
                }
                _ => break,
            }
        }
        if let Some(c) = self.wedged(cx) {
            tracing::debug!(?c, "build bot wedged against a thin block; breaking it");
            next = Next::Act(Phase::Building, Act::Break { pos: c });
        }
        // Every tick on the ground, planned or not: a hold that skipped the claim kept the last one, and the other
        // bot waited on it for good (live: two slabs it could click, 150 s).
        if cx.on_ground {
            let acting_on = match &next {
                Next::Act(_, act) => cell_of(act),
                Next::Go(_, task, _) => Some(*task),
                _ => None,
            };
            if let Some(p) = acting_on
                && !self.batch.contains(&p)
            {
                self.batch.push(p);
            }
            // Another bot claimed it first (bots plan in turn): ask again next tick.
            if !self.claim_batch(acting_on, matches!(next, Next::Act(..))) {
                next = Next::Hold(Phase::Building, Vec::new(), HoldWhy::ClaimLost);
            }
        }
        let (mut why, mut cell) = (None, None);
        let (phase, missing, want) = match next {
            Next::Act(phase, act) => {
                self.arrived = None;
                cell = cell_of(&act);
                (phase, Vec::new(), Want::Act(act))
            }
            Next::Go(phase, task, goal) => {
                if self.commit.as_ref().is_none_or(|(_, g)| *g != goal) {
                    self.bump(|s| s.walks += 1);
                    if let Some(j) = &self.journal {
                        j.walk_start(cx.tick, cx.pos, task, stands_of(&goal).collect());
                    }
                }
                self.commit = Some((task, goal.clone()));
                cell = Some(task);
                (phase, Vec::new(), Want::Go(goal))
            }
            Next::Hold(phase, missing, w) => {
                why = Some(w);
                (phase, missing, Want::Hold)
            }
            Next::Done(result) => {
                self.leave();
                (Phase::Done, Vec::new(), Want::Done(result))
            }
        };
        if self.journal.is_some() {
            let (claimed, taken) = self.claims();
            let kind = match &want {
                Want::Act(_) => journal::Want::Act,
                Want::Go(_) => journal::Want::Go,
                Want::Done(_) => journal::Want::Done,
                _ => journal::Want::Hold,
            };
            let to_place = needs.iter().filter(|n| **n == Need::Place).count();
            let saw = cell.and_then(|c| cx.look.block(c)).map(|v| format!("{}[{}]", v.name, v.props));
            self.note(Entry::Tick { feet: cx.pos, want: kind, phase, why, cell, saw, claimed, taken, to_place });
        }
        if phase == Phase::Waiting {
            let since = *self.waiting_since.get_or_insert(self.tick);
            if self.tick - since > ASK_AGAIN {
                self.stations.iter_mut().for_each(|s| s.lacks.clear());
                self.waiting_since = None;
            }
        } else {
            self.waiting_since = None;
        }
        self.publish(&needs, cx.look, phase, missing);
        want
    }

    fn opts(&self) -> GotoOpts {
        let mut opts = GotoOpts { scaffold: self.job.scaffold.clone(), ..GotoOpts::default() };
        // Stop near the stand's centre when the cell is beside the body: at the default 0.25 off, the body overlaps
        // it (live: a ladder cell beside its stand was never placed). Only there: arriving within 0.1 everywhere
        // cost two bots ~20% more time.
        if let Some((task, goal)) = &self.commit
            && stands_of(goal).any(|f| beside(*task, f))
        {
            opts.tolerance = STAND_TOLERANCE;
        }
        opts.search.allow_bridge = true;
        // Walks failing in a row may be walled in by built cells (live: on a wall top, every walk "Stuck"): then
        // path work may go through them, and the survey rebuilds what it breaks.
        if self.failed_walks < ESCAPE_AFTER {
            opts.search.keep = Some(self.keep.clone());
        }
        opts.search.no_body = Some(self.thin.clone());
        opts
    }

    fn arrived(&mut self) -> bool {
        self.failed_walks = 0;
        self.end_walk(WalkEnd::Arrived);
        self.arrived = self.commit.take().map(|(_, goal)| goal);
        false
    }

    fn failed(&mut self, e: &ActionError) -> bool {
        self.failed_walks += 1;
        if let Some(j) = &self.journal {
            j.walk_failed(self.tick, WalkEnd::Failed, Some(e.to_string()));
        }
        if let Some((task, goal)) = self.commit.take() {
            tracing::debug!(?task, ?goal, %e, "build drive failed");
            self.unreached(task, &goal);
        }
        false
    }

    fn acted(&mut self, act: &Act, result: &Result<(), ActionError>) {
        if result.is_ok() {
            self.failed_walks = 0;
        }
        let (kind, item) = match act {
            Act::Place { item, .. } | Act::UseOn { item, .. } => (Kind::Place, Some(item.clone())),
            Act::Break { .. } => (Kind::Break, None),
            Act::Fill { .. } => (Kind::Fill, None),
            Act::Restock { .. } => (Kind::Restock, None),
        };
        let cell = cell_of(act);
        let target = cell.and_then(|c| self.target(c)).map(|t| t.name.clone());
        self.note(Entry::Acted { kind, cell, item, target, err: result.as_ref().err().map(|e| e.to_string()) });
        match (act, result) {
            (Act::Place { against, face, .. }, Ok(())) => {
                self.tries.entry(face.adjacent(*against)).or_default().places += 1;
                self.bump(|s| s.placed += 1);
            }
            (Act::Place { against, face, .. }, Err(e)) => self.fail(face.adjacent(*against), e),
            (Act::UseOn { .. }, Ok(())) => self.bump(|s| s.placed += 1),
            (Act::UseOn { cell, .. }, Err(e)) => self.fail(*cell, e),
            (Act::Break { .. }, Ok(())) => self.bump(|s| s.broken += 1),
            (Act::Break { pos }, Err(e)) => self.fail(*pos, e),
            (Act::Fill { pos, .. }, Ok(())) => {
                if self.index.contains_key(pos) || !self.seals.contains(pos) {
                    self.add_scaffold(*pos);
                }
            }
            (Act::Fill { pos, .. }, Err(e)) => {
                self.seals.remove(pos);
                self.fail(*pos, e);
            }
            (Act::Restock { station, .. }, r) => {
                self.close_station(*station);
                if let Err(e) = r {
                    tracing::debug!(%e, "restock failed");
                }
                self.bump(|s| s.restocks += 1);
            }
        }
    }

    fn en_route(&mut self) -> Option<Act> {
        let act = self.en_route.take()?;
        if let Some(c) = cell_of(&act) {
            self.hold(c);
            self.note(Entry::EnRoute { cell: c });
        }
        Some(act)
    }

    fn observe(&mut self, event: &Event) {
        if let Event::Placed { pos } = event {
            self.add_scaffold(*pos);
        }
    }

    fn lost_control(&mut self) {
        self.end_walk(WalkEnd::Lost);
        self.commit = None;
    }
}

impl Builder {
    /// After failed walks, a thin block (pane, fence, wall, bars) in the feet or head cell: the body fits beside
    /// it but the pathfinder sees the cell as solid, so no walk starts (live and sim: the outer half of a window
    /// cell, every walk "Stuck 10 times" until the run ended).
    fn wedged(&self, cx: &Ctx) -> Option<acacia_physics::BlockPos> {
        if self.failed_walks < ESCAPE_AFTER || !cx.on_ground {
            return None;
        }
        let feet = cx.pos.map(|v| v.floor() as i32);
        [feet, [feet[0], feet[1] + 1, feet[2]]].into_iter().find(|c| cx.look.block(*c).is_some_and(|v| v.solid() && !v.full && super::rules::thin(v.name)))
    }

    fn end_walk(&self, end: WalkEnd) {
        if let Some(j) = &self.journal {
            j.walk_end(self.tick, end);
        }
    }
}
