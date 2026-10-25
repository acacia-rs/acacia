//! `ConfirmScreen` (and `DeathScreen.TitleConfirmScreen`, which only swaps the background): title,
//! wrapped message and a yes/no row stacked and centred.

use super::{Env, Escape, Screen, arena_children, menu_background, render_all};
use crate::java::widget::input::{Action, Child, Fired};
use crate::java::text::{Style, Text};
use crate::java::widget::Ctx;
use crate::java::widget::button::Button;
use crate::java::widget::layout::{Arena, Elem, Linear, align_in_rect};
use crate::java::widget::text::{MultiLineText, MultiLineTextWidget, StringWidget};

/// The buttons' ids: the callback's true and false.
pub const YES: &str = "yes";
pub const NO: &str = "no";

pub struct ConfirmScreen {
    pub env: Env,
    /// `DeathScreen.TitleConfirmScreen`: the death screen's wash behind it.
    pub death_background: bool,
    widgets: Arena,
}

pub struct Confirm {
    pub title: Text,
    pub message: Text,
    /// The message's own style, which a cut-off last line's ellipsis takes.
    pub message_style: Style,
    pub yes: Text,
    pub no: Text,
    /// `setDelay(delay)` then `ticks` ticks: the buttons are off until the delay counts down to
    /// exactly 0 (so never after `setDelay(0)`).
    pub delay: Option<u32>,
    pub ticks: u32,
}

impl ConfirmScreen {
    /// `init()` at the env's size.
    pub fn new(ctx: &Ctx, env: Env, c: Confirm) -> ConfirmScreen {
        let m = ctx.font;
        let mut widgets: Arena = Vec::new();
        let mut layout = Linear::new(true, 8);
        layout.grid.defaults = layout.grid.defaults.align_x(0.5);
        widgets.push(Box::new(StringWidget::fit(m, c.title)));
        layout.add_default(Elem::Widget(0));
        let mut text = MultiLineText::new(c.message);
        text.max_width = Some(env.width - 50);
        text.max_rows = Some(15);
        text.centered = true;
        text.ellipsis = c.message_style;
        widgets.push(Box::new(MultiLineTextWidget::new(m, text)));
        layout.add_default(Elem::Widget(1));
        let mut buttons = Linear::new(false, 4);
        buttons.grid.defaults = buttons.grid.defaults.pad_top(16);
        for (label, id) in [(c.yes, YES), (c.no, NO)] {
            let mut b = Button::sized(150, label);
            b.base.id = id.into();
            b.base.active = c.delay.is_none_or(|d| d > 0 && c.ticks >= d);
            widgets.push(Box::new(b));
            buttons.add_default(Elem::Widget(widgets.len() - 1));
        }
        layout.add_default(buttons.into_elem());
        let mut root = layout.into_elem();
        if let Elem::Layout(l) = &mut root {
            l.arrange(&mut widgets);
        }
        align_in_rect(&mut root, &mut widgets, 0, 0, env.width, env.height, 0.5, 0.5);
        ConfirmScreen { env, death_background: false, widgets }
    }
}

impl Screen for ConfirmScreen {
    fn background(&mut self, ctx: &mut Ctx) {
        if self.death_background {
            super::death::death_background(&self.env, ctx);
        } else {
            menu_background(&self.env, ctx);
        }
    }

    fn foreground(&mut self, ctx: &mut Ctx) {
        render_all(&mut self.widgets, ctx);
    }

    fn children(&mut self) -> Vec<Child<'_>> {
        arena_children(&mut self.widgets)
    }

    /// `ConfirmScreen.keyPressed`: no, once the delay is over (the buttons are on).
    fn escape(&self) -> Escape {
        match self.widgets.last() {
            Some(no) if no.base().active => Escape::Fire(Fired { id: NO.into(), action: Action::Press }),
            _ => Escape::Ignore,
        }
    }
}
