//! `AbstractSliderButton`: the track, the handle at `value` and the label over them.

use super::input::{Action, Key, KeyOutcome};
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

    /// `setValue`: clamped to the track.
    fn set_value(&mut self, v: f64) -> Action {
        self.value = v.clamp(0.0, 1.0);
        Action::Slide(self.value)
    }

    /// `setValueFromMouse`.
    fn set_value_from_mouse(&mut self, x: f64) -> Action {
        self.set_value((x - f64::from(self.base.x + 4)) / f64::from(self.base.w - 8))
    }
}

impl Widget for Slider {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn on_click(&mut self, x: f64, _: bool) -> Option<Action> {
        Some(self.set_value_from_mouse(x))
    }
    fn on_drag(&mut self, x: f64) -> Option<Action> {
        Some(self.set_value_from_mouse(x))
    }
    /// Enter or Space toggles `canChangeValue`; then Left and Right step one pixel of the track.
    fn key_pressed(&mut self, key: Key, _: bool) -> KeyOutcome {
        if key.is_selection() {
            self.can_change = !self.can_change;
            return Some(None);
        }
        let step = match key {
            Key::Left => -1.0,
            Key::Right => 1.0,
            _ => return None,
        };
        self.can_change.then(|| Some(self.set_value(self.value + step / f64::from(self.base.w - 8))))
    }
    fn set_focused(&mut self, focused: bool, pointer: bool) {
        self.base.focused = focused;
        if !focused {
            self.can_change = false;
        } else if pointer {
            self.can_change = true;
        }
    }
    fn can_change(&self) -> bool {
        self.can_change
    }
    fn set_can_change(&mut self, on: bool) {
        self.can_change = on;
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
