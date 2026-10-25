//! The creative screen's search box: an unbordered `EditBox` (80 × 9, white text, no hint or
//! suggestion), its scrolling (`scrollTo`) and drawing (`extractWidgetRenderState`).
// TODO: the screens agent's EditBox widget once merged; this covers only what the creative screen sets.

use crate::java::gui::{Gui, Metrics};
use crate::java::text::{Style, Text};

const WIDTH: i32 = 80;
const MAX_LENGTH: usize = 50;
const COLOR: u32 = 0xFFFF_FFFF;

/// `StringSplitter.plainIndexAtWidth` over unformatted text.
fn fit(font: &dyn Metrics, chars: &[char], max: i32) -> usize {
    let mut left = max as f32;
    let mut n = 0;
    for &c in chars {
        left -= font.advance(c, &Style::default());
        if left < 0.0 {
            break;
        }
        n += 1;
    }
    n
}

/// `plainTailByWidth`'s length.
fn fit_tail(font: &dyn Metrics, chars: &[char], max: i32) -> usize {
    let mut width = 0.0;
    let mut n = 0;
    for &c in chars.iter().rev() {
        width += font.advance(c, &Style::default());
        if width > max as f32 {
            break;
        }
        n += 1;
    }
    n
}

/// The box's state; positions are `char` indices (UTF-16 units for the BMP text it holds).
#[derive(Clone, Debug, Default)]
pub struct SearchBox {
    pub x: i32,
    pub y: i32,
    pub value: Vec<char>,
    pub cursor: usize,
    pub highlight: usize,
    pub display: usize,
    /// `TextCursorUtils.isCursorVisible` at this frame.
    pub blink_on: bool,
}

impl SearchBox {
    pub fn new(x: i32, y: i32) -> SearchBox {
        SearchBox { x, y, blink_on: true, ..SearchBox::default() }
    }

    /// `setValue`: cut to the length limit, cursor and highlight to the end.
    pub fn set_value(&mut self, font: &dyn Metrics, value: &str) {
        self.value = value.chars().take(MAX_LENGTH).collect();
        let end = self.value.len();
        self.set_cursor(font, end);
        self.set_highlight(font, end);
        self.set_highlight(font, end);
    }

    pub fn set_cursor(&mut self, font: &dyn Metrics, pos: usize) {
        self.cursor = pos.min(self.value.len());
        self.scroll_to(font, self.cursor);
    }

    pub fn set_highlight(&mut self, font: &dyn Metrics, pos: usize) {
        self.highlight = pos.min(self.value.len());
        self.scroll_to(font, self.highlight);
    }

    fn scroll_to(&mut self, font: &dyn Metrics, pos: usize) {
        let len = self.value.len() as i32;
        let mut display = (self.display as i32).min(len);
        let last = fit(font, &self.value[display as usize..], WIDTH) as i32 + display;
        let pos = pos as i32;
        if pos == display {
            display -= fit_tail(font, &self.value, WIDTH) as i32;
        }
        if pos > last {
            display += pos - last;
        } else if pos <= display {
            display = pos;
        }
        self.display = display.clamp(0, len) as usize;
    }

    /// `extractWidgetRenderState` of the visible, focused box.
    pub fn extract(&self, gui: &mut Gui, font: &dyn Metrics) {
        let rest = &self.value[self.display..];
        let displayed: String = rest[..fit(font, rest, WIDTH)].iter().collect();
        let shown = displayed.chars().count();
        let rel_cursor = self.cursor as i32 - self.display as i32;
        let on_screen = rel_cursor >= 0 && rel_cursor <= shown as i32;
        let show_cursor = self.blink_on && on_screen;
        let (text_x, text_y) = (self.x, self.y);
        let mut draw_x = text_x;
        let rel_highlight = (self.highlight as i32 - self.display as i32).clamp(0, shown as i32) as usize;
        let head = |n: usize| -> String { displayed.chars().take(n).collect() };
        if shown > 0 {
            let half = if on_screen { head(rel_cursor as usize) } else { displayed.clone() };
            let width = font.width(&Text::literal(half.clone()));
            gui.text(Text::literal(half), draw_x, text_y, COLOR, true);
            draw_x += width + 1;
        }
        let insert = self.cursor < self.value.len() || self.value.len() >= MAX_LENGTH;
        let mut cursor_x = draw_x;
        if !on_screen {
            cursor_x = if rel_cursor > 0 { text_x + WIDTH } else { text_x };
        } else if insert {
            cursor_x -= 1;
            draw_x -= 1;
        }
        if shown > 0 && on_screen && (rel_cursor as usize) < shown {
            let tail: String = displayed.chars().skip(rel_cursor as usize).collect();
            gui.text(Text::literal(tail), draw_x, text_y, COLOR, true);
        }
        if rel_highlight as i32 != rel_cursor {
            let highlight_x = text_x + font.width(&Text::literal(head(rel_highlight)));
            let right = self.x + WIDTH;
            gui.highlight(cursor_x.min(right), text_y - 1, (highlight_x - 1).min(right), text_y + 1 + 9, false);
        }
        if show_cursor {
            if insert {
                gui.fill(cursor_x, text_y - 1, cursor_x + 1, text_y + 10, COLOR);
            } else {
                gui.text(Text::literal("_"), cursor_x, text_y, COLOR, true);
            }
        }
    }
}
