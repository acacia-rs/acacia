//! `EditBox.extractWidgetRenderState`: the text either side of the caret, the grey completion,
//! the selection highlight and the blinking caret.

use super::edit_box::{EditBox, TEXT_COLOR};
use crate::java::gui::{Gui, Metrics};
use crate::java::split::plain_head_by_width;
use crate::java::text::{Style, Text};

const SUGGESTION_COLOR: u32 = 0xFF80_8080;

impl EditBox {
    /// Focused and editable; the caret blinks on for 300 ms of every 600 after `blink_ms` since
    /// focus. `format` styles a piece of the value starting at an offset (`TextFormatter`).
    pub fn extract(&self, gui: &mut Gui, m: &dyn Metrics, blink_ms: u64, format: &dyn Fn(&str, usize) -> Text) {
        let color = TEXT_COLOR;
        let (text_x, text_y) = (self.text_x(), self.text_y());
        let rel_cursor = self.cursor as isize - self.display as isize;
        let tail = self.slice(self.display, self.len());
        let displayed: Vec<u16> = plain_head_by_width(m, &tail, self.inner_width(), &Style::default()).encode_utf16().collect();
        let piece = |a: usize, b: usize| String::from_utf16_lossy(&displayed[a..b]);
        let on_screen = rel_cursor >= 0 && rel_cursor as usize <= displayed.len();
        let show_cursor = self.focused && (blink_ms / 300).is_multiple_of(2) && on_screen;
        let mut draw_x = text_x;
        let rel_highlight = (self.highlight as isize - self.display as isize).clamp(0, displayed.len() as isize) as usize;
        if !displayed.is_empty() {
            let half = if on_screen { piece(0, rel_cursor as usize) } else { piece(0, displayed.len()) };
            let text = format(&half, self.display);
            draw_x += m.width(&text) + 1;
            gui.text(text, text_x, text_y, color, true);
        }
        let insert = self.cursor < self.len() || self.len() >= self.max_length;
        let mut cursor_x = draw_x;
        if !on_screen {
            cursor_x = if rel_cursor > 0 { text_x + self.width } else { text_x };
        } else if insert {
            cursor_x -= 1;
            draw_x -= 1;
        }
        if !displayed.is_empty() && on_screen && (rel_cursor as usize) < displayed.len() {
            gui.text(format(&piece(rel_cursor as usize, displayed.len()), self.cursor), draw_x, text_y, color, true);
        }
        if let Some(suggestion) = self.suggestion.as_ref().filter(|_| !insert) {
            gui.text(Text::legacy(suggestion), cursor_x - 1, text_y, SUGGESTION_COLOR, true);
        }
        if rel_highlight as isize != rel_cursor {
            let highlight_x = text_x + m.width(&Text::literal(piece(0, rel_highlight)));
            let right = self.x + self.width;
            gui.text_highlight(cursor_x.min(right), text_y - 1, (highlight_x - 1).min(right), text_y + 1 + 9, true);
        }
        if show_cursor {
            if insert {
                gui.fill(cursor_x, text_y - 1, cursor_x + 1, text_y + 10, color);
            } else {
                gui.text("_", cursor_x, text_y, color, true);
            }
        }
    }
}
