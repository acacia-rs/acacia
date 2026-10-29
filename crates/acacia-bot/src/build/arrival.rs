//! A stand reached but the click not made from there. The pilot stops anywhere in a stand's cell when the walk
//! starts inside it (an empty path) and up to its tolerance off centre otherwise, while stands are judged from their
//! centre (live: 0.43 off, a plank's one Up-face point hidden, a comparator facing east for north). So: drive to that
//! stand's centre (`GotoOpts::settle`), then try the task's other stands, then rest the task.

use acacia_physics::{BlockPos, Vec3};

use super::builder::Builder;
use super::plan::Next;
use super::status::{HoldWhy, Phase};
use crate::pathfind::Goal;
use crate::pathfind::process::Ctx;

/// Within this of a stand's centre (horizontally) counts as on it: under 0.5 minus the body margin (`plan::BODY`).
pub(super) const STAND_TOLERANCE: f32 = 0.1;

pub(super) fn stands_of(goal: &Goal) -> Box<dyn Iterator<Item = BlockPos> + '_> {
    match goal {
        Goal::Block(p) => Box::new(std::iter::once(*p)),
        Goal::Or(gs) => Box::new(gs.iter().flat_map(stands_of)),
        _ => Box::new(std::iter::empty()),
    }
}

fn any_of(mut stands: Vec<BlockPos>) -> Option<Goal> {
    match stands.len() {
        0 => None,
        1 => stands.pop().map(Goal::Block),
        _ => Some(Goal::Or(stands.into_iter().map(Goal::Block).collect())),
    }
}

fn off_centre(pos: Vec3, s: BlockPos) -> f32 {
    (pos[0] - s[0] as f32 - 0.5).hypot(pos[2] - s[2] as f32 - 0.5)
}

pub(super) enum Missed {
    /// Not a stand reached and asked for again.
    No,
    /// Drive there instead.
    Go(Next),
    /// Every stand tried: the task rests; plan again.
    Rested,
}

/// `next` asks to walk to stands the bot just reached (all of them among `next`'s): the click failed from here.
pub(super) fn missed(b: &mut Builder, cx: &Ctx, next: &Next) -> Missed {
    let Next::Go(phase, task, goal) = next else { return Missed::No };
    let reached = b.arrived.as_ref().is_some_and(|a| stands_of(a).next().is_some() && stands_of(a).all(|s| stands_of(goal).any(|g| g == s)));
    if !reached {
        return Missed::No;
    }
    let before = b.arrived.take();
    // The reached stand is the nearest, not the feet's cell: the pilot may count an edge or a block up as on it, and
    // a stand never marked tried was rewalked every tick (sim 6000 ticks; live two bots, feet a block up, 3800 walks).
    let away = |s: BlockPos| off_centre(cx.pos, s).hypot(cx.pos[1] - s[1] as f32);
    let feet = stands_of(goal).min_by(|a, b| away(*a).total_cmp(&away(*b))).unwrap_or(cx.pos.map(|v| v.floor() as i32));
    if let Some(s) = stands_of(goal).find(|s| *s == feet) {
        let first = !b.tried.contains(&(*task, s));
        if first {
            b.tried.push((*task, s));
        }
        if first && off_centre(cx.pos, s) > STAND_TOLERANCE {
            b.settling = Some(s);
            // Wrapped: the arbiter holds still for a goal equal to the route it just arrived on.
            return Missed::Go(Next::Go(*phase, *task, Goal::Or(vec![Goal::Block(s)])));
        }
    }
    let others: Vec<BlockPos> = stands_of(goal).filter(|s| *s != feet && !b.tried.contains(&(*task, *s))).collect();
    if let Some(g) = any_of(others) {
        let g = if before.as_ref() == Some(&g) { Goal::Or(vec![g]) } else { g };
        return Missed::Go(Next::Go(*phase, *task, g));
    }
    tracing::debug!(?task, ?goal, "build stands reached but no click from any");
    b.tried.retain(|(t, _)| t != task);
    b.rest(*task);
    b.batch.clear();
    Missed::Rested
}

/// The hold while a rested task makes way for the next plan.
pub(super) fn hold() -> Next {
    Next::Hold(Phase::Building, Vec::new(), HoldWhy::ArrivedNoClick)
}
