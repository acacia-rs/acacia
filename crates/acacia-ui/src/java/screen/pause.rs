//! `PauseScreen`: the game menu's button grid (or only the "Game Paused" title when the menu is
//! hidden) over the in-world menu background.

use super::{Env, Screen, blurred, menu_texture, render_all};
use crate::java::widget::button::{Button, SpriteIconButton, Sprites};
use crate::java::widget::layout::{Arena, Elem, Layout, Linear, RowHelper, Settings, align_in_rect};
use crate::java::widget::text::StringWidget;
use crate::java::widget::{Ctx, Widget};

/// What the pause screen reads from the client.
#[derive(Clone, Copy, Debug)]
pub struct Pause {
    /// `showPauseMenu`: false for F3+Esc, which shows only the title.
    pub show_menu: bool,
    /// `minecraft.gui.screen() == this`: only the topmost pause screen blurs.
    pub topmost: bool,
    /// `isLocalServer()`: "Save and Quit to Title" instead of "Disconnect".
    pub local_server: bool,
    /// `!isDemo() && !isOfflineDeveloperMode()`.
    pub friends_available: bool,
    /// The player-reporting button: off in single player when no one else was seen.
    pub reporting_active: bool,
}

pub struct PauseScreen {
    pub env: Env,
    pause: Pause,
    widgets: Arena,
}

fn icon(ctx: &Ctx, key: &str, sprite: &'static str, active: bool) -> Box<dyn Widget> {
    let mut b = SpriteIconButton::new(20, 20, ctx.tr(key), Sprites::one(sprite), [15, 15], true);
    b.base.active = active;
    Box::new(b)
}

/// `RowHelper.addChild(widget, span, settings)`.
fn add(grid: &mut Layout, helper: &mut RowHelper, widgets: &mut Arena, w: Box<dyn Widget>, span: usize, s: Settings) {
    widgets.push(w);
    helper.add(grid, Elem::Widget(widgets.len() - 1), span, s);
}

impl PauseScreen {
    /// `new PauseScreen(showPauseMenu)` after `init`, in a world (`minecraft.level != null`).
    pub fn new(ctx: &Ctx, env: Env, pause: Pause) -> PauseScreen {
        let mut widgets: Arena = Vec::new();
        if pause.show_menu {
            Self::menu(ctx, &env, &pause, &mut widgets);
        }
        let title = ctx.tr(if pause.show_menu { "menu.game" } else { "menu.paused" });
        let tw = ctx.width(&title);
        widgets.push(Box::new(StringWidget::new(ctx.font, env.width / 2 - tw / 2, if pause.show_menu { 40 } else { 10 }, tw, 9, title)));
        PauseScreen { env, pause, widgets }
    }

    /// `createPauseMenu`.
    fn menu(ctx: &Ctx, env: &Env, pause: &Pause, widgets: &mut Arena) {
        let mut grid = Layout::grid();
        grid.defaults = grid.defaults.padding(4, 4, 4, 0);
        let d = grid.defaults;
        let mut helper = RowHelper::new(2);
        let h = &mut helper;
        add(&mut grid, h, widgets, Box::new(Button::sized(204, ctx.tr("menu.returnToGame"))), 2, d.pad_top(50));
        add(&mut grid, h, widgets, Box::new(Button::sized(98, ctx.tr("gui.advancements"))), 1, d);
        add(&mut grid, h, widgets, Box::new(Button::sized(98, ctx.tr("gui.stats"))), 1, d);
        let mut row = Linear::new(false, 4);
        let icons = [
            icon(ctx, "menu.reportBugs", "pause_menu/bug", true),
            icon(ctx, "menu.sendFeedback", "pause_menu/social_interactions", true),
            icon(ctx, "gui.friends.open", "friends/friends", pause.friends_available),
            icon(ctx, "menu.playerReporting", "pause_menu/player_reporting", pause.reporting_active),
        ];
        for w in icons {
            widgets.push(w);
            row.add_default(Elem::Widget(widgets.len() - 1));
        }
        h.add(&mut grid, row.into_elem(), 2, d.align_x(0.5));
        add(&mut grid, h, widgets, Box::new(Button::sized(98, ctx.tr("menu.options"))), 1, d);
        add(&mut grid, h, widgets, Box::new(Button::sized(98, ctx.tr("options.worldOptions.button"))), 1, d);
        let disconnect = ctx.tr(if pause.local_server { "menu.returnToMenu" } else { "menu.disconnect" });
        add(&mut grid, h, widgets, Box::new(Button::sized(204, disconnect)), 2, d);
        let mut root = Elem::Layout(Box::new(grid));
        if let Elem::Layout(l) = &mut root {
            l.arrange(widgets);
        }
        align_in_rect(&mut root, widgets, 0, 0, env.width, env.height, 0.5, 0.25);
    }
}

impl Screen for PauseScreen {
    fn background(&mut self, ctx: &mut Ctx) {
        if self.pause.show_menu {
            if self.pause.topmost {
                blurred(&self.env, ctx);
            }
            menu_texture(&self.env, ctx, 0, 0, self.env.width, self.env.height);
        }
    }

    fn foreground(&mut self, ctx: &mut Ctx) {
        render_all(&mut self.widgets, ctx);
    }
}
