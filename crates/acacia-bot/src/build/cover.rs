//! Where to walk next: the stand that reaches the most ready placements for the walk, not the stand of the first
//! one. Walking was half of a build (act timing, house16 one bot: moving ~110 of ~210 s), and a stand that reaches
//! several cells pays for its walk once. Once there, `plan` places everything clickable, held item first.

use acacia_physics::BlockPos;

use super::builder::Builder;
use super::plan::{go, no_stand, unbuilt, Next, SEARCHES, STANDS};
use super::reach::{click_for, stands, Keep, Stand, STAND_SLACK};
use super::sight_stats;
use super::status::Phase;
use crate::pathfind::process::Ctx;
use crate::pathfind::Goal;

/// Blocks of travel a stand may cost per extra cell it reaches before a nearer one wins. At 4, with pillars at 4 a
/// block, live house16 one bot walked 26 s more and cleaned up 21 s more scaffold (54 blocks broken against
/// 25-36): high pillar stands won on reach.
const WALK: f32 = 2.0;
/// A pillar block counts as this much travel: it is climbed, and broken again at the cleanup.
const PILLAR: f32 = 12.0;

/// Ready placements whose stands are scored per tick: [`SEARCHES`], or `BUILD_SEARCHES` for an A/B.
fn searches() -> usize {
    static N: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *N.get_or_init(|| std::env::var("BUILD_SEARCHES").ok().and_then(|v| v.parse().ok()).unwrap_or(SEARCHES))
}

/// `ready`: placements that can be done once in reach, best rank first; `committed`: the task the pilot drives
/// to, kept while it is still ready (a new goal restarts the pilot).
pub(super) fn walk(b: &mut Builder, cx: &Ctx, ready: &[(usize, String)], committed: Option<BlockPos>) -> Option<Next> {
    let mut candidates: Vec<(BlockPos, Stand)> = Vec::new();
    for (k, (i, _)) in ready.iter().take(searches()).enumerate() {
        let (pos, target) = (b.cells[*i].pos, b.cells[*i].target.clone());
        let keep = Keep { feet: &|f| no_stand(b, cx.look, f), pillar: &|c| unbuilt(b, cx.look, c) };
        let found = stands(pos, cx.pos, cx.look, &keep, &|e| sight_stats::at("stand search", || click_for(pos, &target, e, cx.look, STAND_SLACK).is_some()), STANDS);
        if found.is_empty() {
            b.rest(pos);
            continue;
        }
        if k == 0 && Some(pos) == committed {
            return go(Phase::Building, pos, found, b);
        }
        candidates.extend(found.into_iter().map(|s| (pos, s)));
    }
    let reached = |s: &Stand| -> Vec<BlockPos> {
        let eye = s.eye();
        ready.iter().map(|(j, _)| &b.cells[*j]).filter(|c| sight_stats::at("stand reach", || click_for(c.pos, &c.target, eye, cx.look, STAND_SLACK).is_some())).map(|c| c.pos).collect()
    };
    let travel = |s: &Stand| {
        let d = [s.feet[0] as f32 + 0.5 - cx.pos[0], s.feet[1] as f32 - cx.pos[1], s.feet[2] as f32 + 0.5 - cx.pos[2]];
        d[0].hypot(d[2]) + d[1].abs() * 2.0 + s.pillar as f32 * PILLAR + b.away_cost(s.feet)
    };
    let mut scored: Vec<(f32, f32, BlockPos, BlockPos, Vec<BlockPos>)> = candidates
        .iter()
        .map(|(task, s)| {
            let cells = reached(s);
            (cells.len() as f32 / (1.0 + travel(s) / WALK), travel(s), *task, s.feet, cells)
        })
        .collect();
    scored.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.total_cmp(&y.1)));
    scored.dedup_by_key(|x| x.3);
    // The best few, not the best one: the pilot takes the cheapest it can reach (live sim: a lone stand it could
    // not reach cost 601 ticks a try, again after each avoid ran out).
    let task = scored.first()?.2;
    b.batch = scored[0].4.clone();
    let goals: Vec<Goal> = scored.iter().take(STANDS).map(|x| Goal::Block(x.3)).collect();
    Some(Next::Go(Phase::Building, task, if goals.len() == 1 { goals[0].clone() } else { Goal::Or(goals) }))
}
