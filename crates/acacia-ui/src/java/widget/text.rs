//! The text widgets: `StringWidget`, `MultiLineTextWidget` (over `MultiLineLabel`) and
//! `FocusableTextWidget`.

use super::measure::{self, ELLIPSIS};
use super::{Base, Ctx, Widget};
use crate::java::gui::{Gui, Metrics};
use crate::java::mth::{as8, with_alpha, white};
use crate::java::text::{Style, Text};

/// `StringWidget.TextOverflow`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overflow {
    Clamped,
    Scrolling,
}

/// `StringWidget`: one line, clipped or scrolled when wider than its maximum.
#[derive(Clone, Debug)]
pub struct StringWidget {
    pub base: Base,
    pub max_width: i32,
    pub overflow: Overflow,
    text_width: i32,
}

impl StringWidget {
    /// `new StringWidget(x, y, w, h, message, font)`.
    pub fn new(m: &dyn Metrics, x: i32, y: i32, w: i32, h: i32, message: Text) -> StringWidget {
        let mut base = Base::new(x, y, w, h, message);
        base.active = false;
        let text_width = m.width(&base.message);
        StringWidget { base, max_width: 0, overflow: Overflow::Clamped, text_width }
    }

    /// `new StringWidget(message, font)`: as wide as the text, 9 high.
    pub fn fit(m: &dyn Metrics, message: Text) -> StringWidget {
        let w = m.width(&message);
        StringWidget::new(m, 0, 0, w, 9, message)
    }

    pub fn with_max_width(mut self, max: i32, overflow: Overflow) -> StringWidget {
        self.max_width = max;
        self.overflow = overflow;
        self
    }
}

impl Widget for StringWidget {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn width(&self) -> i32 {
        if self.max_width > 0 { self.max_width.min(self.text_width) } else { self.base.w }
    }
    fn extract(&mut self, ctx: &mut Ctx, _hovered: bool) {
        let b = self.base.clone();
        let max = if self.max_width > 0 { self.max_width } else { self.width() };
        let y = b.y + (b.h - 9) / 2;
        if self.text_width <= max {
            ctx.label(b.message, b.x, y, b.alpha);
        } else if self.overflow == Overflow::Clamped {
            let clipped = measure::clip(ctx.font, &b.message, max);
            ctx.label(clipped, b.x, y, b.alpha);
        } else {
            ctx.scrolling_over(&Base { w: self.width(), ..b.clone() }, b.message, 2, b.alpha);
        }
    }
}

/// `MultiLineLabel.create(font, maxWidth, maxRows, message)`: the lines and their widths; when rows
/// were cut the last kept line ends in "..." in `ellipsis` (the message's own style).
pub fn label_lines(m: &dyn Metrics, message: &Text, max_width: i32, max_rows: usize, ellipsis: &Style) -> Vec<(Text, i32)> {
    let split = measure::split(m, message, max_width);
    let total = split.len();
    let n = total.min(max_rows);
    split
        .into_iter()
        .take(n)
        .enumerate()
        .map(|(i, line)| {
            if i + 1 == n && n == max_rows && n != total {
                let clipped = measure::head(m, &line, m.width(&line) - measure::width_str(m, ELLIPSIS));
                let with = clipped.append(Text::styled(ELLIPSIS, ellipsis));
                let w = m.width(&with);
                (with, w)
            } else {
                let w = m.width(&line);
                (line, w)
            }
        })
        .collect()
}

/// `MultiLineTextWidget` without its `AbstractWidget` part: lines wrapped to `max_width`, at most
/// `max_rows` of them, left-aligned or centred.
#[derive(Clone, Debug)]
pub struct MultiLineText {
    pub message: Text,
    pub max_width: Option<i32>,
    pub max_rows: Option<usize>,
    pub centered: bool,
    pub x: i32,
    pub y: i32,
    /// The message's own `getStyle()`, which the ellipsis takes (spans don't keep it).
    pub ellipsis: Style,
}

impl MultiLineText {
    pub fn new(message: Text) -> MultiLineText {
        MultiLineText { message, max_width: None, max_rows: None, centered: false, x: 0, y: 0, ellipsis: Style::default() }
    }

    pub fn lines(&self, m: &dyn Metrics) -> Vec<(Text, i32)> {
        label_lines(m, &self.message, self.max_width.unwrap_or(i32::MAX), self.max_rows.unwrap_or(usize::MAX), &self.ellipsis)
    }

    /// `getWidth`: the widest line, at most `max_width`.
    pub fn width(&self, m: &dyn Metrics) -> i32 {
        self.lines(m).iter().map(|l| l.1).max().unwrap_or(0).min(self.max_width.unwrap_or(i32::MAX))
    }

    pub fn height(&self, m: &dyn Metrics) -> i32 {
        self.lines(m).len() as i32 * 9
    }

    /// `visitLines`: from (`x`, `y`), or centred on `widget_x + width / 2` (the widget's own box).
    pub fn visit_lines_at(&self, ctx: &mut Ctx, x: i32, widget_x: i32, y: i32, width: i32, opacity: f32) {
        let mid = widget_x + width / 2;
        for (i, (line, w)) in self.lines(ctx.font).into_iter().enumerate() {
            let left = if self.centered { mid - w / 2 } else { x };
            ctx.label(line, left, y + 9 * i as i32, opacity);
        }
    }

    pub fn visit_lines(&self, ctx: &mut Ctx, opacity: f32) {
        let w = self.width(ctx.font);
        self.visit_lines_at(ctx, self.x, self.x, self.y, w, opacity);
    }
}

/// `MultiLineTextWidget`.
#[derive(Clone, Debug)]
pub struct MultiLineTextWidget {
    pub base: Base,
    pub text: MultiLineText,
    size: [i32; 2],
}

impl MultiLineTextWidget {
    pub fn new(m: &dyn Metrics, text: MultiLineText) -> MultiLineTextWidget {
        let mut base = Base::new(text.x, text.y, 0, 0, text.message.clone());
        base.active = false;
        let size = [text.width(m), text.height(m)];
        MultiLineTextWidget { base, text, size }
    }
}

impl Widget for MultiLineTextWidget {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn width(&self) -> i32 {
        self.size[0]
    }
    fn height(&self) -> i32 {
        self.size[1]
    }
    fn extract(&mut self, ctx: &mut Ctx, _hovered: bool) {
        self.text.visit_lines_at(ctx, self.base.x, self.base.x, self.base.y, self.size[0], self.base.alpha);
    }
}

/// `FocusableTextWidget.BackgroundFill`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fill {
    Always,
    OnFocus,
    Never,
}

/// `FocusableTextWidget`: centred wrapped text in a padded box, outlined and filled by focus.
#[derive(Clone, Debug)]
pub struct FocusableTextWidget {
    pub base: Base,
    pub text: MultiLineText,
    pub padding: i32,
    pub always_border: bool,
    pub fill: Fill,
}

impl FocusableTextWidget {
    /// `FocusableTextWidget.builder(message, font, padding).maxWidth(max)...build()`; `max` -1 for none.
    pub fn new(m: &dyn Metrics, message: Text, padding: i32, max: i32, always_border: bool, fill: Fill) -> FocusableTextWidget {
        let mut text = MultiLineText::new(message.clone());
        text.centered = true;
        let w = if max != -1 {
            text.max_width = Some(max - padding * 2);
            max
        } else {
            m.width(&message) + padding * 2
        };
        let h = 9 * measure::split(m, &message, text.width(m)).len() as i32 + padding * 2;
        FocusableTextWidget { base: Base::new(0, 0, w, h, message), text, padding, always_border, fill }
    }
}

impl Widget for FocusableTextWidget {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn extract(&mut self, ctx: &mut Ctx, _hovered: bool) {
        let b = &self.base;
        let border = if self.always_border && !b.focused { with_alpha(as8(b.alpha), 0xA0A0A0) } else { white(b.alpha) };
        if self.fill == Fill::Always || (self.fill == Fill::OnFocus && b.focused) {
            ctx.gui.fill(b.x + 1, b.y, b.x + b.w, b.y + b.h, with_alpha(as8(b.alpha), 0));
        }
        if b.focused || self.always_border {
            outline(ctx.gui, b.x, b.y, b.w, b.h, border);
        }
        self.text.visit_lines_at(ctx, b.x + self.padding, b.x, b.y + self.padding, b.w, b.alpha);
    }
}

/// `GuiGraphicsExtractor.outline`.
pub fn outline(gui: &mut Gui, x: i32, y: i32, w: i32, h: i32, color: u32) {
    gui.fill(x, y, x + w, y + 1, color);
    gui.fill(x, y + h - 1, x + w, y + h, color);
    gui.fill(x, y + 1, x + 1, y + h - 1, color);
    gui.fill(x + w - 1, y + 1, x + w, y + h - 1, color);
}
