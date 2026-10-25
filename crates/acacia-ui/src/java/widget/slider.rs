//! `AbstractSliderButton`: the track, the handle at `value` and the label over them.

use super::{Base, Ctx, Widget};
use crate::java::mth::white;
use crate::java::text::Text;

#[derive(Clone, Debug)]
pub struct Slider {
    pub base: Base,
    /// 0 to 1.
    pub value: f64,
    /// `canChangeValue`: set by focusing with the mouse or Tab, toggled by Enter.
    pub can_change: bool,
}

impl Slider {
    pub fn new(x: i32, y: i32, w: i32, h: i32, message: Text, value: f64) -> Slider {
        Slider { base: Base::new(x, y, w, h, message), value, can_change: false }
    }
}

impl Widget for Slider {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn extract(&mut self, ctx: &mut Ctx, hovered: bool) {
        let b = &self.base;
        let track = if b.is_active() && b.focused && !self.can_change { "widget/slider_highlighted" } else { "widget/slider" };
        let handle = if !b.is_active() || (!hovered && !self.can_change) { "widget/slider_handle" } else { "widget/slider_handle_highlighted" };
        ctx.gui.sprite_tinted(track, b.x, b.y, b.w, b.h, white(b.alpha));
        ctx.gui.sprite_tinted(handle, b.x + (self.value * f64::from(b.w - 8)) as i32, b.y, 8, b.h, white(b.alpha));
        ctx.scrolling_over(b, b.shown_message(), 2, b.alpha);
    }
}
