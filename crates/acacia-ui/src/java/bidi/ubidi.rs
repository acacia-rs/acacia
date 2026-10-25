//! ICU4J's `Bidi.setPara` for what `FormattedBidiReorder` asks of it: one text (possibly several
//! paragraphs), a default paragraph level, `REORDER_DEFAULT`, no options, no context, no custom
//! classifier. Levels are Java bytes (`i8`): bit 7 is the override flag while explicit levels resolve.

use super::props::*;

/// `Bidi.Isolate`: the implicit-level state saved at an isolate initiator, resumed at its PDI.
#[derive(Clone, Copy, Default)]
pub(super) struct Isolate {
    pub start_on: isize,
    pub start1: usize,
    pub state_imp: u8,
    pub state: u8,
}

pub(super) struct Bidi {
    pub text: Vec<u16>,
    pub dir_props: Vec<u8>,
    pub levels: Vec<i8>,
    pub flags: u32,
    pub para_level: i8,
    pub default_para_level: i8,
    /// Each paragraph's limit and level.
    pub paras: Vec<(usize, i8)>,
    /// `LTR` (0), `RTL` (1) or `MIXED` (2).
    pub direction: u8,
    pub trailing_ws_start: usize,
    pub isolate_count: isize,
    pub isolates: Vec<Isolate>,
}

pub(super) const MIXED: u8 = 2;

fn is_default_level(level: i8) -> bool {
    level as u8 & 126 == 126
}

impl Bidi {
    /// `new Bidi(text, paraLevel)` (`LEVEL_DEFAULT_LTR` is 126).
    pub fn new(text: Vec<u16>, para_level: i8) -> Bidi {
        let n = text.len();
        let mut b = Bidi {
            text,
            dir_props: vec![0; n],
            levels: vec![0; n],
            flags: 0,
            para_level,
            default_para_level: if is_default_level(para_level) { para_level } else { 0 },
            paras: Vec::new(),
            direction: (para_level & 1) as u8,
            trailing_ws_start: n,
            isolate_count: -1,
            isolates: Vec::new(),
        };
        if n == 0 {
            if is_default_level(para_level) {
                b.para_level &= 1;
                b.default_para_level = 0;
            }
            return b;
        }
        b.get_dir_props();
        b.direction = b.resolve_explicit_levels();
        b.isolate_count = -1;
        if b.direction != MIXED {
            b.trailing_ws_start = 0;
            return b;
        }
        if b.paras.len() <= 1 && b.flags & MULTI_RUNS == 0 {
            let (sor, eor) = (lr(b.para_level_at(0)), lr(b.para_level_at(n - 1)));
            b.resolve_implicit_levels(0, n, sor, eor);
        } else {
            b.resolve_level_runs();
        }
        b.adjust_ws_levels();
        b
    }

    /// `GetParaLevelAt`.
    pub fn para_level_at(&self, index: usize) -> i8 {
        if self.default_para_level == 0 || index < self.paras[0].0 {
            return self.para_level;
        }
        let mut i = 1;
        while i < self.paras.len() && index >= self.paras[i].0 {
            i += 1;
        }
        self.paras[i.min(self.paras.len() - 1)].1
    }

    /// `setPara`'s loop over level runs, each resolved with its own sos and eos.
    fn resolve_level_runs(&mut self) {
        let n = self.text.len();
        let mut limit = 0;
        let level0 = self.para_level_at(0);
        let mut next_level = self.levels[0];
        let mut eor = if level0 < next_level { lr(next_level) } else { lr(level0) };
        loop {
            let start = limit;
            let level = next_level;
            let sor = if start > 0 && self.dir_props[start - 1] == B { lr(self.para_level_at(start)) } else { eor };
            loop {
                limit += 1;
                if !(limit < n && (self.levels[limit] == level || flag(self.dir_props[limit]) & MASK_BN_EXPLICIT != 0)) {
                    break;
                }
            }
            next_level = if limit < n { self.levels[limit] } else { self.para_level_at(n - 1) };
            eor = if no_override(level) < no_override(next_level) { lr(next_level) } else { lr(level) };
            if level >= 0 {
                self.resolve_implicit_levels(start, limit, sor, eor);
            } else {
                for l in &mut self.levels[start..limit] {
                    *l &= 127;
                }
            }
            if limit >= n {
                break;
            }
        }
    }

    /// `directionFromFlags`.
    pub fn direction_from_flags(&self) -> u8 {
        if self.flags & MASK_RTL != 0 || (self.flags & flag(AN) != 0 && self.flags & MASK_POSSIBLE_N != 0) {
            if self.flags & MASK_LTR == 0 { 1 } else { MIXED }
        } else {
            0
        }
    }

    /// `adjustWSLevels` (rule L1 for segment and paragraph separators and the text's end; removed
    /// characters take the level of what follows them).
    fn adjust_ws_levels(&mut self) {
        if self.flags & MASK_WS == 0 {
            return;
        }
        let mut i = self.trailing_ws_start;
        while i > 0 {
            while i > 0 {
                i -= 1;
                if flag(self.dir_props[i]) & MASK_WS == 0 {
                    break;
                }
                self.levels[i] = self.para_level_at(i);
            }
            while i > 0 {
                i -= 1;
                let f = flag(self.dir_props[i]);
                if f & MASK_BN_EXPLICIT != 0 {
                    self.levels[i] = self.levels[i + 1];
                } else if f & MASK_B_S != 0 {
                    self.levels[i] = self.para_level_at(i);
                    break;
                }
            }
        }
    }
}
