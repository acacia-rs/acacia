//! `EditBox`: a one-line text field with its cursor, selection, scroll position, hint and
//! suggestion. Positions are in characters (the game's are UTF-16 units: the same for the BMP).

use super::measure::{plain_head, plain_tail, width_str};
use super::{Base, Ctx, Widget};
use crate::java::gui::Metrics;
use crate::java::text::{Style, Text};

pub const TEXT_COLOR: u32 = 0xFFE0_E0E0;
pub const TEXT_COLOR_UNEDITABLE: u32 = 0xFF70_7070;
const SUGGESTION_COLOR: u32 = 0xFF80_8080;

#[derive(Clone, Debug)]
pub struct EditBox {
    pub base: Base,
    pub value: String,
    pub max_length: usize,
    pub bordered: bool,
    pub editable: bool,
    pub centered: bool,
    pub shadow: bool,
    pub invert_highlight: bool,
    pub display_pos: usize,
    pub cursor: usize,
    pub highlight: usize,
    pub text_color: u32,
    pub text_color_uneditable: u32,
    pub suggestion: Option<String>,
    pub hint: Option<Text>,
    /// `Util.getMillis()` when it was last focused, for the caret blink.
    pub focused_time: i64,
    text_offset: (i32, i32),
}

fn len(s: &str) -> usize {
    s.chars().count()
}

fn sub(s: &str, from: usize, to: usize) -> String {
    s.chars().skip(from).take(to.saturating_sub(from)).collect()
}

impl EditBox {
    pub fn new(x: i32, y: i32, w: i32, h: i32, narration: Text) -> EditBox {
        EditBox {
            base: Base::new(x, y, w, h, narration),
            value: String::new(),
            max_length: 32,
            bordered: true,
            editable: true,
            centered: false,
            shadow: true,
            invert_highlight: true,
            display_pos: 0,
            cursor: 0,
            highlight: 0,
            text_color: TEXT_COLOR,
            text_color_uneditable: TEXT_COLOR_UNEDITABLE,
            suggestion: None,
            hint: None,
            focused_time: 0,
            text_offset: (4, (h - 8) / 2),
        }
    }

    pub fn inner_width(&self) -> i32 {
        if self.bordered { self.base.w - 8 } else { self.base.w }
    }

    /// `setHint`: an unstyled hint turns dark grey.
    pub fn set_hint(&mut self, hint: Text) {
        let plain = hint.spans.iter().all(|s| s.style == Style::default());
        self.hint = Some(if plain { hint.with_color(0x55_5555) } else { hint });
    }

    /// `setValue`: cut to `max_length`, cursor and selection at the end.
    pub fn set_value(&mut self, m: &dyn Metrics, value: &str) {
        self.value = value.chars().take(self.max_length).collect();
        self.move_cursor_to(m, len(&self.value), false);
    }

    /// `moveCursorTo`.
    pub fn move_cursor_to(&mut self, m: &dyn Metrics, pos: usize, extend: bool) {
        self.set_cursor_position(m, pos);
        if !extend {
            self.set_highlight_pos(m, self.cursor);
        }
        self.update_text_position(m);
    }

    pub fn set_cursor_position(&mut self, m: &dyn Metrics, pos: usize) {
        self.cursor = pos.min(len(&self.value));
        self.scroll_to(m, self.cursor);
    }

    pub fn set_highlight_pos(&mut self, m: &dyn Metrics, pos: usize) {
        self.highlight = pos.min(len(&self.value));
        self.scroll_to(m, self.highlight);
    }

    /// `scrollTo`: keeps `pos` on screen.
    fn scroll_to(&mut self, m: &dyn Metrics, pos: usize) {
        let n = len(&self.value);
        self.display_pos = self.display_pos.min(n);
        let inner = self.inner_width();
        let shown = plain_head(m, &sub(&self.value, self.display_pos, n), inner);
        let last = len(&shown) + self.display_pos;
        let mut display = self.display_pos as i64;
        if pos == self.display_pos {
            display -= len(&plain_tail(m, &self.value, inner)) as i64;
        }
        if pos > last {
            display += (pos - last) as i64;
        } else if pos as i64 <= display {
            display = pos as i64;
        }
        self.display_pos = display.clamp(0, n as i64) as usize;
    }

    /// `updateTextPosition`. Java caches the text origin and refreshes it only on `setValue`,
    /// `moveCursorTo`, `setX/Y`, `setBordered` and `setCentered`: a later `setHighlightPos` that
    /// scrolls leaves a centred field's text where it was. Kept relative to the box.
    pub fn update_text_position(&mut self, m: &dyn Metrics) {
        let b = &self.base;
        let shown = plain_head(m, &sub(&self.value, self.display_pos, len(&self.value)), self.inner_width());
        let dx = if self.centered { (b.w - width_str(m, &shown)) / 2 } else if self.bordered { 4 } else { 0 };
        let dy = if self.bordered { (b.h - 8) / 2 } else { 0 };
        self.text_offset = (dx, dy);
    }

    fn text_pos(&self) -> (i32, i32) {
        (self.base.x + self.text_offset.0, self.base.y + self.text_offset.1)
    }
}

impl Widget for EditBox {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn extract(&mut self, ctx: &mut Ctx, _hovered: bool) {
        let b = self.base.clone();
        let m = ctx.font;
        if self.bordered {
            // WidgetSprites(sprite, focused): focus picks the sprite whether active or not.
            let sprite = if b.focused { "widget/text_field_highlighted" } else { "widget/text_field" };
            ctx.gui.sprite(sprite, b.x, b.y, b.w, b.h);
        }
        let (text_x, text_y) = self.text_pos();
        let color = if self.editable { self.text_color } else { self.text_color_uneditable };
        let rel_cursor = self.cursor as i64 - self.display_pos as i64;
        let shown = plain_head(m, &sub(&self.value, self.display_pos, len(&self.value)), self.inner_width());
        let shown_len = len(&shown) as i64;
        let on_screen = rel_cursor >= 0 && rel_cursor <= shown_len;
        let blink = (ctx.millis - self.focused_time) / 300 % 2 == 0;
        let show_cursor = b.focused && blink && on_screen;
        let mut draw_x = text_x;
        let rel_highlight = (self.highlight as i64 - self.display_pos as i64).clamp(0, shown_len) as usize;
        if !shown.is_empty() {
            let half = if on_screen { sub(&shown, 0, rel_cursor as usize) } else { shown.clone() };
            let w = width_str(m, &half);
            ctx.gui.text(Text::literal(half), draw_x, text_y, color, self.shadow);
            draw_x += w + 1;
        }
        let insert = self.cursor < len(&self.value) || len(&self.value) >= self.max_length;
        let mut cursor_x = draw_x;
        if !on_screen {
            cursor_x = if rel_cursor > 0 { text_x + b.w } else { text_x };
        } else if insert {
            cursor_x -= 1;
            draw_x -= 1;
        }
        if !shown.is_empty() && on_screen && rel_cursor < shown_len {
            ctx.gui.text(Text::literal(sub(&shown, rel_cursor as usize, len(&shown))), draw_x, text_y, color, self.shadow);
        }
        if let Some(hint) = &self.hint
            && shown.is_empty()
            && !b.focused
        {
            ctx.gui.text(hint.clone(), draw_x, text_y, color, true);
        }
        if !insert && let Some(s) = &self.suggestion {
            ctx.gui.text(Text::literal(s.clone()), cursor_x - 1, text_y, SUGGESTION_COLOR, self.shadow);
        }
        if rel_highlight as i64 != rel_cursor {
            let highlight_x = text_x + width_str(m, &sub(&shown, 0, rel_highlight));
            let right = b.x + b.w;
            ctx.gui.highlight(cursor_x.min(right), text_y - 1, (highlight_x - 1).min(right), text_y + 1 + 9, self.invert_highlight);
        }
        if show_cursor {
            if insert {
                ctx.gui.fill(cursor_x, text_y - 1, cursor_x + 1, text_y + 10, color);
            } else {
                ctx.gui.text(Text::literal("_"), cursor_x, text_y, color, self.shadow);
            }
        }
    }
}
