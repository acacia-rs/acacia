//! `CommandSuggestions` for a chat message (input not starting with `/`): Tab lists the names
//! (`getCustomTabSuggestions`, the online players) matching the word at the caret, and the list
//! box above the input cycles and applies them. Brigadier's `Suggestion(s)` and
//! `SharedSuggestionProvider.suggest` come along. Positions are UTF-16 units, as `EditBox`'s.
//! Ported from Horizon's `chat.rs` (MIT).
// TODO: commands (`/`): parsing against the client command tree, argument colours, usage lines
// and parse errors need a Brigadier dispatcher and the vanilla command tree.

use super::edit_box::EditBox;
use crate::java::gui::{Gui, Metrics};
use crate::java::text::Text;

/// `CommandSuggestions`' fill colour in the chat screen (`ChatScreen.USAGE_BACKGROUND_COLOR`).
pub const FILL: u32 = 0xD000_0000;
const LINE_LIMIT: usize = 10;
const SELECTED: u32 = 0xFFFF_FF00;
const UNSELECTED: u32 = 0xFFAA_AAAA;

/// A Brigadier `Suggestion`: `text` replacing units `[start, end)` of the input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

impl Suggestion {
    /// `Suggestion.apply`.
    pub fn apply(&self, input: &[u16]) -> String {
        let mut out: Vec<u16> = input[..self.start.min(input.len())].to_vec();
        out.extend(self.text.encode_utf16());
        out.extend_from_slice(&input[self.end.min(input.len())..]);
        String::from_utf16_lossy(&out)
    }
}

/// `CommandSuggestions.getLastWordIndex`: after the last run of `\s`.
pub fn last_word_index(s: &[u16]) -> usize {
    let space = |u: u16| matches!(u, 9..=13 | 32);
    (1..=s.len()).rev().find(|&i| space(s[i - 1])).unwrap_or(0)
}

/// `SharedSuggestionProvider.matchesSubStr`: `pattern` at the start of `input` or after a `.`,
/// `_` or `/`.
pub fn matches_sub_str(pattern: &str, input: &str) -> bool {
    let mut index = 0;
    while !input[index..].starts_with(pattern) {
        match input[index..].find(['.', '_', '/']) {
            Some(i) => index += i + 1,
            None => return false,
        }
    }
    true
}

/// `SharedSuggestionProvider.suggest(values, new SuggestionsBuilder(input, start))` and
/// `Suggestions.create`: the matches, deduplicated, sorted ignoring case.
pub fn suggest<'a>(values: impl IntoIterator<Item = &'a str>, input: &[u16], start: usize) -> Vec<Suggestion> {
    let remaining = String::from_utf16_lossy(&input[start..]);
    let lower = remaining.to_lowercase();
    let mut out: Vec<Suggestion> = Vec::new();
    for v in values {
        let s = Suggestion { start, end: input.len(), text: v.to_owned() };
        if matches_sub_str(&lower, &v.to_lowercase()) && v != remaining && !out.contains(&s) {
            out.push(s);
        }
    }
    out.sort_by_key(|s| s.text.to_lowercase());
    out
}

/// `CommandSuggestions.SuggestionsList`.
#[derive(Clone, Debug)]
pub struct SuggestionsList {
    /// `rect`: x, y, width, height.
    pub rect: [i32; 4],
    original: Vec<u16>,
    list: Vec<Suggestion>,
    pub offset: usize,
    pub current: usize,
    last_mouse: (i32, i32),
    tab_cycles: bool,
}

impl SuggestionsList {
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

    /// `cycle`.
    pub fn cycle(&mut self, input: &mut EditBox, dir: isize) {
        self.select(input, self.current as isize + dir);
        let max = self.list.len().saturating_sub(LINE_LIMIT);
        if self.current < self.offset {
            self.offset = self.current.min(max);
        } else if self.current > self.offset + LINE_LIMIT - 1 {
            self.offset = (self.current + 1).saturating_sub(LINE_LIMIT).min(max);
        }
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

    /// `SuggestionsList.extractRenderState`; hovering a line with a moved mouse selects it.
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
            gui.fill(rx, ry - 1, rx + rw, ry, FILL);
            gui.fill(rx, ry + rh, rx + rw, ry + rh + 1, FILL);
            for (on, y) in [(has_prev, ry - 1), (has_next, ry + rh)] {
                for x in (0..rw).step_by(2).filter(|_| on) {
                    gui.fill(rx + x, y, rx + x + 1, y + 1, 0xFFFF_FFFF);
                }
            }
        }
        for i in 0..limit {
            let y = ry + 12 * i as i32;
            gui.fill(rx, y, rx + rw, y + 12, FILL);
            if mouse.0 > rx && mouse.0 < rx + rw && mouse.1 > y && mouse.1 < y + 12 && moved {
                self.select(input, (i + self.offset) as isize);
            }
            let color = if i + self.offset == self.current { SELECTED } else { UNSELECTED };
            gui.text(Text::legacy(&self.list[i + self.offset].text), rx + 1, y + 2, color, true);
        }
    }
}

/// `CommandSuggestions` as the chat screen sets it up (anchored to the bottom, ten lines, never
/// hidden by Tab).
#[derive(Clone, Debug, Default)]
pub struct CommandSuggestions {
    /// `pendingSuggestions`: done at once for messages.
    pending: Option<Vec<Suggestion>>,
    pub list: Option<SuggestionsList>,
    pub allow_suggestions: bool,
    pub keep_suggestions: bool,
}

impl CommandSuggestions {
    /// `updateCommandInfo` for a message: the names matching the word before the caret.
    pub fn update(&mut self, input: &mut EditBox, names: &[String]) {
        if !self.keep_suggestions {
            input.suggestion = None;
            self.list = None;
        }
        let value: Vec<u16> = input.value().encode_utf16().collect();
        self.pending = if input.value().chars().all(char::is_whitespace) {
            None
        } else {
            let partial = &value[..input.cursor];
            Some(suggest(names.iter().map(String::as_str), partial, last_word_index(partial)))
        };
    }

    /// `setAllowSuggestions`.
    pub fn set_allow(&mut self, allow: bool) {
        self.allow_suggestions = allow;
        if !allow {
            self.list = None;
        }
    }

    /// `showSuggestions`: the list over the input, at the start of the word being completed.
    pub fn show(&mut self, m: &dyn Metrics, input: &mut EditBox, screen_height: i32) {
        let Some(found) = self.pending.as_ref().filter(|p| !p.is_empty()) else { return };
        let width = found.iter().map(|s| m.width(&Text::legacy(&s.text))).max().unwrap_or(0);
        let start = found.iter().map(|s| s.start).min().unwrap_or(0);
        let x = clamp(input.screen_x(m, start), 0, input.screen_x(m, 0) + input.inner_width() - width);
        let y = screen_height - 12;
        // sortSuggestions: those starting with the word at the caret first.
        let value: Vec<u16> = input.value().encode_utf16().collect();
        let partial = &value[..input.cursor];
        let word = String::from_utf16_lossy(&partial[last_word_index(partial)..]).to_lowercase();
        let (mut first, rest): (Vec<Suggestion>, Vec<Suggestion>) =
            found.iter().cloned().partition(|s| s.text.starts_with(&word) || s.text.starts_with(&format!("minecraft:{word}")));
        first.extend(rest);
        let n = first.len().min(LINE_LIMIT) as i32;
        let left = x - i32::from(!input.bordered);
        let mut list = SuggestionsList {
            rect: [left, y - 3 - n * 12, width + 1, n * 12],
            original: value,
            list: first,
            offset: 0,
            current: 0,
            last_mouse: (0, 0),
            tab_cycles: false,
        };
        list.select(input, 0);
        self.list = Some(list);
    }
}

/// `Mth.clamp(int)`: the minimum wins over a smaller maximum.
fn clamp(v: i32, lo: i32, hi: i32) -> i32 {
    if v < lo { lo } else { v.min(hi) }
}
