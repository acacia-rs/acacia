//! `MultiLineEditBox`: a [`MultilineField`] in a [`TextArea`], scrolled to keep its cursor in view.

use super::multiline::MultilineField;
use super::text_area::{INNER_PADDING, TextArea};
use super::{Base, Ctx, Widget};
use crate::java::gui::Metrics;
use crate::java::text::Text;
use crate::java::widget::input::Key;
use crate::java::widget::measure::width_str;

#[derive(Clone, Debug)]
pub struct MultiLineEditBox {
    pub area: TextArea,
    pub field: MultilineField,
    pub text_color: u32,
    pub cursor_color: u32,
    pub shadow: bool,
    /// `Util.getMillis()` when it was last focused, for the caret blink.
    pub focused_time: i64,
}

impl MultiLineEditBox {
    /// `MultiLineEditBox.builder()...build(font, w, h, narration)` with the builder's defaults.
    pub fn new(m: &dyn Metrics, x: i32, y: i32, w: i32, h: i32) -> MultiLineEditBox {
        let field = MultilineField::new(m, w - INNER_PADDING * 2);
        MultiLineEditBox { area: TextArea::new(x, y, w, h, 4.0), field, text_color: 0xFFE0_E0E0, cursor_color: 0xFFD0_D0D0, shadow: true, focused_time: 0 }
    }

    fn inner_height(&self) -> i32 {
        9 * self.field.lines().len() as i32
    }

    /// `scrollToCursor`, the field's cursor listener.
    pub fn scroll_to_cursor(&mut self) {
        let f = &self.field;
        let h = self.area.base.h;
        let mut scroll = self.area.scroll;
        let line = f.line_at_cursor().map_or(-1, |l| l as i32);
        if f.cursor <= f.line_view((scroll / 9.0) as i32).0 {
            scroll = f64::from(line * 9);
        } else if f.cursor > f.line_view(((scroll + f64::from(h)) / 9.0) as i32 - 1).1 {
            scroll = f64::from(line * 9 - h + 9 + INNER_PADDING * 2);
        }
        let inner = self.inner_height();
        self.area.set_scroll(scroll, inner);
    }

    /// `setValue(value, allowOverflowLineLimit)`.
    pub fn set_value(&mut self, m: &dyn Metrics, value: &str, allow_overflow: bool) {
        if self.field.set_value(m, value, allow_overflow) {
            self.scroll_to_cursor();
        }
    }

    /// `seekCursor(ABSOLUTE, pos)` with the selection anchor kept when `selecting`.
    pub fn seek(&mut self, pos: usize, selecting: bool) {
        self.field.selecting = selecting;
        self.field.seek(pos);
        self.scroll_to_cursor();
    }

    /// `charTyped` while focused.
    pub fn char_typed(&mut self, m: &dyn Metrics, c: char) -> bool {
        if !(self.area.base.visible && self.area.base.focused && super::multiline::allowed(c)) {
            return false;
        }
        if self.field.insert_text(m, c.encode_utf8(&mut [0; 4])) {
            self.scroll_to_cursor();
        }
        true
    }

    /// `keyPressed`.
    pub fn key(&mut self, m: &dyn Metrics, key: Key, shift: bool) -> bool {
        match self.field.key_pressed(m, key, shift) {
            Some(listener) => {
                if listener {
                    self.scroll_to_cursor();
                }
                true
            }
            None => false,
        }
    }

    /// `onClick` (single) and `onDrag`: the cursor to the mouse.
    pub fn click(&mut self, m: &dyn Metrics, x: f64, y: f64, shift: bool, drag: bool) {
        let a = &self.area;
        let (px, py) = (x - f64::from(a.base.x + INNER_PADDING), y - f64::from(a.base.y + INNER_PADDING) + a.scroll);
        self.field.selecting = drag || shift;
        self.field.seek_point(m, px, py);
        self.scroll_to_cursor();
        self.field.selecting = shift;
    }

    fn contents(&self, ctx: &mut Ctx) {
        let (f, a) = (&self.field, &self.area);
        let value = &f.value;
        let m = ctx.font;
        let focused = a.base.focused;
        // The unfocused empty box shows its placeholder: the book's is empty and draws nothing.
        if value.is_empty() && !focused {
            return;
        }
        let show_cursor = focused && (ctx.millis - self.focused_time) / 300 % 2 == 0;
        let needs = a.base.visible && focused;
        let insert = f.cursor < value.len();
        let (left, mut top) = (a.inner_left(), a.inner_top());
        let (mut cursor_x, mut cursor_y) = (0, 0);
        let mut drawn = false;
        let text = |ctx: &mut Ctx, s: &str, x: i32, y: i32| ctx.gui.text(Text::literal(s), x, y, self.text_color, self.shadow);
        for &(b, e) in f.lines() {
            let visible = a.within(top, top + 9);
            if !drawn && (needs || show_cursor) && insert && f.cursor >= b && f.cursor <= e {
                let before = &value[b..f.cursor];
                cursor_x = left + width_str(m, before);
                cursor_y = top;
                if visible {
                    text(ctx, before, left, top);
                    text(ctx, &value[f.cursor..e], cursor_x, top);
                    if show_cursor {
                        ctx.gui.fill(cursor_x, cursor_y - 1, cursor_x + 1, cursor_y + 10, self.cursor_color);
                    }
                }
                drawn = true;
            } else {
                let line = &value[b..e];
                if visible {
                    text(ctx, line, left, top);
                }
                if (needs || show_cursor) && !insert {
                    cursor_x = left + width_str(m, line);
                    cursor_y = top;
                }
            }
            top += 9;
        }
        if show_cursor && !insert && a.within(cursor_y, cursor_y + 9) {
            ctx.gui.text(Text::literal("_"), cursor_x, cursor_y, self.cursor_color, self.shadow);
        }
        if f.has_selection() {
            let (sb, se) = f.selected();
            let mut top = a.inner_top();
            for &(b, e) in f.lines() {
                if sb > e {
                    top += 9;
                    continue;
                }
                if b > se {
                    break;
                }
                if a.within(top, top + 9) {
                    let begin = width_str(m, &value[b..sb.max(b)]);
                    let end = if se > e { a.base.w - INNER_PADDING } else { width_str(m, &value[b..se]) };
                    ctx.gui.highlight(left + begin, top, left + end, top + 9, true);
                }
                top += 9;
            }
        }
    }
}

impl Widget for MultiLineEditBox {
    fn base(&self) -> &Base {
        &self.area.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.area.base
    }
    fn extract(&mut self, ctx: &mut Ctx, _hovered: bool) {
        let inner = self.inner_height();
        let this = &*self;
        this.area.extract(ctx, inner, |ctx| this.contents(ctx), |ctx| {
            if let Some(limit) = this.field.char_limit {
                let b = &this.area.base;
                let count = Text::literal(this.field.value.chars().count().to_string());
                let t = ctx.tr_args("gui.multiLineEditBox.character_limit", &[count, Text::literal(limit.to_string())]);
                let x = b.x + b.w - ctx.width(&t);
                ctx.gui.text(t, x, b.y + b.h + 4, 0xFFA0_A0A0, true);
            }
        });
    }
}
