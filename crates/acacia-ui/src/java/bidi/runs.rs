//! ICU4J's `BidiLine.getRuns` / `reorderLine` / `getVisualRun` on the paragraph object (the whole
//! text is the line; `setPara` leaves `trailingWSStart` at the end, so there is no trailing run).

use super::ubidi::{Bidi, MIXED};

/// A visual run: logical start, length, and whether its level is odd (right to left).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Run {
    pub start: usize,
    pub len: usize,
    pub odd: bool,
}

impl Bidi {
    /// `getVisualRun(0..countRuns())`.
    pub fn visual_runs(&self) -> Vec<Run> {
        let n = self.text.len();
        if n == 0 {
            return Vec::new();
        }
        if self.direction != MIXED {
            return vec![Run { start: 0, len: n, odd: self.para_level & 1 == 1 }];
        }
        let levels = &self.levels;
        let mut runs: Vec<(usize, usize)> = Vec::new();
        let (mut min, mut max) = (126i8, 0i8);
        let mut i = 0;
        while i < n {
            let start = i;
            let level = levels[i];
            min = min.min(level);
            max = max.max(level);
            while i < n && levels[i] == level {
                i += 1;
            }
            runs.push((start, i - start));
        }
        if runs.len() > 1 {
            reorder_line(&mut runs, levels, min, max);
        }
        runs.into_iter().map(|(start, len)| Run { start, len, odd: levels[start] & 1 == 1 }).collect()
    }
}

/// `reorderLine` (rule L2 on runs): every maximal sequence at or above each level from the highest
/// down to the lowest odd one is reversed.
fn reorder_line(runs: &mut [(usize, usize)], levels: &[i8], min: i8, mut max: i8) {
    if max <= (min | 1) {
        return;
    }
    let min = min + 1;
    let count = runs.len();
    let at = |runs: &[(usize, usize)], r: usize| levels[runs[r].0];
    loop {
        max -= 1;
        if max < min {
            break;
        }
        let mut first = 0;
        loop {
            while first < count && at(runs, first) < max {
                first += 1;
            }
            if first >= count {
                break;
            }
            let mut limit = first + 1;
            while limit < count && at(runs, limit) >= max {
                limit += 1;
            }
            runs[first..limit].reverse();
            if limit == count {
                break;
            }
            first = limit + 1;
        }
    }
    if min & 1 == 0 {
        runs.reverse();
    }
}
