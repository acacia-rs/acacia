//! Each [`View`] as the Java screen showing it, built by its `init` from the viewer's state.

use acacia_ui::java::screen::confirm::{Confirm, ConfirmScreen};
use acacia_ui::java::screen::credits::credits_and_attribution;
use acacia_ui::java::screen::death::DeathScreen;
use acacia_ui::java::screen::loading::{LevelLoadingScreen, Load, Reason};
use acacia_ui::java::screen::message::MessageScreen;
use acacia_ui::java::screen::options::pages::{self, Client};
use acacia_ui::java::screen::pause::{Pause, PauseScreen};
use acacia_ui::java::screen::win::{WinScreen, scroll_speed};
use acacia_ui::java::screen::{Env, Screen};
use acacia_ui::java::text::{Style, Text};
use acacia_ui::java::widget::Ctx;

use super::menus::View;
use super::{JavaGui, options};

/// What the options pages read from a client that is not signed in to Mojang's services: no
/// friends, the vanilla high-contrast pack, the stable feature set. Telemetry is allowed, as for
/// any account that hasn't turned it off, so its screen opens.
const CLIENT: Client = Client { telemetry: true, high_contrast_pack: true, minecart: false, friends: false, friend_requests: false, ticked: true };

/// `minecraft.gui.screen()` for `view` at `size` after `init`.
pub fn build(gui: &JavaGui, ctx: &Ctx, view: &View, size: [i32; 2]) -> Box<dyn Screen> {
    let env = Env { width: size[0], height: size[1], in_world: true, blur: true };
    match view {
        View::Pause => Box::new(PauseScreen::new(ctx, env, Pause { show_menu: true, topmost: true, local_server: false, friends_available: true, reporting_active: true })),
        View::Options { page, settings } => {
            if let Some(screen) = beside_options(ctx, env, page) {
                return screen;
            }
            let values = |id: &str| options::value(settings, id);
            let screen = pages::build(ctx, env, page, &values, CLIENT).or_else(|| pages::build(ctx, env, "options", &values, CLIENT));
            Box::new(screen.expect("the options screen builds"))
        }
        View::Win { ticks } => Box::new(credits_roll(gui, size, *ticks)),
        // The bot reports no load progress or chunk statuses, only that the player has not arrived.
        View::Loading => Box::new(LevelLoadingScreen::new(env, Reason::Other, Load { has_progress: false, progress: 0.0, view: None, visualize: false })),
        // No death message or score reaches the viewer.
        View::Death { ticks } => Box::new(DeathScreen::new(ctx, env, None, false, 0, *ticks)),
        View::TitleConfirm { ticks } => {
            let confirm = Confirm {
                title: ctx.tr("deathScreen.quit.confirm"),
                message: Text::default(),
                message_style: Style::default(),
                yes: ctx.tr("deathScreen.titleScreen"),
                no: ctx.tr("deathScreen.respawn"),
                delay: Some(20),
                ticks: *ticks,
            };
            let mut screen = ConfirmScreen::new(ctx, env, confirm);
            screen.death_background = true;
            Box::new(screen)
        }
        View::Disconnected { reason } => Box::new(MessageScreen::disconnected(ctx, Env { in_world: false, ..env }, ctx.tr("disconnect.lost"), Text::legacy(reason), Style::default())),
    }
}

/// `WinScreen` from the options (no poem, no speed-up keys held), `ticks` after it opened: `tick`
/// adds `speed` to the scroll each whole tick and the frame the partial tick's share.
pub fn credits_roll(gui: &JavaGui, size: [i32; 2], ticks: f32) -> WinScreen {
    let env = Env { width: size[0], height: size[1], in_world: true, blur: true };
    let mut screen = WinScreen::new(env, false, gui.credits().clone());
    screen.scroll = ticks.floor() * scroll_speed(false, false, 0, false);
    screen.partial = ticks.fract();
    screen
}

/// The screens the options link to that are not option lists (`input::link`'s other pages).
fn beside_options(ctx: &Ctx, env: Env, page: &str) -> Option<Box<dyn Screen>> {
    Some(match page {
        "credits" => Box::new(credits_and_attribution(ctx, env)),
        _ => return None,
    })
}
