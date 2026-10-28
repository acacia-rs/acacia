//! `MultilineTextField`: a wrapped text value with a cursor and a selection anchor, its display
//! lines as `StringSplitter.splitLines` breaks them. Positions are byte offsets on char
//! boundaries (the game's are UTF-16 units: the same for ASCII).

use crate::java::gui::Metrics;
use crate::java::split::{plain_head_by_width, split_lines_plain};
use crate::java::text::Style;
use crate::java::widget::input::Key;
use crate::java::widget::measure::width_str;

/// `StringView`: `[begin, end)` of the value.
pub type View = (usize, usize);

#[derive(Clone, Debug)]
pub struct MultilineField {
    pub value: String,
    pub cursor: usize,
    pub select: usize,
    pub selecting: bool,
    pub width: i32,
    pub char_limit: Option<usize>,
    pub line_limit: Option<usize>,
    lines: Vec<View>,
}

/// `StringUtil.isAllowedChatCharacter`.
pub fn allowed(c: char) -> bool {
    c != '§' && c >= ' ' && c != '\u{7f}'
}

fn truncate(s: &str, n: usize) -> &str {
    s.char_indices().nth(n).map_or(s, |(i, _)| &s[..i])
}

fn lines_of(m: &dyn Metrics, value: &str, width: i32) -> Vec<View> {
    if value.is_empty() {
        return vec![(0, 0)];
    }
    let mut lines: Vec<View> = split_lines_plain(m, value, width, &Style::default(), false).into_iter().map(|(_, a, b)| (a, b)).collect();
    if value.ends_with('\n') {
        lines.push((value.len(), value.len()));
    }
    lines
}

impl MultilineField {
    pub fn new(m: &dyn Metrics, width: i32) -> MultilineField {
        let mut f = MultilineField { value: String::new(), cursor: 0, select: 0, selecting: false, width, char_limit: None, line_limit: None, lines: Vec::new() };
        f.set_value(m, "", false);
        f
    }

    pub fn lines(&self) -> &[View] {
        &self.lines
    }

    fn overflows(&self, m: &dyn Metrics, value: &str) -> bool {
        self.line_limit.is_some_and(|limit| {
            split_lines_plain(m, value, self.width, &Style::default(), false).len() + usize::from(value.ends_with('\n')) > limit
        })
    }

    fn changed(&mut self, m: &dyn Metrics) {
        self.lines = lines_of(m, &self.value, self.width);
    }

    /// `setValue`; true when taken (the cursor listener ran).
    pub fn set_value(&mut self, m: &dyn Metrics, value: &str, allow_overflow: bool) -> bool {
        let value = self.char_limit.map_or(value, |n| truncate(value, n));
        if !allow_overflow && self.overflows(m, value) {
            return false;
        }
        self.value = value.to_owned();
        self.cursor = self.value.len();
        self.select = self.cursor;
        self.changed(m);
        true
    }

    pub fn selected(&self) -> View {
        (self.select.min(self.cursor), self.select.max(self.cursor))
    }

    pub fn has_selection(&self) -> bool {
        self.select != self.cursor
    }

    /// `insertText`: replaces the selection; true when the value changed.
    pub fn insert_text(&mut self, m: &dyn Metrics, input: &str) -> bool {
        if input.is_empty() && !self.has_selection() {
            return false;
        }
        let filtered: String = input.chars().filter(|&c| c == '\n' || allowed(c)).collect();
        let text = match self.char_limit {
            Some(n) => truncate(&filtered, n.saturating_sub(self.value.chars().count())).to_owned(),
            None => filtered,
        };
        let (a, b) = self.selected();
        let value = format!("{}{}{}", &self.value[..a], text, &self.value[b..]);
        if self.overflows(m, &value) {
            return false;
        }
        self.value = value;
        self.cursor = a + text.len();
        self.select = self.cursor;
        self.changed(m);
        true
    }

    fn step(&self, from: usize, dir: i32) -> usize {
        if dir < 0 {
            self.value[..from].char_indices().next_back().map_or(0, |(i, _)| i)
        } else {
            self.value[from..].chars().next().map_or(from, |c| from + c.len_utf8())
        }
    }

    /// `deleteText(dir)` by one character (or the selection).
    pub fn delete_text(&mut self, m: &dyn Metrics, dir: i32) -> bool {
        if !self.has_selection() {
            self.select = self.step(self.cursor, dir);
        }
        self.insert_text(m, "")
    }

    /// `seekCursor(ABSOLUTE, pos)`.
    pub fn seek(&mut self, pos: usize) {
        let mut pos = pos.min(self.value.len());
        while !self.value.is_char_boundary(pos) {
            pos -= 1;
        }
        self.cursor = pos;
        if !self.selecting {
            self.select = self.cursor;
        }
    }

    pub fn line_at_cursor(&self) -> Option<usize> {
        self.lines.iter().position(|&(a, b)| self.cursor >= a && self.cursor <= b)
    }

    /// `getLineView`: clamped to the lines.
    pub fn line_view(&self, i: i32) -> View {
        self.lines[i.clamp(0, self.lines.len() as i32 - 1) as usize]
    }

    fn cursor_line(&self, offset: i32) -> View {
        match self.line_at_cursor() {
            Some(i) => self.line_view(i as i32 + offset),
            None => *self.lines.last().expect("at least one line"),
        }
    }

    /// `seekCursorLine`: to the same pixel column (2 to the right) of another line.
    pub fn seek_line(&mut self, m: &dyn Metrics, offset: i32) {
        if offset == 0 {
            return;
        }
        let left = width_str(m, &self.value[self.cursor_line(0).0..self.cursor]) + 2;
        let (a, b) = self.cursor_line(offset);
        let column = plain_head_by_width(m, &self.value[a..b], left, &Style::default()).len();
        self.seek(a + column);
    }

    /// `seekCursorToPoint`: `(x, y)` from the text's top left.
    pub fn seek_point(&mut self, m: &dyn Metrics, x: f64, y: f64) {
        let (a, b) = self.line_view((y / 9.0).floor() as i32);
        let column = plain_head_by_width(m, &self.value[a..b], x.floor() as i32, &Style::default()).len();
        self.seek(a + column);
    }

    /// `keyPressed` for the keys the GUI routes; true when the cursor listener ran.
    pub fn key_pressed(&mut self, m: &dyn Metrics, key: Key, shift: bool) -> Option<bool> {
        self.selecting = shift;
        Some(match key {
            Key::A => {
                self.cursor = self.value.len();
                self.select = 0;
                false
            }
            Key::Backspace => self.delete_text(m, -1),
            Key::Delete => self.delete_text(m, 1),
            Key::Enter => self.insert_text(m, "\n"),
            Key::Home => {
                self.seek(self.cursor_line(0).0);
                true
            }
            Key::End => {
                self.seek(self.cursor_line(0).1);
                true
            }
            Key::PageUp => {
                self.seek(0);
                true
            }
            Key::PageDown => {
                self.seek(self.value.len());
                true
            }
            Key::Left => {
                self.seek(self.step(self.cursor, -1));
                true
            }
            Key::Right => {
                self.seek(self.step(self.cursor, 1));
                true
            }
            Key::Down => {
                self.seek_line(m, 1);
                true
            }
            Key::Up => {
                self.seek_line(m, -1);
                true
            }
            _ => return None,
        })
    }
}
