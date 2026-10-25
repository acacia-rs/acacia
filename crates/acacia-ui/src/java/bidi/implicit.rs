//! ICU4J's `resolveImplicitLevels` and `processPropertySeq` with `impTab_DEFAULT` (rules W1-W7,
//! N1-N2, I1-I2 as state tables), isolates resumed at their PDI.

use super::props::*;
use super::ubidi::{Bidi, Isolate};

/// `groupProp`: dir prop to `impTabProps` column.
const GROUP_PROP: [u8; 25] = [0, 1, 2, 7, 8, 3, 9, 6, 5, 4, 4, 10, 10, 12, 10, 10, 10, 11, 10, 4, 4, 4, 4, 13, 14];
const PROP_ON: u8 = 4;

/// `impTabProps`: low 5 bits the next state, high 3 the action; column 15 the state's resolved prop.
const IMP_TAB_PROPS: [[u8; 16]; 24] = [
    [1, 2, 4, 5, 7, 15, 17, 7, 9, 7, 0, 7, 3, 18, 21, 4],
    [1, 34, 36, 37, 39, 47, 49, 39, 41, 39, 1, 1, 35, 50, 53, 0],
    [33, 2, 36, 37, 39, 47, 49, 39, 41, 39, 2, 2, 35, 50, 53, 1],
    [33, 34, 38, 38, 40, 48, 49, 40, 40, 40, 3, 3, 3, 50, 53, 1],
    [33, 34, 4, 37, 39, 47, 49, 74, 11, 74, 4, 4, 35, 18, 21, 2],
    [33, 34, 36, 5, 39, 47, 49, 39, 41, 76, 5, 5, 35, 50, 53, 3],
    [33, 34, 6, 6, 40, 48, 49, 40, 40, 77, 6, 6, 35, 18, 21, 3],
    [33, 34, 36, 37, 7, 47, 49, 7, 78, 7, 7, 7, 35, 50, 53, 4],
    [33, 34, 38, 38, 8, 48, 49, 8, 8, 8, 8, 8, 35, 50, 53, 4],
    [33, 34, 4, 37, 7, 47, 49, 7, 9, 7, 9, 9, 35, 18, 21, 4],
    [97, 98, 4, 101, 135, 111, 113, 135, 142, 135, 10, 135, 99, 18, 21, 2],
    [33, 34, 4, 37, 39, 47, 49, 39, 11, 39, 11, 11, 35, 18, 21, 2],
    [97, 98, 100, 5, 135, 111, 113, 135, 142, 135, 12, 135, 99, 114, 117, 3],
    [97, 98, 6, 6, 136, 112, 113, 136, 136, 136, 13, 136, 99, 18, 21, 3],
    [33, 34, 132, 37, 7, 47, 49, 7, 14, 7, 14, 14, 35, 146, 149, 4],
    [33, 34, 36, 37, 39, 15, 49, 39, 41, 39, 15, 39, 35, 50, 53, 5],
    [33, 34, 38, 38, 40, 16, 49, 40, 40, 40, 16, 40, 35, 50, 53, 5],
    [33, 34, 36, 37, 39, 47, 17, 39, 41, 39, 17, 39, 35, 50, 53, 6],
    [33, 34, 18, 37, 39, 47, 49, 83, 20, 83, 18, 18, 35, 18, 21, 0],
    [97, 98, 18, 101, 135, 111, 113, 135, 142, 135, 19, 135, 99, 18, 21, 0],
    [33, 34, 18, 37, 39, 47, 49, 39, 20, 39, 20, 20, 35, 18, 21, 0],
    [33, 34, 21, 37, 39, 47, 49, 86, 23, 86, 21, 21, 35, 18, 21, 3],
    [97, 98, 21, 101, 135, 111, 113, 135, 142, 135, 22, 135, 99, 18, 21, 3],
    [33, 34, 21, 37, 39, 47, 49, 39, 23, 39, 23, 23, 35, 18, 21, 3],
];

type ImpTab = [[u8; 8]; 6];

/// `impTabL_DEFAULT` / `impTabR_DEFAULT`: low 4 bits the next state, high 4 the action; column 7
/// the level the state adds.
const IMP_TAB_L: ImpTab = [
    [0, 1, 0, 2, 0, 0, 0, 0],
    [0, 1, 3, 3, 20, 20, 0, 1],
    [0, 1, 0, 2, 21, 21, 0, 2],
    [0, 1, 3, 3, 20, 20, 0, 2],
    [0, 33, 51, 51, 4, 4, 0, 0],
    [0, 33, 0, 50, 5, 5, 0, 0],
];
const IMP_TAB_R: ImpTab = [
    [1, 0, 2, 2, 0, 0, 0, 0],
    [1, 0, 1, 3, 20, 20, 0, 1],
    [1, 0, 2, 2, 0, 0, 0, 1],
    [1, 0, 1, 3, 5, 5, 0, 1],
    [33, 0, 33, 3, 4, 4, 0, 0],
    [1, 0, 1, 3, 5, 5, 0, 0],
];

/// `Bidi.LevState`.
struct LevState {
    imp_tab: &'static ImpTab,
    start_on: isize,
    run_start: usize,
    state: u8,
    run_level: i8,
}

impl Bidi {
    fn set_levels_outside_isolates(&mut self, start: isize, limit: usize, level: i8) {
        let mut depth = 0;
        for k in start.max(0) as usize..limit {
            let p = self.dir_props[k];
            if p == PDI {
                depth -= 1;
            }
            if depth == 0 {
                self.levels[k] = level;
            }
            if p == LRI || p == RLI {
                depth += 1;
            }
        }
    }

    fn process_property_seq(&mut self, ls: &mut LevState, prop: u8, start: usize, limit: usize) {
        let start0 = start as isize;
        let mut start = start as isize;
        let cell = ls.imp_tab[usize::from(ls.state)][usize::from(prop)];
        ls.state = cell & 15;
        let add_level = ls.imp_tab[usize::from(ls.state)][7] as i8;
        match cell >> 4 {
            1 => ls.start_on = start0,
            2 => start = ls.start_on,
            3 => self.set_levels_outside_isolates(ls.start_on, start0 as usize, ls.run_level + 1),
            4 => self.set_levels_outside_isolates(ls.start_on, start0 as usize, ls.run_level + 2),
            _ => {}
        }
        if add_level != 0 || start < start0 {
            let level = ls.run_level + add_level;
            if start >= ls.run_start as isize {
                self.levels[start as usize..limit].fill(level);
            } else {
                self.set_levels_outside_isolates(start, limit, level);
            }
        }
    }

    /// The last character of `start..limit` that is not BN or an explicit embedding control.
    fn last_kept(&self, start: usize, limit: usize) -> u8 {
        let mut k = limit - 1;
        while k > start && flag(self.dir_props[k]) & MASK_BN_EXPLICIT != 0 {
            k -= 1;
        }
        self.dir_props[k]
    }

    pub(super) fn resolve_implicit_levels(&mut self, start: usize, limit: usize, sor: u8, eor: u8) {
        let run_level = self.levels[start];
        let imp_tab = if run_level & 1 == 0 { &IMP_TAB_L } else { &IMP_TAB_R };
        let mut ls = LevState { imp_tab, start_on: -1, run_start: start, state: 0, run_level };
        let mut start1;
        let mut state_imp: u8;
        if self.dir_props[start] == PDI {
            let saved = self.isolates[self.isolate_count as usize];
            (ls.start_on, start1, state_imp, ls.state) = (saved.start_on, saved.start1, saved.state_imp, saved.state);
            self.isolate_count -= 1;
        } else {
            start1 = start;
            state_imp = if self.dir_props[start] == NSM { 1 + sor } else { 0 };
            self.process_property_seq(&mut ls, sor, start, start);
        }
        let mut start2 = start;
        for i in start..=limit {
            let gprop = if i >= limit {
                if matches!(self.last_kept(start, limit), LRI | RLI) {
                    break;
                }
                eor
            } else {
                let prop = self.dir_props[i];
                if prop == B {
                    self.isolate_count = -1;
                }
                GROUP_PROP[usize::from(prop)]
            };
            let old_state = usize::from(state_imp);
            let cell = IMP_TAB_PROPS[old_state][usize::from(gprop)];
            state_imp = cell & 31;
            let mut action = cell >> 5;
            if i == limit && action == 0 {
                action = 1;
            }
            if action == 0 {
                continue;
            }
            let res_prop = IMP_TAB_PROPS[old_state][15];
            match action {
                1 => {
                    self.process_property_seq(&mut ls, res_prop, start1, i);
                    start1 = i;
                }
                2 => start2 = i,
                3 => {
                    self.process_property_seq(&mut ls, res_prop, start1, start2);
                    self.process_property_seq(&mut ls, PROP_ON, start2, i);
                    start1 = i;
                }
                4 => {
                    self.process_property_seq(&mut ls, res_prop, start1, start2);
                    start1 = start2;
                    start2 = i;
                }
                _ => unreachable!("impTabProps has actions 1-4"),
            }
        }
        if matches!(self.last_kept(start, limit), LRI | RLI) && limit < self.text.len() {
            self.isolate_count += 1;
            let saved = Isolate { start_on: ls.start_on, start1, state_imp, state: ls.state };
            let at = self.isolate_count as usize;
            if at == self.isolates.len() {
                self.isolates.push(saved);
            } else {
                self.isolates[at] = saved;
            }
        } else {
            self.process_property_seq(&mut ls, eor, limit, limit);
        }
    }
}
