//! `EditBox`: a one-line text field with a caret, a selection and a window scrolled over text too
//! long for it. The value is kept as UTF-16 and every position counts UTF-16 units, as Java's do,
//! so the scroll arithmetic lands where the game's does. Only what the chat input uses is here: no
//! border sprite, hint, centring or clipboard. Ported from Horizon's `screen_draw.rs` (MIT) and
//! checked against the game's `EditBox`.

use crate::java::gui::Metrics;
use crate::java::split::{plain_head_by_width, plain_tail_by_width};
use crate::java::text::{Style, Text};

/// A key the text field or the screen around it handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Backspace,
    Delete,
    Home,
    End,
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    Tab,
    Escape,
    Enter,
    /// The `A` key (Ctrl+A selects all).
    A,
}

/// `EditBox.DEFAULT_TEXT_COLOR`.
pub const TEXT_COLOR: u32 = 0xFFE0_E0E0;

#[derive(Clone, Debug)]
pub struct EditBox {
    value: Vec<u16>,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub max_length: usize,
    pub bordered: bool,
    pub focused: bool,
    pub cursor: usize,
    pub highlight: usize,
    pub display: usize,
    /// Grey text drawn after the value when the caret is at its end.
    pub suggestion: Option<String>,
}

fn units(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `StringUtil.isAllowedChatCharacter`.
pub fn allowed_chat_character(c: char) -> bool {
    c != '§' && c >= ' ' && c != '\u{7f}'
}

impl EditBox {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> EditBox {
        EditBox { value: Vec::new(), x, y, width, height, max_length: 32, bordered: true, focused: false, cursor: 0, highlight: 0, display: 0, suggestion: None }
    }

    pub fn value(&self) -> String {
        self.slice(0, self.value.len())
    }

    pub fn len(&self) -> usize {
        self.value.len()
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// `value.substring(from, to)`.
    pub fn slice(&self, from: usize, to: usize) -> String {
        String::from_utf16_lossy(&self.value[from.min(to)..to])
    }

    pub fn inner_width(&self) -> i32 {
        if self.bordered { self.width - 8 } else { self.width }
    }

    pub(super) fn text_x(&self) -> i32 {
        self.x + if self.bordered { 4 } else { 0 }
    }

    pub(super) fn text_y(&self) -> i32 {
        if self.bordered { self.y + (self.height - 8) / 2 } else { self.y }
    }

    /// `setValue`; the responder runs after it (it always does).
    pub fn set_value(&mut self, m: &dyn Metrics, value: &str) {
        self.value = value.encode_utf16().take(self.max_length).collect();
        self.move_cursor_to(m, self.value.len(), false);
        self.set_highlight(m, self.cursor);
    }

    /// `insertText`: replaces the selection; true when the responder runs (room was left).
    pub fn insert_text(&mut self, m: &dyn Metrics, input: &str) -> bool {
        let (start, end) = (self.cursor.min(self.highlight), self.cursor.max(self.highlight));
        let room = self.max_length as isize - self.value.len() as isize + (end - start) as isize;
        if room <= 0 {
            return false;
        }
        let mut text = Vec::new();
        for c in input.chars().filter(|&c| allowed_chat_character(c)) {
            let mut buf = [0u16; 2];
            let encoded = c.encode_utf16(&mut buf);
            // A surrogate pair is never cut in half.
            if text.len() + encoded.len() > room as usize {
                break;
            }
            text.extend_from_slice(encoded);
        }
        let inserted = text.len();
        self.value.splice(start..end, text);
        self.set_cursor(m, start + inserted);
        self.set_highlight(m, self.cursor);
        true
    }

    /// `deleteText`: a character or (`whole_word`) a word in `dir`, or the selection.
    pub fn delete_text(&mut self, m: &dyn Metrics, dir: isize, whole_word: bool) -> bool {
        if whole_word && !self.value.is_empty() && self.highlight == self.cursor {
            let to = self.word_position(dir, self.cursor, true);
            return self.delete_to(m, to);
        }
        if whole_word && !self.value.is_empty() {
            return self.insert_text(m, "");
        }
        let to = self.offset_by_codepoints(self.cursor, dir);
        self.delete_to(m, to)
    }

    /// `deleteCharsToPos`.
    fn delete_to(&mut self, m: &dyn Metrics, pos: usize) -> bool {
        if self.value.is_empty() {
            return false;
        }
        if self.highlight != self.cursor {
            return self.insert_text(m, "");
        }
        let (start, end) = (pos.min(self.cursor), pos.max(self.cursor));
        if start == end {
            return false;
        }
        self.value.drain(start..end);
        self.set_cursor(m, start);
        self.move_cursor_to(m, start, false);
        true
    }

    /// `Util.offsetByCodepoints`.
    fn offset_by_codepoints(&self, mut pos: usize, offset: isize) -> usize {
        let v = &self.value;
        let high = |u: u16| (0xD800..0xDC00).contains(&u);
        let low = |u: u16| (0xDC00..0xE000).contains(&u);
        if offset >= 0 {
            for _ in 0..offset {
                if pos >= v.len() {
                    break;
                }
                pos += 1;
                if high(v[pos - 1]) && pos < v.len() && low(v[pos]) {
                    pos += 1;
                }
            }
        } else {
            for _ in 0..-offset {
                if pos == 0 {
                    break;
                }
                pos -= 1;
                if low(v[pos]) && pos > 0 && high(v[pos - 1]) {
                    pos -= 1;
                }
            }
        }
        pos
    }

    /// `EditBox.getWordPosition`: words are separated by spaces only.
    pub fn word_position(&self, dir: isize, from: usize, strip_spaces: bool) -> usize {
        let space = u16::from(b' ');
        let v = &self.value;
        let mut result = from;
        for _ in 0..dir.unsigned_abs() {
            if dir > 0 {
                result = v.iter().skip(result).position(|&u| u == space).map_or(v.len(), |i| i + result);
                while strip_spaces && result < v.len() && v[result] == space {
                    result += 1;
                }
            } else {
                while strip_spaces && result > 0 && v[result - 1] == space {
                    result -= 1;
                }
                while result > 0 && v[result - 1] != space {
                    result -= 1;
                }
            }
        }
        result
    }

    /// `moveCursorTo`: the caret to `pos`, the selection kept when `extend`.
    pub fn move_cursor_to(&mut self, m: &dyn Metrics, pos: usize, extend: bool) {
        self.set_cursor(m, pos);
        if !extend {
            self.set_highlight(m, self.cursor);
        }
    }

    /// `setCursorPosition`.
    pub fn set_cursor(&mut self, m: &dyn Metrics, pos: usize) {
        self.cursor = pos.min(self.value.len());
        self.scroll_to(m, self.cursor);
    }

    /// `setHighlightPos`.
    pub fn set_highlight(&mut self, m: &dyn Metrics, pos: usize) {
        self.highlight = pos.min(self.value.len());
        self.scroll_to(m, self.highlight);
    }

    /// `scrollTo`: the window moved so `pos` shows.
    fn scroll_to(&mut self, m: &dyn Metrics, pos: usize) {
        let len = self.value.len() as isize;
        let inner = self.inner_width();
        let style = Style::default();
        let mut display = (self.display as isize).min(len);
        let tail = self.slice(display as usize, self.value.len());
        let last = units(plain_head_by_width(m, &tail, inner, &style)) as isize + display;
        let pos = pos as isize;
        // As the game does it: back by what of the whole value's end fits, not of the head.
        if pos == display {
            display -= units(plain_tail_by_width(m, &self.value(), inner, &style)) as isize;
        }
        if pos > last {
            display += pos - last;
        } else if pos <= display {
            display = pos;
        }
        self.display = display.clamp(0, len) as usize;
    }

    /// `EditBox.keyPressed`: `None` when the key is not the field's, else whether the responder runs.
    pub fn key(&mut self, m: &dyn Metrics, key: Key, shift: bool, ctrl: bool) -> Option<bool> {
        Some(match key {
            Key::Backspace => self.delete_text(m, -1, ctrl),
            Key::Delete => self.delete_text(m, 1, ctrl),
            Key::Home => {
                self.move_cursor_to(m, 0, shift);
                false
            }
            Key::End => {
                self.move_cursor_to(m, self.value.len(), shift);
                false
            }
            Key::Left | Key::Right => {
                let dir = if key == Key::Right { 1 } else { -1 };
                let to = if ctrl { self.word_position(dir, self.cursor, true) } else { self.offset_by_codepoints(self.cursor, dir) };
                self.move_cursor_to(m, to, shift);
                false
            }
            Key::A if ctrl && !shift => {
                self.move_cursor_to(m, self.value.len(), false);
                self.set_highlight(m, 0);
                false
            }
            _ => return None,
        })
    }

    /// `charTyped`: true when the responder runs.
    pub fn char_typed(&mut self, m: &dyn Metrics, c: char) -> bool {
        allowed_chat_character(c) && self.insert_text(m, &c.to_string())
    }

    /// `isMouseOver` (`areCoordinatesInRectangle`).
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= f64::from(self.x) && y >= f64::from(self.y) && x < f64::from(self.x + self.width) && y < f64::from(self.y + self.height)
    }

    /// `onClick`: the caret to the character under `x`.
    pub fn click(&mut self, m: &dyn Metrics, x: f64, shift: bool) {
        let width = (crate::java::mth::floor_d(x) - self.text_x()).min(self.inner_width());
        let shown = self.slice(self.display, self.value.len());
        let pos = self.display + units(plain_head_by_width(m, &shown, width, &Style::default()));
        self.move_cursor_to(m, pos, shift);
    }

    /// `getScreenX`.
    pub fn screen_x(&self, m: &dyn Metrics, index: usize) -> i32 {
        if index > self.value.len() { self.x } else { self.x + m.width(&Text::literal(self.slice(0, index))) }
    }
}
