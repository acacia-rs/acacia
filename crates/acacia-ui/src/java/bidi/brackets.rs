//! ICU4J's bracket pairing (rule N0) as `resolveExplicitLevels` runs it: `bracketInit`,
//! `bracketProcess*`, `bracketAddOpening`, `fixN0c`.

use super::props::*;
use super::ubidi::Bidi;

const FOUND_L: u8 = 1;
const FOUND_R: u8 = 2;

/// `Bidi.IsoRun`: one isolating run sequence's openings (`start..limit`) and context.
#[derive(Clone, Copy, Default)]
struct IsoRun {
    context_pos: usize,
    start: usize,
    limit: usize,
    level: i8,
    last_strong: u8,
    last_base: u8,
    context_dir: u8,
}

impl IsoRun {
    fn reset(&mut self, level: i8, context: i8) {
        self.level = level;
        let dir = lr(context);
        (self.last_strong, self.last_base, self.context_dir) = (dir, dir, dir);
    }
}

/// `Bidi.Opening`; `matched` is the closing bracket's char, or minus the closing's position once
/// paired but not yet stable, or 0.
#[derive(Clone, Copy, Default)]
struct Opening {
    position: usize,
    matched: i32,
    context_pos: usize,
    flags: u8,
    context_dir: u8,
}

pub(super) struct BracketData {
    openings: Vec<Opening>,
    iso_runs: Vec<IsoRun>,
    last: usize,
}

impl BracketData {
    /// `bracketInit`.
    pub fn new(level: i8) -> BracketData {
        let mut run = IsoRun::default();
        run.reset(level, level);
        BracketData { openings: Vec::new(), iso_runs: vec![run], last: 0 }
    }

    /// `bracketProcessB`.
    pub fn process_b(&mut self, level: i8) {
        self.last = 0;
        let run = &mut self.iso_runs[0];
        run.limit = 0;
        run.reset(level, level);
        run.context_pos = 0;
    }

    /// `bracketProcessLRI_RLI`.
    pub fn process_lri_rli(&mut self, level: i8) {
        self.iso_runs[self.last].last_base = ON;
        let last_limit = self.iso_runs[self.last].limit;
        self.last += 1;
        if self.last == self.iso_runs.len() {
            self.iso_runs.push(IsoRun::default());
        }
        let run = &mut self.iso_runs[self.last];
        (run.start, run.limit) = (last_limit, last_limit);
        run.reset(level, level);
        run.context_pos = 0;
    }

    /// `bracketProcessPDI`.
    pub fn process_pdi(&mut self) {
        self.last -= 1;
        self.iso_runs[self.last].last_base = ON;
    }

    fn add_opening(&mut self, matched: u16, position: usize) {
        let run = self.iso_runs[self.last];
        let opening = Opening { position, matched: i32::from(matched), context_pos: run.context_pos, flags: 0, context_dir: run.context_dir };
        if run.limit == self.openings.len() {
            self.openings.push(opening);
        } else {
            self.openings[run.limit] = opening;
        }
        self.iso_runs[self.last].limit += 1;
    }
}

impl Bidi {
    /// `bracketProcessBoundary`.
    pub(super) fn bracket_process_boundary(&self, bd: &mut BracketData, last_cc_pos: usize, context: i8, embedding: i8) {
        if flag(self.dir_props[last_cc_pos]) & MASK_ISO != 0 {
            return;
        }
        let context = if no_override(embedding) > no_override(context) { embedding } else { context };
        let run = &mut bd.iso_runs[bd.last];
        run.limit = run.start;
        run.reset(embedding, context);
        run.context_pos = last_cc_pos;
    }

    fn fix_n0c(&mut self, bd: &mut BracketData, opening_index: usize, new_prop_position: usize, new_prop: u8) {
        let limit = bd.iso_runs[bd.last].limit;
        for k in opening_index + 1..limit {
            let q = bd.openings[k];
            if q.matched >= 0 {
                continue;
            }
            if new_prop_position < q.context_pos {
                break;
            }
            if new_prop_position < q.position {
                if new_prop == q.context_dir {
                    break;
                }
                let closing = (-q.matched) as usize;
                self.dir_props[q.position] = new_prop;
                self.dir_props[closing] = new_prop;
                bd.openings[k].matched = 0;
                self.fix_n0c(bd, k, q.position, new_prop);
                self.fix_n0c(bd, k, closing, new_prop);
            }
        }
    }

    /// `bracketProcessClosing`: the pair's resolved class, or ON when nothing strong is inside.
    fn bracket_process_closing(&mut self, bd: &mut BracketData, open_idx: usize, position: usize) -> u8 {
        let run = bd.iso_runs[bd.last];
        let o = bd.openings[open_idx];
        let direction = lr(run.level);
        let mut stable = true;
        let new_prop = if (direction == L && o.flags & FOUND_L != 0) || (direction == R && o.flags & FOUND_R != 0) {
            direction
        } else if o.flags & (FOUND_L | FOUND_R) != 0 {
            stable = open_idx == run.start;
            if direction != o.context_dir { o.context_dir } else { direction }
        } else {
            bd.iso_runs[bd.last].limit = open_idx;
            return ON;
        };
        self.dir_props[o.position] = new_prop;
        self.dir_props[position] = new_prop;
        self.fix_n0c(bd, open_idx, o.position, new_prop);
        if stable {
            let mut limit = open_idx;
            while limit > run.start && bd.openings[limit - 1].position == o.position {
                limit -= 1;
            }
            bd.iso_runs[bd.last].limit = limit;
        } else {
            bd.openings[open_idx].matched = -(position as i32);
            let mut k = open_idx;
            while k > run.start && bd.openings[k - 1].position == o.position {
                k -= 1;
                bd.openings[k].matched = 0;
            }
            for q in &mut bd.openings[open_idx + 1..run.limit] {
                if q.position >= position {
                    break;
                }
                if q.matched > 0 {
                    q.matched = 0;
                }
            }
        }
        new_prop
    }

    /// `bracketProcessChar`.
    pub(super) fn bracket_process_char(&mut self, bd: &mut BracketData, position: usize) {
        let dir_prop = self.dir_props[position];
        if dir_prop == ON {
            let c = self.text[position];
            let run = bd.iso_runs[bd.last];
            let mut opener = true;
            if let Some(idx) = (run.start..run.limit).rev().find(|&i| bd.openings[i].matched == i32::from(c)) {
                let new_prop = self.bracket_process_closing(bd, idx, position);
                if new_prop != ON {
                    let run = &mut bd.iso_runs[bd.last];
                    run.last_base = ON;
                    run.context_dir = new_prop;
                    run.context_pos = position;
                    let level = self.levels[position];
                    if level < 0 {
                        let strong = lr(level);
                        run.last_strong = strong;
                        let start = run.start;
                        for o in &mut bd.openings[start..idx] {
                            o.flags |= flag(strong) as u8;
                        }
                        self.levels[position] &= 127;
                    }
                    self.levels[bd.openings[idx].position] &= 127;
                    return;
                }
                opener = false;
            }
            if let Some(m) = closing_bracket(c).filter(|_| opener) {
                // Canonically equivalent angle brackets close each other's openings.
                if m == 0x232A {
                    bd.add_opening(0x3009, position);
                } else if m == 0x3009 {
                    bd.add_opening(0x232A, position);
                }
                bd.add_opening(m, position);
            }
        }
        let level = self.levels[position];
        let run = &mut bd.iso_runs[bd.last];
        let new_prop = if level < 0 {
            let p = lr(level);
            if !matches!(dir_prop, S | WS | ON) {
                self.dir_props[position] = p;
            }
            (run.last_base, run.last_strong, run.context_dir, run.context_pos) = (p, p, p, position);
            p
        } else if dir_prop <= R || dir_prop == AL {
            let p = if dir_prop == L { L } else { R };
            (run.last_base, run.last_strong, run.context_dir, run.context_pos) = (dir_prop, dir_prop, p, position);
            p
        } else if dir_prop == EN {
            run.last_base = EN;
            run.context_pos = position;
            if run.last_strong == L {
                self.dir_props[position] = ENL;
                run.context_dir = L;
                L
            } else {
                self.dir_props[position] = if run.last_strong == AL { AN } else { ENR };
                run.context_dir = R;
                R
            }
        } else if dir_prop == AN {
            (run.last_base, run.context_dir, run.context_pos) = (AN, R, position);
            R
        } else if dir_prop == NSM {
            let p = run.last_base;
            if p == ON {
                self.dir_props[position] = p;
            }
            p
        } else {
            run.last_base = dir_prop;
            dir_prop
        };
        if new_prop <= R || new_prop == AL {
            let f = flag(if new_prop == L { L } else { R }) as u8;
            let (start, limit) = (run.start, run.limit);
            for o in &mut bd.openings[start..limit] {
                if position > o.position {
                    o.flags |= f;
                }
            }
        }
    }
}
