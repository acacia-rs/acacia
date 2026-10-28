//! `CommandSuggestions.SuggestionsList`: the box of completions over the input, scrolled ten lines
//! at a time, the chosen one previewed grey after the input. Ported from Horizon's `chat.rs` (MIT).

use super::edit_box::EditBox;
use crate::java::command::Suggestion;
use crate::java::gui::Gui;
use crate::java::text::Text;

pub const LINE_LIMIT: usize = 10;
const SELECTED: u32 = 0xFFFF_FF00;
const UNSELECTED: u32 = 0xFFAA_AAAA;

#[derive(Clone, Debug)]
pub struct SuggestionsList {
    /// `rect`: x, y, width, height.
    pub rect: [i32; 4],
    pub original: Vec<u16>,
    pub list: Vec<Suggestion>,
    pub offset: usize,
    pub current: usize,
    last_mouse: (i32, i32),
    tab_cycles: bool,
    fill: u32,
}

impl SuggestionsList {
    /// The list's constructor: `rect` placed by the caller, the first completion selected.
    pub fn new(rect: [i32; 4], original: Vec<u16>, list: Vec<Suggestion>, fill: u32, input: &mut EditBox) -> SuggestionsList {
        let mut s = SuggestionsList { rect, original, list, offset: 0, current: 0, last_mouse: (0, 0), tab_cycles: false, fill };
        s.select(input, 0);
        s
    }

    /// `Rect2i.contains`: edges included.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        let [rx, ry, w, h] = self.rect;
        x >= rx && x <= rx + w && y >= ry && y <= ry + h
    }

    /// `select`: the grey rest of the chosen completion after the input.
    pub fn select(&mut self, input: &mut EditBox, index: isize) {
        let n = self.list.len() as isize;
        let i = if index < 0 { index + n } else if index >= n { index - n } else { index };
        self.current = i as usize;
        let applied = self.list[self.current].apply(&self.original);
        input.suggestion = applied.strip_prefix(input.value().as_str()).map(str::to_owned);
    }

    fn max_offset(&self) -> usize {
        self.list.len().saturating_sub(LINE_LIMIT)
    }

    /// `cycle`.
    pub fn cycle(&mut self, input: &mut EditBox, dir: isize) {
        self.select(input, self.current as isize + dir);
        if self.current < self.offset {
            self.offset = self.current.min(self.max_offset());
        } else if self.current > self.offset + LINE_LIMIT - 1 {
            // lineStartOffset is 1 in the chat screen.
            self.offset = (self.current + 1).saturating_sub(LINE_LIMIT).min(self.max_offset());
        }
    }

    /// `mouseScrolled` over the list: `scroll` lines up (positive) or down.
    pub fn scroll(&mut self, scroll: f64) {
        let offset = (self.offset as f64 - scroll) as i64;
        self.offset = offset.clamp(0, self.max_offset() as i64) as usize;
    }

    /// The completion `useSuggestion` applies: the new value and where the caret goes.
    pub fn chosen(&self) -> (String, usize) {
        let s = &self.list[self.current];
        (s.apply(&self.original), s.start + s.text.encode_utf16().count())
    }

    pub fn tab_cycles(&self) -> bool {
        self.tab_cycles
    }

    pub fn set_tab_cycles(&mut self, on: bool) {
        self.tab_cycles = on;
    }

    /// The line under `y`, if a suggestion is there (`mouseClicked`).
    pub fn line_at(&self, y: i32) -> Option<usize> {
        let line = (y - self.rect[1]) / 12 + self.offset as i32;
        (line >= 0 && (line as usize) < self.list.len()).then_some(line as usize)
    }

    /// `extractRenderState`; hovering a line with a moved mouse selects it.
    pub fn extract(&mut self, gui: &mut Gui, input: &mut EditBox, mouse: (i32, i32)) {
        let limit = self.list.len().min(LINE_LIMIT);
        let [rx, ry, rw, rh] = self.rect;
        let has_prev = self.offset > 0;
        let has_next = self.list.len() > self.offset + limit;
        let moved = self.last_mouse != mouse;
        if moved {
            self.last_mouse = mouse;
        }
        if has_prev || has_next {
            gui.fill(rx, ry - 1, rx + rw, ry, self.fill);
            gui.fill(rx, ry + rh, rx + rw, ry + rh + 1, self.fill);
            for (on, y) in [(has_prev, ry - 1), (has_next, ry + rh)] {
                for x in (0..rw).step_by(2).filter(|_| on) {
                    gui.fill(rx + x, y, rx + x + 1, y + 1, 0xFFFF_FFFF);
                }
            }
        }
        for i in 0..limit {
            let y = ry + 12 * i as i32;
            gui.fill(rx, y, rx + rw, y + 12, self.fill);
            if mouse.0 > rx && mouse.0 < rx + rw && mouse.1 > y && mouse.1 < y + 12 && moved {
                self.select(input, (i + self.offset) as isize);
            }
            let color = if i + self.offset == self.current { SELECTED } else { UNSELECTED };
            gui.text(Text::legacy(&self.list[i + self.offset].text), rx + 1, y + 2, color, true);
        }
    }
}
