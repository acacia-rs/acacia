//! ICU4J's `Bidi.getDirProps` (classes, paragraphs and their default levels, FSI resolved) and
//! `resolveExplicitLevels` (rules X1-X8, brackets gathered for N0 on the way).

use super::brackets::BracketData;
use super::props::*;
use super::ubidi::{Bidi, MIXED};

const NOT_SEEKING_STRONG: u8 = 0;
const SEEKING_STRONG_FOR_PARA: u8 = 1;
const SEEKING_STRONG_FOR_FSI: u8 = 2;
const LOOKING_FOR_PDI: u8 = 3;

/// `UTF16.charAt`: the code point at `i`, a lone surrogate as itself.
fn char_at(text: &[u16], i: usize) -> u32 {
    let c = text[i];
    if (0xD800..0xDC00).contains(&c) {
        if let Some(&low) = text.get(i + 1).filter(|&&l| (0xDC00..0xE000).contains(&l)) {
            return 0x10000 + ((u32::from(c) - 0xD800) << 10) + (u32::from(low) - 0xDC00);
        }
    }
    u32::from(c)
}

impl Bidi {
    pub(super) fn get_dir_props(&mut self) {
        let n = self.text.len();
        self.flags = 0;
        let is_default = self.default_para_level != 0;
        let default_level = self.para_level & 1;
        let mut isolate_start = [0usize; 126];
        let mut previous_state = [0u8; 126];
        let mut stack_last: isize = -1;
        let mut state;
        if is_default {
            self.paras = vec![(0, default_level)];
            state = SEEKING_STRONG_FOR_PARA;
        } else {
            self.paras = vec![(0, self.para_level)];
            state = NOT_SEEKING_STRONG;
        }
        let mut i = 0;
        while i < n {
            let i0 = i;
            let c = char_at(&self.text, i);
            i += if c > 0xFFFF { 2 } else { 1 };
            let dir_prop = class(c);
            self.flags |= flag(dir_prop);
            self.dir_props[i - 1] = dir_prop;
            if i - 1 > i0 {
                self.flags |= flag(BN);
                self.dir_props[i0] = BN;
            }
            let last = self.paras.len() - 1;
            let top = usize::try_from(stack_last).ok().filter(|&t| t <= 125);
            if dir_prop == L {
                if state == SEEKING_STRONG_FOR_PARA {
                    self.paras[last].1 = 0;
                    state = NOT_SEEKING_STRONG;
                } else if state == SEEKING_STRONG_FOR_FSI {
                    if stack_last <= 125 {
                        self.flags |= flag(LRI);
                    }
                    state = LOOKING_FOR_PDI;
                }
            } else if dir_prop == R || dir_prop == AL {
                if state == SEEKING_STRONG_FOR_PARA {
                    self.paras[last].1 = 1;
                    state = NOT_SEEKING_STRONG;
                } else if state == SEEKING_STRONG_FOR_FSI {
                    if let Some(t) = top {
                        self.dir_props[isolate_start[t]] = RLI;
                        self.flags |= flag(RLI);
                    }
                    state = LOOKING_FOR_PDI;
                }
            } else if (FSI..=RLI).contains(&dir_prop) {
                stack_last += 1;
                if stack_last <= 125 {
                    isolate_start[stack_last as usize] = i - 1;
                    previous_state[stack_last as usize] = state;
                }
                if dir_prop == FSI {
                    self.dir_props[i - 1] = LRI;
                    state = SEEKING_STRONG_FOR_FSI;
                } else {
                    state = LOOKING_FOR_PDI;
                }
            } else if dir_prop == PDI {
                if state == SEEKING_STRONG_FOR_FSI && stack_last <= 125 {
                    self.flags |= flag(LRI);
                }
                if stack_last >= 0 {
                    if let Some(t) = top {
                        state = previous_state[t];
                    }
                    stack_last -= 1;
                }
            } else if dir_prop == B && (i >= n || c != 13 || self.text[i] != 10) {
                self.paras[last].0 = i;
                if i < n {
                    self.paras.push((0, if is_default { default_level } else { self.para_level }));
                    state = if is_default { SEEKING_STRONG_FOR_PARA } else { NOT_SEEKING_STRONG };
                    stack_last = -1;
                }
            }
        }
        if stack_last > 125 {
            stack_last = 125;
            state = SEEKING_STRONG_FOR_FSI;
        }
        while stack_last >= 0 {
            if state == SEEKING_STRONG_FOR_FSI {
                self.flags |= flag(LRI);
                break;
            }
            state = previous_state[stack_last as usize];
            stack_last -= 1;
        }
        let last = self.paras.len() - 1;
        self.paras[last].0 = n;
        if is_default {
            self.para_level = self.paras[0].1;
        }
        for &(_, level) in &self.paras {
            self.flags |= flag_lr(level);
        }
    }

    /// The paragraphs' character ranges with their levels.
    fn para_ranges(&self) -> Vec<(usize, usize, i8)> {
        let mut start = 0;
        self.paras.iter().map(|&(limit, level)| {
            let r = (start, limit, level);
            start = limit;
            r
        }).collect()
    }

    fn breaks_paragraph(&self, i: usize) -> bool {
        i + 1 < self.text.len() && (self.text[i] != 13 || self.text[i + 1] != 10)
    }

    pub(super) fn resolve_explicit_levels(&mut self) -> u8 {
        let n = self.text.len();
        let level = self.para_level_at(0);
        self.isolate_count = 0;
        let direction = self.direction_from_flags();
        if direction != MIXED {
            return direction;
        }
        let mut bd = BracketData::new(level);
        if self.flags & (MASK_EXPLICIT | MASK_ISO) == 0 {
            for (start, limit, level) in self.para_ranges() {
                for i in start..limit {
                    self.levels[i] = level;
                    match self.dir_props[i] {
                        BN => {}
                        B => {
                            if self.breaks_paragraph(i) {
                                bd.process_b(level);
                            }
                        }
                        _ => self.bracket_process_char(&mut bd, i),
                    }
                }
            }
            return direction;
        }
        let (mut embedding, mut previous) = (level, level);
        let mut last_cc_pos = 0;
        let mut stack = [0i16; 127];
        let mut stack_last = 0usize;
        let (mut overflow_isolates, mut overflow_embeddings, mut valid_isolates) = (0i32, 0i32, 0isize);
        stack[0] = i16::from(level);
        self.flags = 0;
        for i in 0..n {
            let dir_prop = self.dir_props[i];
            match dir_prop {
                B => {
                    self.flags |= flag(B);
                    self.levels[i] = self.para_level_at(i);
                    if self.breaks_paragraph(i) {
                        (overflow_isolates, overflow_embeddings, valid_isolates, stack_last) = (0, 0, 0, 0);
                        embedding = self.para_level_at(i + 1);
                        previous = embedding;
                        stack[0] = i16::from(embedding);
                        bd.process_b(embedding);
                    }
                }
                LRE | LRO | RLE | RLO => {
                    self.flags |= flag(BN);
                    self.levels[i] = previous;
                    let new_level = if dir_prop == LRE || dir_prop == LRO {
                        ((i32::from(embedding) + 2) & 126) as i8
                    } else {
                        ((i32::from(no_override(embedding)) + 1) | 1) as i8
                    };
                    if new_level <= 125 && overflow_isolates == 0 && overflow_embeddings == 0 {
                        last_cc_pos = i;
                        embedding = new_level;
                        if dir_prop == LRO || dir_prop == RLO {
                            embedding |= -128;
                        }
                        stack_last += 1;
                        stack[stack_last] = i16::from(embedding);
                    } else if overflow_isolates == 0 {
                        overflow_embeddings += 1;
                    }
                }
                PDF => {
                    self.flags |= flag(BN);
                    self.levels[i] = previous;
                    if overflow_isolates <= 0 {
                        if overflow_embeddings > 0 {
                            overflow_embeddings -= 1;
                        } else if stack_last > 0 && stack[stack_last] < 256 {
                            last_cc_pos = i;
                            stack_last -= 1;
                            embedding = stack[stack_last] as i8;
                        }
                    }
                }
                BN => {
                    self.levels[i] = previous;
                    self.flags |= flag(BN);
                }
                LRI | RLI => {
                    self.flags |= flag(ON) | flag_lr(embedding);
                    self.levels[i] = no_override(embedding);
                    if no_override(embedding) != no_override(previous) {
                        self.bracket_process_boundary(&mut bd, last_cc_pos, previous, embedding);
                        self.flags |= MULTI_RUNS;
                    }
                    previous = embedding;
                    let new_level = if dir_prop == LRI {
                        ((i32::from(embedding) + 2) & 126) as i8
                    } else {
                        ((i32::from(no_override(embedding)) + 1) | 1) as i8
                    };
                    if new_level <= 125 && overflow_isolates == 0 && overflow_embeddings == 0 {
                        self.flags |= flag(dir_prop);
                        last_cc_pos = i;
                        valid_isolates += 1;
                        self.isolate_count = self.isolate_count.max(valid_isolates);
                        embedding = new_level;
                        stack_last += 1;
                        stack[stack_last] = i16::from(embedding) + 256;
                        bd.process_lri_rli(embedding);
                    } else {
                        self.dir_props[i] = WS;
                        overflow_isolates += 1;
                    }
                }
                PDI => {
                    if no_override(embedding) != no_override(previous) {
                        self.bracket_process_boundary(&mut bd, last_cc_pos, previous, embedding);
                        self.flags |= MULTI_RUNS;
                    }
                    if overflow_isolates > 0 {
                        overflow_isolates -= 1;
                        self.dir_props[i] = WS;
                    } else if valid_isolates > 0 {
                        self.flags |= flag(PDI);
                        last_cc_pos = i;
                        overflow_embeddings = 0;
                        while stack[stack_last] < 256 {
                            stack_last -= 1;
                        }
                        stack_last -= 1;
                        valid_isolates -= 1;
                        bd.process_pdi();
                    } else {
                        self.dir_props[i] = WS;
                    }
                    embedding = (stack[stack_last] & !256) as i8;
                    self.flags |= flag(ON) | flag_lr(embedding);
                    previous = embedding;
                    self.levels[i] = no_override(embedding);
                }
                _ => {
                    if no_override(embedding) != no_override(previous) {
                        self.bracket_process_boundary(&mut bd, last_cc_pos, previous, embedding);
                        self.flags |= MULTI_RUNS;
                        self.flags |= if embedding < 0 { flag_o(embedding) } else { flag_e(embedding) };
                    }
                    previous = embedding;
                    self.levels[i] = embedding;
                    self.bracket_process_char(&mut bd, i);
                    self.flags |= flag(self.dir_props[i]);
                }
            }
        }
        if self.flags & MASK_EMBEDDING != 0 {
            self.flags |= flag_lr(self.para_level);
        }
        self.direction_from_flags()
    }
}
