//! Planning the next segment while a partial path is still followed, and splicing it on (Baritone's plan-ahead).

use acacia_physics::BlockPos;
use tokio::sync::oneshot::error::TryRecvError;

use super::Pilot;
use crate::movement::Sim;
use crate::pathfind::astar::SearchOpts;
use crate::pathfind::job::{Answer, SearchJob, Spawn};

/// Planned ticks left on a partial path when the next segment's search starts (Baritone's lookahead is 150).
pub(super) const PLAN_AHEAD_TICKS: f32 = 100.0;

#[derive(Default)]
pub(super) struct Ahead {
    answer: Option<Answer>,
    /// The follower generation and path length the last search was started for: one per segment end.
    started_for: Option<(u32, usize)>,
    pub joins: Vec<BlockPos>,
}

impl Ahead {
    pub fn running(&self) -> bool {
        self.answer.is_some()
    }

    pub fn clear(&mut self) {
        (self.answer, self.started_for) = (None, None);
    }
}

impl Pilot {
    /// On a partial path nearing its end, starts the next segment's search from the last node as planned.
    pub(super) fn plan_ahead<S: Sim>(&mut self, live: &S, opts: &SearchOpts, spawn: Spawn<S>) {
        let Some(f) = self.follower.as_ref().filter(|_| self.partial && self.pending.is_none()) else { return };
        let key = (self.generation, f.path().len());
        let Some(end) = f.path().last().filter(|_| self.ahead.started_for != Some(key)) else { return };
        // A running shortcut holds the follower's index back, so the planned ticks overstate what is left: the
        // straight line at the fastest speed bounds it from below.
        let pos = live.body().pos;
        let line = (pos[0] - end.pos[0] as f32 - 0.5).hypot(pos[2] - end.pos[2] as f32 - 0.5) * opts.speed(S::envelope()).per_block;
        if f.ticks_left().min(line) > PLAN_AHEAD_TICKS {
            return;
        }
        tracing::debug!(from = ?end.pos, "planning the next segment ahead");
        let from = end.mom.synth(end.pos, live);
        let job = SearchJob { from, goal: self.goal.clone(), opts: SearchOpts { favour: None, ..opts.clone() }, cache: self.cache.clone() };
        self.guard.searches += 1;
        self.ahead.started_for = Some(key);
        self.ahead.answer = Some(spawn(job));
    }

    /// Splices a finished plan-ahead on if the path it continues is still the one followed. One that found no
    /// way on is dropped: arriving at the end then re-plans from the live state.
    pub(super) fn poll_ahead(&mut self) {
        let Some(answer) = &mut self.ahead.answer else { return };
        let path = match answer.try_recv() {
            Err(TryRecvError::Empty) => return,
            done => done.ok(),
        };
        self.ahead.answer = None;
        let current = self.follower.as_mut().filter(|f| self.ahead.started_for == Some((self.generation, f.path().len())));
        match (current, path) {
            (Some(f), Some(p)) if !p.nodes.is_empty() => {
                tracing::debug!(nodes = p.nodes.len(), complete = p.complete, "next segment spliced on");
                self.ahead.joins.extend(f.path().last().map(|n| n.pos));
                f.splice(p.nodes);
                f.set_ends_at_goal(p.complete);
                self.partial = !p.complete;
            }
            (Some(_), _) => tracing::debug!("plan-ahead found no way on"),
            (None, _) => tracing::debug!("plan-ahead dropped: the path changed meanwhile"),
        }
    }
}
