//! `AbstractTextAreaWidget` over `AbstractScrollArea`: a text-field frame, the contents scissored
//! and scrolled inside a 4-pixel padding, and the scroll bar just right of the frame.

use super::{Base, Ctx};
use crate::java::gui::PoseOp;

pub const INNER_PADDING: i32 = 4;
const SCROLLBAR_WIDTH: i32 = 6;

#[derive(Clone, Debug)]
pub struct TextArea {
    pub base: Base,
    pub scroll: f64,
    /// `ScrollbarSettings.scrollRate`.
    pub scroll_rate: f64,
    pub show_background: bool,
    pub show_decorations: bool,
}

impl TextArea {
    pub fn new(x: i32, y: i32, w: i32, h: i32, scroll_rate: f64) -> TextArea {
        TextArea { base: Base::new(x, y, w, h, Default::default()), scroll: 0.0, scroll_rate, show_background: true, show_decorations: true }
    }

    pub fn inner_left(&self) -> i32 {
        self.base.x + INNER_PADDING
    }

    pub fn inner_top(&self) -> i32 {
        self.base.y + INNER_PADDING
    }

    /// `contentHeight`: the inner height and the padding.
    fn content_height(inner_height: i32) -> i32 {
        inner_height + INNER_PADDING * 2
    }

    pub fn max_scroll(&self, inner_height: i32) -> i32 {
        (Self::content_height(inner_height) - self.base.h).max(0)
    }

    /// `setScrollAmount`: clamped to what the content allows.
    pub fn set_scroll(&mut self, v: f64, inner_height: i32) {
        self.scroll = v.clamp(0.0, f64::from(self.max_scroll(inner_height)));
    }

    /// `withinContentAreaTopBottom`.
    pub fn within(&self, top: i32, bottom: i32) -> bool {
        f64::from(bottom) - self.scroll >= f64::from(self.base.y) && f64::from(top) - self.scroll <= f64::from(self.base.y + self.base.h)
    }

    fn scroller_height(&self, inner_height: i32) -> i32 {
        let h = self.base.h;
        // Mth.clamp: max then min.
        (((h * h) as f32 / Self::content_height(inner_height) as f32) as i32).max(32).min(h - 8)
    }

    /// `extractWidgetRenderState`: frame, scissored contents under the scroll, scroll bar;
    /// `decorations` runs after when they are shown.
    pub fn extract(&self, ctx: &mut Ctx, inner_height: i32, contents: impl FnOnce(&mut Ctx), decorations: impl FnOnce(&mut Ctx)) {
        let b = &self.base;
        if !b.visible {
            return;
        }
        if self.show_background {
            let sprite = if b.focused { "widget/text_field_highlighted" } else { "widget/text_field" };
            ctx.gui.sprite(sprite, b.x, b.y, b.w, b.h);
        }
        ctx.enable_scissor(b.x + 1, b.y + 1, b.x + b.w - 1, b.y + b.h - 1);
        ctx.push(vec![PoseOp::Translate(0.0, -self.scroll as f32)]);
        contents(ctx);
        ctx.pop();
        ctx.disable_scissor();
        let max = self.max_scroll(inner_height);
        if max > 0 {
            let x = b.x + b.w;
            let sh = self.scroller_height(inner_height);
            let y = b.y.max(self.scroll as i32 * (b.h - sh) / max + b.y);
            ctx.gui.sprite("widget/scroller_background", x, b.y, SCROLLBAR_WIDTH, b.h);
            ctx.gui.sprite("widget/scroller", x, y, SCROLLBAR_WIDTH, sh);
        }
        if self.show_decorations {
            decorations(ctx);
        }
    }

    /// `isMouseOver`: the frame and its scroll bar.
    pub fn over(&self, x: f64, y: f64) -> bool {
        let b = &self.base;
        b.is_active() && x >= f64::from(b.x) && y >= f64::from(b.y) && x < f64::from(b.x + b.w + SCROLLBAR_WIDTH) && y < f64::from(b.y + b.h)
    }
}
