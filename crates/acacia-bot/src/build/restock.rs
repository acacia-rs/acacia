//! Restocking by remaining need: everything the open placements still need (sharing a job: this bot's share,
//! the need over the bots on the board), minus what the inventory holds, from the nearest station not yet found
//! without it. A share, not the need of the cells at hand: with 4×4 tiles (before cell claims) each bot walked
//! back after nearly every tile.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use super::bill::items;
use super::builder::{Builder, Restock};
use super::plan::Next;
use super::reach::in_reach;
use super::status::{HoldWhy, Phase};
use super::survey::Need;
use crate::pathfind::process::{Act, Ctx, Look};
use crate::pathfind::Goal;

/// Items still to place, minus the inventory's, in plan order: those the inventory has none of, then by the
/// lowest cell needing them (places go bottom up). The act takes them in this order while there is room, so a
/// need past the inventory (mansion slice: 60 stacks, 36 slots) brings what is placed next.
pub(super) fn shortfall(b: &Builder, cx: &Ctx, needs: &[Need]) -> Vec<(String, u32)> {
    let mut want: BTreeMap<String, (u32, i32)> = BTreeMap::new();
    for (c, n) in b.cells.iter().zip(needs) {
        if *n == Need::Place && !b.given_up(c.pos) {
            for (item, k) in items(&c.target) {
                let e = want.entry(item).or_insert((0, i32::MAX));
                (e.0, e.1) = (e.0 + k, e.1.min(c.pos[1]));
            }
        }
    }
    let share = b.sharers();
    let mut out: Vec<(String, u32, i32)> =
        want.into_iter().filter_map(|(item, (n, low))| n.div_ceil(share).checked_sub(cx.look.count(&item)).filter(|d| *d > 0).map(|d| (item, d, low))).collect();
    out.sort_by_key(|(item, n, low)| (cx.look.count(item) > 0, *low, Reverse(*n)));
    out.into_iter().map(|(item, n, _)| (item, n)).collect()
}

/// Restocks for the items the inventory has none of, or `None` when no station may have them or nothing would
/// fit (the cells holding items still go in, freeing slots).
pub(super) fn fetch(b: &mut Builder, cx: &Ctx, needs: &[Need]) -> Option<Next> {
    if cx.look.free_slots() == 0 {
        return None;
    }
    let want = shortfall(b, cx, needs);
    let empty: Vec<&String> = want.iter().filter(|(item, _)| cx.look.count(item) == 0).map(|(item, _)| item).collect();
    let eye = cx.eye();
    let d = |p: [i32; 3]| {
        let v = [p[0] as f32 + 0.5 - eye[0], p[1] as f32 + 0.5 - eye[1], p[2] as f32 + 0.5 - eye[2]];
        v[0] * v[0] + v[1] * v[1] + v[2] * v[2]
    };
    let station = b
        .stations
        .iter()
        .enumerate()
        .filter(|(_, s)| empty.iter().any(|item| !s.lacks.contains(*item)))
        .min_by(|(_, x), (_, y)| d(x.pos).total_cmp(&d(y.pos)))
        .map(|(i, s)| (i, s.pos))?;
    let (i, pos) = station;
    if !in_reach(eye, pos) {
        return Some(Next::Go(Phase::Restocking, pos, Goal::Near(pos, 2.5)));
    }
    // One bot at a time in a container: two taking for the same need at once would both see the full stock.
    if !b.open_station(pos) {
        return Some(Next::Hold(Phase::Restocking, Vec::new(), HoldWhy::Station));
    }
    let before = want.iter().map(|(item, _)| (item.clone(), cx.look.count(item))).collect();
    b.restock = Some(Restock { station: i, before });
    Some(Next::Act(Phase::Restocking, Act::Restock { station: pos, items: want }))
}

impl Builder {
    /// Learns what a station lacks from what the last restock brought. A full inventory took nothing more, so
    /// what did not come is not lacking (live: the mansion slice waited 204 s on items its barrels held).
    pub(super) fn check_restock(&mut self, look: &dyn Look) {
        let Some(r) = self.restock.take() else { return };
        let free = look.free_slots();
        let got: Vec<(String, u32)> = r.before.iter().map(|(item, before)| (item.clone(), look.count(item).saturating_sub(*before))).collect();
        for (item, n) in &got {
            if *n == 0 && free > 0 {
                self.stations[r.station].lacks.insert(item.clone());
            }
        }
        self.note(super::journal::Entry::Restocked { station: self.stations[r.station].pos, got, free_slots: free });
    }
}
