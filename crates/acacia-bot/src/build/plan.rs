//! Picks the builder's next step: seal liquids, break what is wrong (top down), place what is missing (bottom up,
//! supported first), restock, visit unloaded cells, then remove scaffold. Nearest first within each rank.

use acacia_physics::{BlockPos, Vec3};

use super::builder::Builder;
use super::reach::{breaks_from, click_for, fills_from, stands, Keep, Stand};
use super::restock;
use super::rules;
use super::status::{HoldWhy, Phase};
use super::survey::Need;
use crate::interact::Face;
use crate::pathfind::process::{Act, Ctx, Look};
use crate::pathfind::Goal;
use crate::ActionError;

/// Candidates tried per tick, in rank order, before settling for a later rank.
const TRIES: usize = 24;
/// Stands handed to the pilot at once (it takes the cheapest it can reach).
pub(super) const STANDS: usize = 3;
/// Placements whose stands are searched per tick (the search tries every face from every spot). Live house16
/// A/B (tools/build-ab.sh, 3 runs each): 24 against 6 took one bot 178 s against 195, two bots 108 against 122.
pub(super) const SEARCHES: usize = 24;
const BODY: f32 = 0.35;

pub(super) enum Next {
    Act(Phase, Act),
    /// Drive to the goal for the task at the cell.
    Go(Phase, BlockPos, Goal),
    /// Stand still; the items no station has.
    Hold(Phase, Vec<String>, HoldWhy),
    Done(Result<(), ActionError>),
}

fn dist(from: Vec3, p: BlockPos) -> f32 {
    let d = [p[0] as f32 + 0.5 - from[0], p[1] as f32 + 0.5 - from[1], p[2] as f32 + 0.5 - from[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// The bot's box (at feet `pos`) overlaps the cell.
pub(super) fn in_body(pos: Vec3, p: BlockPos) -> bool {
    let overlap = |lo: f32, hi: f32, c: i32| lo < (c + 1) as f32 && hi > c as f32;
    overlap(pos[0] - BODY, pos[0] + BODY, p[0]) && overlap(pos[1], pos[1] + 1.85, p[1]) && overlap(pos[2] - BODY, pos[2] + BODY, p[2])
}

/// No bot's body is in the cell: a block can go there.
fn free(b: &Builder, cx: &Ctx, p: BlockPos) -> bool {
    !in_body(cx.pos, p) && !b.occupied(p)
}

/// Drives to the cheapest stands; the committed goal again when it is for the same task (a new goal restarts the pilot).
pub(super) fn go(phase: Phase, task: BlockPos, found: Vec<Stand>, b: &Builder) -> Option<Next> {
    if let Some((t, goal)) = &b.commit
        && *t == task
        && !found.is_empty()
    {
        return Some(Next::Go(phase, task, goal.clone()));
    }
    let goals: Vec<Goal> = found.into_iter().take(STANDS).map(|s| Goal::Block(s.feet)).collect();
    match goals.len() {
        0 => None,
        1 => Some(Next::Go(phase, task, goals.into_iter().next().expect("one"))),
        _ => Some(Next::Go(phase, task, Goal::Or(goals))),
    }
}

/// Breaks the block at `pos`, or goes where it can.
fn break_at(cx: &Ctx, pos: BlockPos, phase: Phase, b: &Builder) -> Option<Next> {
    let feet = cx.pos.map(|v| v.floor() as i32);
    let on_it = feet == [pos[0], pos[1] + 1, pos[2]];
    if breaks_from(cx.eye(), pos, cx.look) && !on_it {
        return Some(Next::Act(phase, Act::Break { pos }));
    }
    let above = [pos[0], pos[1] + 1, pos[2]];
    let ok = |e: Vec3| breaks_from(e, pos, cx.look);
    go(phase, pos, stands(pos, cx.pos, cx.look, &Keep { feet: &|f| f == above || no_stand(b, cx.look, f), pillar: &|c| unbuilt(b, cx.look, c) }, &ok, STANDS), b)
}

/// A liquid next to `pos` that breaking it would let in (from above or the sides), not held back by a target.
fn leak(b: &Builder, look: &dyn Look, pos: BlockPos) -> Option<BlockPos> {
    [Face::Up, Face::North, Face::South, Face::West, Face::East].into_iter().map(|f| f.adjacent(pos)).find(|n| {
        let target_solid = b.target(*n).is_some_and(|t| !t.is_air());
        !target_solid && look.block(*n).is_some_and(|v| v.liquid)
    })
}

/// A cell the structure fills that is still empty.
fn fills_later(b: &Builder, look: &dyn Look, c: BlockPos) -> bool {
    b.target(c).is_some_and(|t| !t.is_air()) && look.solid(c) != Some(true)
}

/// Pillars stay out of cells the structure still fills (dirt in a wall would have to be dug out again), and out
/// of regions other bots hold.
pub(super) fn unbuilt(b: &Builder, look: &dyn Look, c: BlockPos) -> bool {
    fills_later(b, look, c) || b.theirs(c)
}

/// Feet in `f` are no stand: body in a cell the structure still fills (live: the bot stood in the unbuilt top
/// of a ladder column on a narrow wall and could reach nothing from there), or shunned.
pub(super) fn no_stand(b: &Builder, look: &dyn Look, f: BlockPos) -> bool {
    b.shunned(f) || fills_later(b, look, f) || fills_later(b, look, [f[0], f[1] + 1, f[2]])
}

fn fill(b: &Builder, pos: BlockPos) -> Act {
    Act::Fill { pos, blocks: b.job.scaffold.clone() }
}

pub(super) fn next(b: &mut Builder, cx: &Ctx, needs: &[Need]) -> Next {
    let eye = cx.eye();
    b.batch.clear();
    b.en_route = None;
    // A cell with a placement in flight shows the prediction, not the server's block: leave it until decided.
    let open = |b: &Builder, i: usize, n: Need| needs[i] == n && !b.resting(b.cells[i].pos) && b.mine(b.cells[i].pos) && !cx.look.in_flight(b.cells[i].pos);
    let mut order: Vec<usize> = (0..b.cells.len()).collect();
    // The committed task first while it is still open: switching goals restarts the pilot.
    let committed = b.commit.as_ref().map(|(p, _)| *p);
    order.sort_by(|&x, &y| {
        let (p, q) = (b.cells[x].pos, b.cells[y].pos);
        (Some(q) == committed).cmp(&(Some(p) == committed)).then((dist(eye, p) + b.away_cost(p)).total_cmp(&(dist(eye, q) + b.away_cost(q))))
    });

    // Breaks and drains, top down: seal leaks first.
    // Scaffold standing where air belongs stays until the cleanup: the pilot may still be using it.
    let held = |b: &Builder, i: usize| b.scaffold.contains(&b.cells[i].pos) && b.cells[i].target.is_air();
    let mut breaks: Vec<usize> = order.iter().copied().filter(|&i| (open(b, i, Need::Break) && !held(b, i)) || open(b, i, Need::Drain)).collect();
    breaks.sort_by_key(|&i| (Some(b.cells[i].pos) != committed, -b.cells[i].pos[1]));
    for &i in breaks.iter().take(TRIES) {
        let pos = b.cells[i].pos;
        let act = match leak(b, cx.look, pos) {
            Some(n) => {
                if !b.index.contains_key(&n) {
                    b.seals.insert(n);
                }
                Some(fill(b, n))
            }
            None if needs[i] == Need::Drain => Some(fill(b, pos)),
            None => None,
        };
        let cell = match &act {
            Some(Act::Fill { pos: c, .. }) => *c,
            _ => pos,
        };
        let ok = |e: Vec3| fills_from(e, cell, cx.look);
        let next = match act {
            Some(act) if ok(eye) && free(b, cx, pos) => Some(Next::Act(Phase::Building, act)),
            Some(_) => go(Phase::Building, pos, stands(pos, cx.pos, cx.look, &Keep { feet: &|f| no_stand(b, cx.look, f), pillar: &|c| unbuilt(b, cx.look, c) }, &ok, STANDS), b),
            None => break_at(cx, pos, Phase::Building, b),
        };
        match next {
            Some(next) => return next,
            None => b.rest(pos),
        }
    }

    let adjusts: Vec<usize> = order.iter().copied().filter(|&i| open(b, i, Need::Adjust)).collect();
    if let Some(next) = super::adjust::next(b, cx, &adjusts) {
        return next;
    }

    // Places, bottom up.
    let mut places: Vec<usize> = order.iter().copied().filter(|&i| open(b, i, Need::Place)).collect();
    places.sort_by_key(|&i| b.cells[i].pos[1]);
    let mut missing: Vec<String> = Vec::new();
    let mut unsupported = None;
    let mut ready: Vec<(usize, String)> = Vec::new();
    // Never cover an open cell: a block over it takes away the click on the face below (and the stand above).
    let covers = |b: &Builder, p: BlockPos| b.index.get(&[p[0], p[1] - 1, p[2]]).is_some_and(|&j| needs[j] == Need::Place && !b.given_up(b.cells[j].pos) && rules::hangs(&b.cells[j].target).is_none());
    // A pair's second half covers too (a bed's head), unless it stands on the first (a door's upper half).
    let covers_pair = |b: &Builder, p: BlockPos| covers(b, p) || super::pairs::second(b, p).is_some_and(|o| [o[0], o[1] - 1, o[2]] != p && covers(b, o));
    let candidates: Vec<usize> = places.iter().copied().filter(|&i| !covers_pair(b, b.cells[i].pos)).take(TRIES * 2).collect();
    let held_back = super::pairs::held(b, cx.look, needs);
    for i in candidates {
        let (pos, target) = (b.cells[i].pos, &b.cells[i].target);
        let Some(item) = rules::redstone::item_now(target, cx.look.block(pos), cx.look).or_else(|| rules::item(target)) else { continue };
        if held_back.contains(&pos) {
            b.rest(pos);
            continue;
        }
        let open_at = |dy: i32| cx.look.solid([pos[0], pos[1] + dy, pos[2]]) != Some(true);
        let hangs_open = rules::hangs(target).is_some_and(|only| open_at(1) && (only || open_at(-1)));
        if missing.contains(&item) || (rules::gravity(&target.name) && open_at(-1)) || hangs_open || !rules::redstone_order::ready(target, pos, cx.look, &|p| b.target(p).cloned()) {
            continue;
        }
        let supported = Face::ALL.iter().any(|f| {
            let o = f.offset();
            cx.look.block([pos[0] - o[0], pos[1] - o[1], pos[2] - o[2]]).is_some_and(|v| v.solid() && !v.interactive && (v.full || v.name.ends_with("_slab") || rules::post(&v) || rules::redstone::top(v.name).is_some()))
        });
        if !supported {
            unsupported.get_or_insert(pos);
            continue;
        }
        if super::pairs::waits(b, cx.look, pos) {
            b.rest(pos);
            continue;
        }
        if cx.look.count(&item) == 0 {
            match restock::fetch(b, cx, needs) {
                Some(next) => return next,
                None => missing.push(item),
            }
            continue;
        }
        ready.push((i, item));
    }
    rules::redstone_order::dry_first(&mut ready, |i| b.cells[i].target.clone());
    // The held item first: every switch costs an equip (live: 235 switches, a third of a build's time).
    ready.sort_by_key(|(_, item)| Some(item.as_str()) != cx.look.held());
    // Whatever can be clicked from here first (still lowest first), then walk for the lowest. Walking, a click
    // whose state needs no particular look is placed on the way instead of stopping for it (`en_route`).
    let (mut here, walking) = (None, b.commit.is_some() && super::en_route::enabled() && super::en_route::plain_step(cx));
    for (i, item) in &ready {
        let (pos, target) = (b.cells[*i].pos, &b.cells[*i].target);
        let slack = if walking { super::en_route::SLACK } else { 0.0 };
        if free(b, cx, pos)
            && super::pairs::second(b, pos).is_none_or(|o| free(b, cx, o))
            && let Some(c) = click_for(pos, target, eye, cx.look, slack)
        {
            let act = rules::redstone::act(item.clone(), c.against, c.face, c.click, pos);
            // A bucket pours along the look, so a pour (or till) made on the move lands elsewhere (live 2026-10-28).
            if walking && super::en_route::look_free(&target.name) && matches!(act, Act::Place { .. }) {
                b.en_route.get_or_insert(act);
            } else {
                here.get_or_insert(act);
            }
            b.batch.push(pos);
        }
    }
    if let Some(act) = here {
        return Next::Act(Phase::Building, act);
    }
    // The committed task first: picking the nearest anew each tick switches goals as the bot moves, and the
    // pilot restarts on every switch.
    if let Some(at) = ready.iter().position(|(i, _)| Some(b.cells[*i].pos) == committed) {
        ready[..=at].rotate_right(1);
    }
    if let Some(next) = super::cover::walk(b, cx, &ready, committed) {
        return next;
    }
    if let Some(act) = b.en_route.take() {
        return Next::Act(Phase::Building, act);
    }
    // Nothing to click against: a scaffold column up to the lowest such cell.
    if let Some(pos) = unsupported
        && let Some(next) = support(b, cx, pos)
    {
        return next;
    }

    if let Some(i) = order.iter().copied().find(|&i| needs[i] == Need::Unknown && b.mine(b.cells[i].pos)) {
        return Next::Go(Phase::Surveying, b.cells[i].pos, Goal::Near(b.cells[i].pos, 3.0));
    }
    let pending = cx.look.any_in_flight() || needs.iter().enumerate().any(|(i, n)| matches!(n, Need::Place | Need::Break | Need::Drain | Need::Adjust) && !b.given_up(b.cells[i].pos) && !(*n == Need::Break && held(b, i)));
    if pending {
        b.revive();
        let why = match () {
            _ if !missing.is_empty() => HoldWhy::Missing,
            _ if cx.look.any_in_flight() => HoldWhy::InFlight,
            _ => HoldWhy::Blocked,
        };
        return Next::Hold(if missing.is_empty() { Phase::Building } else { Phase::Waiting }, missing, why);
    }

    // Scaffold, top down; seals outside the footprint stay.
    let standing: Vec<BlockPos> = b.scaffold.iter().copied().filter(|p| !b.seals.contains(p) && !b.given_up(*p) && cx.look.solid(*p) == Some(true)).filter(|p| b.target(*p).is_none_or(|t| t.is_air())).collect();
    let mut left: Vec<BlockPos> = standing.iter().copied().filter(|p| !b.resting(*p) && b.mine(*p)).collect();
    left.sort_by(|p, q| (Some(*q) == committed).cmp(&(Some(*p) == committed)).then(q[1].cmp(&p[1])).then(dist(eye, *p).total_cmp(&dist(eye, *q))));
    for &pos in left.iter().take(TRIES) {
        match break_at(cx, pos, Phase::Cleaning, b) {
            Some(next) => return next,
            None => b.rest(pos),
        }
    }
    if !standing.is_empty() {
        return Next::Hold(Phase::Cleaning, Vec::new(), HoldWhy::Cleaning);
    }
    if b.revive() {
        return Next::Hold(Phase::Building, Vec::new(), HoldWhy::Revive);
    }
    let failed = b.cells.iter().filter(|c| b.given_up(c.pos)).count() + b.scaffold.iter().filter(|p| b.given_up(**p) && cx.look.solid(**p) == Some(true)).count();
    Next::Done(if failed == 0 { Ok(()) } else { Err(ActionError::NotPossible(format!("{failed} cells could not be built"))) })
}

/// Fills the lowest empty cell of the column under `pos` (it stands on something), so the column grows up to it.
fn support(b: &Builder, cx: &Ctx, pos: BlockPos) -> Option<Next> {
    let mut cell = [pos[0], pos[1] - 1, pos[2]];
    while cx.look.solid([cell[0], cell[1] - 1, cell[2]]) == Some(false) {
        cell[1] -= 1;
        if pos[1] - cell[1] > 16 {
            return None;
        }
    }
    if b.target(cell).is_some_and(|t| !t.is_air()) {
        return None;
    }
    let ok = |e: Vec3| fills_from(e, cell, cx.look);
    if ok(cx.eye()) && free(b, cx, cell) {
        return Some(Next::Act(Phase::Building, fill(b, cell)));
    }
    go(Phase::Building, cell, stands(cell, cx.pos, cx.look, &Keep { feet: &|f| f == cell || no_stand(b, cx.look, f), pillar: &|c| unbuilt(b, cx.look, c) }, &ok, STANDS), b)
}
