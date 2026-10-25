//! The message screens: `AlertScreen`, `ProgressScreen`, `DisconnectedScreen` and
//! `GenericWaitingScreen.createCompleted` (its loading-dots variants follow the wall clock).

use super::{Env, Screen, menu_background, render_all};
use crate::java::text::{Style, Text};
use crate::java::widget::Ctx;
use crate::java::widget::button::Button;
use crate::java::widget::layout::{Arena, Elem, Linear, align_in_rect};
use crate::java::widget::text::{MultiLineText, MultiLineTextWidget, StringWidget, label_lines};

/// Lines of `MultiLineLabel.create(font, message, maxWidth)` drawn centred on `x` from `y`.
fn centered_lines(ctx: &mut Ctx, message: &Text, max_width: i32, x: i32, y: i32) {
    for (i, (line, _)) in label_lines(ctx.font, message, max_width, usize::MAX, &Style::default()).into_iter().enumerate() {
        ctx.label_centered(line, x, y + 9 * i as i32, 1.0);
    }
}

fn line_count(ctx: &Ctx, message: &Text, max_width: i32) -> i32 {
    label_lines(ctx.font, message, max_width, usize::MAX, &Style::default()).len() as i32
}

/// A screen of buttons and text drawn over the menu background.
pub struct MessageScreen {
    pub env: Env,
    widgets: Arena,
    texts: Vec<Line>,
}

/// Text drawn after the widgets.
enum Line {
    /// `centeredText(font, text, x, y, -1)`.
    Centered(Text, i32, i32),
    /// A wrapped message's lines centred on x from y.
    Wrapped(Text, i32, i32, i32),
}

impl MessageScreen {
    /// `new AlertScreen(callback, title, message, okButton, closeOnEsc)` after `init`.
    pub fn alert(ctx: &Ctx, env: Env, title: Text, message: Text, ok: Text) -> MessageScreen {
        let lines = line_count(ctx, &message, env.width - 50);
        let y = (90 + lines * 9 + 12).max(env.height / 6 + 96).min(env.height - 24);
        let widgets: Arena = vec![Box::new(Button::new((env.width - 150) / 2, y, 150, 20, ok))];
        let texts = vec![Line::Centered(title, env.width / 2, 70), Line::Wrapped(message, env.width - 50, env.width / 2, 90)];
        MessageScreen { env, widgets, texts }
    }

    /// `ProgressScreen` after `progressStart(header)`, `progressStage(stage)` and
    /// `progressStagePercentage(progress)`.
    pub fn progress(env: Env, header: Option<Text>, stage: Option<Text>, progress: i32) -> MessageScreen {
        let mut texts = Vec::new();
        if let Some(h) = header {
            texts.push(Line::Centered(h, env.width / 2, 70));
        }
        if let Some(s) = stage.filter(|_| progress != 0) {
            texts.push(Line::Centered(s.append(Text::literal(format!(" {progress}%"))), env.width / 2, 90));
        }
        MessageScreen { env, widgets: Vec::new(), texts }
    }

    /// `GenericWaitingScreen.createCompleted(title, message, button, callback)` after `ticks` ticks.
    pub fn completed(env: Env, ctx: &Ctx, title: Text, message: Text, button: Text, ticks: u32) -> MessageScreen {
        let lines = line_count(ctx, &message, 360) + 1;
        let y = (120 + lines.max(5) * 9).min(env.height - 40);
        let mut b = Button::new((env.width - 150) / 2, y, 150, 20, button);
        b.base.active = ticks == 0 || ticks >= 20;
        let texts = vec![Line::Centered(title, env.width / 2, 80), Line::Wrapped(message, 360, env.width / 2, 120)];
        MessageScreen { env, widgets: vec![Box::new(b)], texts }
    }

    /// `new DisconnectedScreen(parent, title, reason)` with no report or bug link, on a client
    /// that does not allow multiplayer (the back button goes to the title screen).
    pub fn disconnected(ctx: &Ctx, env: Env, title: Text, reason: Text, reason_style: Style) -> MessageScreen {
        let m = ctx.font;
        let mut widgets: Arena = vec![Box::new(StringWidget::fit(m, title))];
        let mut layout = Linear::new(true, 0);
        layout.grid.defaults = layout.grid.defaults.align_x(0.5).padding(10, 10, 10, 10);
        layout.add_default(Elem::Widget(0));
        let mut text = MultiLineText::new(reason);
        text.max_width = Some(env.width - 50);
        text.centered = true;
        text.ellipsis = reason_style;
        widgets.push(Box::new(MultiLineTextWidget::new(m, text)));
        layout.add_default(Elem::Widget(1));
        layout.grid.defaults = layout.grid.defaults.padding(2, 2, 2, 2);
        widgets.push(Box::new(Button::sized(200, ctx.tr("gui.toTitle"))));
        layout.add_default(Elem::Widget(2));
        let mut root = layout.into_elem();
        if let Elem::Layout(l) = &mut root {
            l.arrange(&mut widgets);
        }
        align_in_rect(&mut root, &mut widgets, 0, 0, env.width, env.height, 0.5, 0.5);
        MessageScreen { env, widgets, texts: Vec::new() }
    }
}

impl Screen for MessageScreen {
    fn background(&mut self, ctx: &mut Ctx) {
        menu_background(&self.env, ctx);
    }

    fn foreground(&mut self, ctx: &mut Ctx) {
        render_all(&mut self.widgets, ctx);
        for line in &self.texts {
            match line {
                Line::Centered(t, x, y) => ctx.centered_text(t.clone(), *x, *y, 0xFFFF_FFFF),
                Line::Wrapped(t, max, x, y) => centered_lines(ctx, t, *max, *x, *y),
            }
        }
    }
}
