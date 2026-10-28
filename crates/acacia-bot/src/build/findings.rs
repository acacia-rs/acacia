//! What went wrong or slow in a run, from the bots' journals (`build::journal`): where each bot's time went,
//! walk statistics, named findings, and the history of given cells. Printed by the live examples at the end of a
//! run; simulations can assert on [`findings`].

use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use acacia_physics::BlockPos;

use super::journal::{read_dir, Entry, Kind, Line, Want, WalkEnd};
use super::status::HoldWhy;

/// Consecutive hold ticks reported as a long hold.
const LONG_HOLD: usize = 100;
/// Consecutive hold ticks with cells still claimed reported as a stale claim (over the 40-tick grace).
const STALE_CLAIM: usize = 60;
/// Rests of one cell reported as a loop.
const REST_LOOP: usize = 3;

fn bots(lines: &[Line]) -> Vec<&str> {
    let mut v: Vec<&str> = lines.iter().map(|l| l.bot.as_str()).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// A bot's tick lines with each tick's wall time (to its next tick line).
fn ticks<'a>(lines: &'a [Line], bot: &str) -> Vec<(&'a Line, u64)> {
    let mine: Vec<&Line> = lines.iter().filter(|l| l.bot == bot && matches!(l.e, Entry::Tick { .. })).collect();
    mine.iter().enumerate().map(|(i, l)| (*l, mine.get(i + 1).map_or(0, |n| n.t.saturating_sub(l.t)))).collect()
}

fn pct(v: &mut [u64], p: f32) -> u64 {
    v.sort_unstable();
    v.get(((v.len() as f32 - 1.0) * p).round() as usize).copied().unwrap_or(0)
}

/// Per bot: time by what it wanted (holds by reason), and its walks.
pub fn summary(lines: &[Line]) -> Vec<String> {
    let mut out = Vec::new();
    for bot in bots(lines) {
        let mut by: BTreeMap<String, u64> = BTreeMap::new();
        for (l, ms) in ticks(lines, bot) {
            if let Entry::Tick { want, why, .. } = &l.e {
                let key = match why {
                    Some(w) => format!("hold/{w:?}"),
                    None => format!("{want:?}").to_lowercase(),
                };
                *by.entry(key).or_default() += ms;
            }
        }
        let mut v: Vec<_> = by.into_iter().collect();
        v.sort_by_key(|e| Reverse(e.1));
        let en_route = lines.iter().filter(|l| l.bot == bot && matches!(l.e, Entry::EnRoute { .. })).count();
        out.push(format!("{bot} time: {}; {en_route} placed en route", v.iter().map(|(k, ms)| format!("{k} {:.1}s", *ms as f32 / 1000.0)).collect::<Vec<_>>().join(", ")));
        let walks: Vec<(WalkEnd, u64, u64, f32, u32)> = lines
            .iter()
            .filter(|l| l.bot == bot)
            .filter_map(|l| match &l.e {
                Entry::WalkEnd { end, ticks, ms, dist, acts, .. } => Some((*end, *ticks, *ms, *dist, *acts)),
                _ => None,
            })
            .collect();
        let mut ends: BTreeMap<String, usize> = BTreeMap::new();
        walks.iter().for_each(|w| *ends.entry(format!("{:?}{}", w.0, if w.4 > 0 { "+acts" } else { "" }).to_lowercase()).or_default() += 1);
        let arrived: Vec<_> = walks.iter().filter(|w| w.0 == WalkEnd::Arrived).collect();
        let (mut ms, mut tk): (Vec<u64>, Vec<u64>) = arrived.iter().map(|w| (w.2, w.1)).unzip();
        let dist = arrived.iter().map(|w| w.3).sum::<f32>() / arrived.len().max(1) as f32;
        let total: u64 = walks.iter().map(|w| w.2).sum();
        out.push(format!(
            "{bot} walks: {} ({ends:?}), {:.1}s in all; arrived p50 {} ms / {} ticks, p90 {} ms / {} ticks, mean {dist:.1} blocks",
            walks.len(),
            total as f32 / 1000.0,
            pct(&mut ms.clone(), 0.5),
            pct(&mut tk.clone(), 0.5),
            pct(&mut ms, 0.9),
            pct(&mut tk, 0.9),
        ));
    }
    out
}

/// Cells with one confirmed placement more than their target takes (a double slab takes two) and no confirmed
/// break between, as finding lines.
fn double_places(lines: &[Line]) -> Vec<(BlockPos, String)> {
    let mut out = Vec::new();
    let mut placed: HashMap<BlockPos, Vec<(&str, u64)>> = HashMap::new();
    for l in lines {
        if let Entry::Acted { kind, cell: Some(c), err: None, item, target } = &l.e {
            match kind {
                Kind::Place => {
                    // Counted blocks take up to four (the journal names the block, not its count).
                    let takes = match target.as_deref() {
                        Some(t) if t.ends_with("double_slab") => 2,
                        Some(t) if t.ends_with("candle") || t.ends_with("sea_pickle") || t.ends_with("turtle_egg") => 4,
                        _ => 1,
                    };
                    let before = placed.entry(*c).or_default();
                    before.push((l.bot.as_str(), l.t));
                    if before.len() > takes {
                        let (by, t) = before[before.len() - 2];
                        out.push((*c, format!("DOUBLE_PLACE {c:?} {item:?} (target {target:?}): {by} at {t} ms, then {} at {} ms", l.bot, l.t)));
                    }
                }
                Kind::Break => {
                    placed.remove(c);
                }
                _ => {}
            }
        }
    }
    out
}

/// Named findings: each a line starting with its name.
pub fn findings(lines: &[Line]) -> Vec<String> {
    let mut out: Vec<String> = double_places(lines).into_iter().map(|(_, l)| l).collect();
    for bot in bots(lines) {
        // LONG_HOLD and STALE_CLAIM: runs of hold ticks.
        let ts = ticks(lines, bot);
        let mut i = 0;
        while i < ts.len() {
            let hold = |k: usize| matches!(ts[k].0.e, Entry::Tick { want: Want::Hold, .. });
            if !hold(i) {
                i += 1;
                continue;
            }
            let start = i;
            while i < ts.len() && hold(i) {
                i += 1;
            }
            let run = &ts[start..i];
            let mut whys: BTreeMap<String, usize> = BTreeMap::new();
            let mut held = 0;
            let mut stale = 0usize;
            let mut taken = 0;
            for (l, _) in run {
                if let Entry::Tick { why, claimed, taken: t, .. } = &l.e {
                    *whys.entry(format!("{:?}", why.unwrap_or(HoldWhy::Blocked))).or_default() += 1;
                    stale = if *claimed > 0 { stale + 1 } else { 0 };
                    held = held.max(stale);
                    taken = taken.max(*t);
                }
            }
            let (t0, t1) = (run[0].0.t, run[run.len() - 1].0.t);
            if run.len() >= LONG_HOLD {
                out.push(format!("LONG_HOLD {bot} {} ticks {t0}-{t1} ms, why {whys:?}, others held up to {taken} cells", run.len()));
            }
            if held >= STALE_CLAIM {
                out.push(format!("STALE_CLAIM {bot} kept cells claimed through {held} hold ticks ({t0}-{t1} ms)"));
            }
        }
        // REST_LOOP, WALK_NO_ACT and GOAL_THRASH: per cell counts.
        let mut rests: HashMap<BlockPos, usize> = HashMap::new();
        let mut switched: HashMap<BlockPos, usize> = HashMap::new();
        let mut no_click = 0;
        // WALK_STUCK: failed walks in a row that never left the spot.
        let (mut stuck, mut worst, mut why) = (0, 0, None);
        for l in lines.iter().filter(|l| l.bot == bot) {
            if let Entry::WalkEnd { end, dist, err, .. } = &l.e {
                stuck = if *end == WalkEnd::Failed && *dist < 0.5 { stuck + 1 } else { 0 };
                if stuck > worst {
                    (worst, why) = (stuck, err.clone());
                }
            }
        }
        if worst >= 3 {
            out.push(format!("WALK_STUCK {bot} {worst} failed walks in a row without moving, last {why:?}"));
        }
        for l in lines.iter().filter(|l| l.bot == bot) {
            match &l.e {
                Entry::Rest { cell } => *rests.entry(*cell).or_default() += 1,
                Entry::WalkEnd { task, end: WalkEnd::Switched, acts: 0, .. } => *switched.entry(*task).or_default() += 1,
                Entry::Tick { why: Some(HoldWhy::ArrivedNoClick), .. } => no_click += 1,
                _ => {}
            }
        }
        let mut loops: Vec<_> = rests.into_iter().filter(|(_, n)| *n >= REST_LOOP).collect();
        loops.sort_by_key(|e| Reverse(e.1));
        if !loops.is_empty() {
            out.push(format!("REST_LOOP {bot} {} cells rested {REST_LOOP}+ times, most {:?}", loops.len(), &loops[..loops.len().min(5)]));
        }
        if no_click > 0 {
            out.push(format!("WALK_NO_ACT {bot} reached a stand and could not click {no_click} times"));
        }
        let thrash: usize = switched.values().sum();
        if thrash > 0 {
            let mut top: Vec<_> = switched.into_iter().collect();
            top.sort_by_key(|e| Reverse(e.1));
            out.push(format!("GOAL_THRASH {bot} {thrash} walks replaced before arriving with nothing done, most {:?}", &top[..top.len().min(5)]));
        }
    }
    out
}

/// Everything the journals say about `cell`, in time order.
pub fn biography(lines: &[Line], cell: BlockPos) -> Vec<String> {
    lines
        .iter()
        .filter(|l| match &l.e {
            Entry::Acted { cell: c, .. } | Entry::Tick { cell: c, want: Want::Act, .. } => *c == Some(cell),
            Entry::Rest { cell: c } | Entry::EnRoute { cell: c } => *c == cell,
            Entry::WalkStart { task, .. } | Entry::WalkEnd { task, .. } => *task == cell,
            _ => false,
        })
        .map(|l| format!("{} ms {} {:?}", l.t, l.bot, l.e))
        .collect()
}

/// The summary, the findings, and the history of each cell in `wrong` and each cell placed twice, from the
/// journals in `dir`.
pub fn report(dir: &Path, wrong: &[BlockPos]) -> std::io::Result<Vec<String>> {
    let lines = read_dir(dir)?;
    let mut out = summary(&lines);
    let found = findings(&lines);
    out.push(format!("findings: {}", found.len()));
    out.extend(found);
    let mut cells = wrong.to_vec();
    cells.extend(double_places(&lines).into_iter().map(|(c, _)| c).filter(|c| !wrong.contains(c)).take(5));
    for c in &cells {
        out.push(format!("cell {c:?}:"));
        out.extend(biography(&lines, *c).into_iter().map(|l| format!("  {l}")));
    }
    Ok(out)
}
