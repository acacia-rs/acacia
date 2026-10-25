//! The anvil's name field: an `EditBox` without a border, focused, its cursor kept at the end of
//! the text (where `setValue` leaves it), scrolled as `scrollTo` scrolls it.

use super::item_combiner_text::{plain_head, plain_tail};
use crate::java::gui::{Gui, Metrics};
use crate::java::text::Text;

pub struct NameBox {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub max_length: usize,
    pub value: Vec<char>,
    pub cursor: usize,
    pub display: usize,
    pub focused: bool,
    /// The cursor's blink is in its visible phase.
    pub cursor_shown: bool,
    pub color: u32,
}

impl NameBox {
    pub fn new(x: i32, y: i32, width: i32, max_length: usize) -> NameBox {
        NameBox { x, y, width, max_length, value: Vec::new(), cursor: 0, display: 0, focused: false, cursor_shown: true, color: 0xFFFF_FFFF }
    }

    /// `setValue`: truncated to the limit, the cursor and selection moved to the end.
    pub fn set_value(&mut self, font: &dyn Metrics, value: &str) {
        self.value = value.chars().take(self.max_length).collect();
        self.cursor = self.value.len();
        // setCursorPosition, setHighlightPos in moveCursorTo, then setValue's own setHighlightPos.
        for _ in 0..3 {
            self.scroll_to(font, self.cursor);
        }
    }

    /// `scrollTo`.
    fn scroll_to(&mut self, font: &dyn Metrics, pos: usize) {
        self.display = self.display.min(self.value.len());
        let shown = plain_head(font, &self.value[self.display..], self.width);
        let last = shown + self.display;
        let mut display = self.display as i64;
        if pos == self.display {
            display -= plain_tail(font, &self.value, self.width) as i64;
        }
        if pos > last {
            display += (pos - last) as i64;
        } else if pos as i64 <= display {
            display = pos as i64;
        }
        self.display = display.clamp(0, self.value.len() as i64) as usize;
    }

    fn string(chars: &[char]) -> Text {
        Text::literal(chars.iter().collect::<String>())
    }

    /// `extractWidgetRenderState` with no selection, hint or suggestion.
    pub fn extract(&self, gui: &mut Gui, font: &dyn Metrics) {
        let color = self.color;
        let rel = self.cursor as i64 - self.display as i64;
        let tail = &self.value[self.display..];
        let shown = &tail[..plain_head(font, tail, self.width)];
        let on_screen = rel >= 0 && rel as usize <= shown.len();
        let (text_x, text_y) = (self.x, self.y);
        let mut draw_x = text_x;
        if !shown.is_empty() {
            let half = if on_screen { &shown[..rel as usize] } else { shown };
            let half = Self::string(half);
            draw_x += font.width(&half) + 1;
            gui.text(half, text_x, text_y, color, true);
        }
        let insert = self.cursor < self.value.len() || self.value.len() >= self.max_length;
        let mut cursor_x = draw_x;
        if !on_screen {
            cursor_x = if rel > 0 { text_x + self.width } else { text_x };
        } else if insert {
            cursor_x -= 1;
            draw_x -= 1;
        }
        if !shown.is_empty() && on_screen && (rel as usize) < shown.len() {
            gui.text(Self::string(&shown[rel as usize..]), draw_x, text_y, color, true);
        }
        if self.focused && self.cursor_shown && on_screen {
            if insert {
                gui.fill(cursor_x, text_y - 1, cursor_x + 1, text_y + 10, color);
            } else {
                gui.text("_", cursor_x, text_y, color, true);
            }
        }
    }
}
