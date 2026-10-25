//! The Java look's screens over the world: the menus (`acacia_ui::java::screen`) and the chat
//! input (`java::chat::ChatScreen`). A menu is rebuilt from the viewer's state for every frame and
//! every input, with its focus and scroll restored from the caller's [`Interaction`].

use std::time::Instant;

use acacia_ui::DrawList;
use acacia_ui::java::chat::ChatScreen;
use acacia_ui::java::gui::{Gui, Metrics};
use acacia_ui::java::screen::confirm::{Confirm, ConfirmScreen};
use acacia_ui::java::screen::death::DeathScreen;
use acacia_ui::java::screen::message::MessageScreen;
use acacia_ui::java::screen::options::pages::{self, Client};
use acacia_ui::java::screen::pause::{Pause, PauseScreen};
use acacia_ui::java::screen::{Env, Escape, Screen, extract};
use acacia_ui::java::text::{Style, Text};
use acacia_ui::java::widget::Ctx;
use acacia_ui::java::widget::input::{Fired, Interaction, Key};

pub use super::gui_size;
pub use super::options::{apply as apply_option, value as option_value};

use super::{JavaGui, options};
use crate::settings::Settings;
use crate::ui::{Skin, Ui};

/// A menu the viewer shows.
#[derive(Clone, Debug, PartialEq)]
pub enum View {
    Pause,
    /// An options page (`pages::build`'s names) over these settings.
    Options { page: &'static str, settings: Settings },
    /// `ticks` since the player died.
    Death { ticks: u32 },
    /// `DeathScreen.TitleConfirmScreen`, `ticks` after it opened.
    TitleConfirm { ticks: u32 },
    Disconnected { reason: String },
}

/// Input to a menu, in GUI coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Input {
    Click { x: f64, y: f64, shift: bool },
    Release,
    Drag { x: f64, y: f64, dy: f64 },
    Scroll { x: f64, y: f64, amount: f64 },
    Key { key: Key, shift: bool },
}

/// What a menu's input asks of the viewer.
#[derive(Clone, Debug, PartialEq)]
pub enum Reply {
    /// Escape closed it (`onClose`).
    Close,
    Fired(Fired),
}

/// The Java screens this frame shows.
pub struct Shown<'a> {
    /// The chat input, and how long it has been open.
    pub chat: Option<(&'a mut ChatScreen, u64)>,
    pub menu: Option<(View, Interaction)>,
}

/// What the options pages read from a client that is not signed in to Mojang's services: no
/// telemetry or friends, the vanilla high-contrast pack, the stable feature set.
const CLIENT: Client = Client { telemetry: false, high_contrast_pack: true, minecart: false, friends: false, friend_requests: false, ticked: true };

/// `minecraft.gui.screen()` for `view` at `size` after `init`.
fn build(ctx: &Ctx, view: &View, size: [i32; 2]) -> Box<dyn Screen> {
    let env = Env { width: size[0], height: size[1], in_world: true, blur: true };
    match view {
        View::Pause => Box::new(PauseScreen::new(ctx, env, Pause { show_menu: true, topmost: true, local_server: false, friends_available: true, reporting_active: true })),
        View::Options { page, settings } => {
            let values = |id: &str| options::value(settings, id);
            let screen = pages::build(ctx, env, page, &values, CLIENT).or_else(|| pages::build(ctx, env, "options", &values, CLIENT));
            Box::new(screen.expect("the options screen builds"))
        }
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

impl JavaGui {
    fn lang(&self) -> impl Fn(&str) -> Option<String> + '_ {
        |key| self.lang.get(key).cloned()
    }

    /// The menu's calls at `size` with the mouse at `mouse`.
    pub fn menu(&self, view: &View, s: &Interaction, size: [i32; 2], mouse: (i32, i32), now: Instant) -> Gui {
        let mut gui = Gui::new(size[0], size[1]);
        let lang = self.lang();
        let mut ctx = Ctx::new(&mut gui, &self.fonts, &lang, mouse);
        ctx.millis = self.millis(now);
        let mut screen = build(&ctx, view, size);
        screen.restore(s);
        extract(screen.as_mut(), &mut ctx);
        gui
    }

    /// `input` given to the menu; `s` keeps its focus and scroll.
    pub fn menu_input(&self, view: &View, s: &mut Interaction, size: [i32; 2], input: Input) -> Option<Reply> {
        let mut gui = Gui::new(size[0], size[1]);
        let lang = self.lang();
        let ctx = Ctx::new(&mut gui, &self.fonts, &lang, (0, 0));
        let mut screen = build(&ctx, view, size);
        screen.restore(s);
        let fired = match input {
            Input::Key { key: Key::Escape, .. } => {
                return match screen.escape() {
                    Escape::Close => Some(Reply::Close),
                    Escape::Fire(f) => Some(Reply::Fired(f)),
                    Escape::Ignore => None,
                };
            }
            Input::Click { x, y, shift } => screen.mouse_clicked(s, x, y, shift),
            Input::Release => {
                screen.mouse_released(s);
                None
            }
            Input::Drag { x, y, dy } => screen.mouse_dragged(s, x, y, dy),
            Input::Scroll { x, y, amount } => {
                screen.mouse_scrolled(s, x, y, amount);
                None
            }
            Input::Key { key, shift } => screen.key_pressed(s, key, shift),
        };
        fired.map(Reply::Fired)
    }

    /// The chat input's calls: `ChatScreen.extractRenderState` without the chat lines.
    pub fn chat(&self, chat: &mut ChatScreen, mouse: (i32, i32), blink_ms: u64) -> Gui {
        let mut gui = Gui::new(chat.width, chat.height);
        // Options.getBackgroundColor(Integer.MIN_VALUE): backgroundForChatOnly is on by default.
        chat.extract(&mut gui, &self.fonts, mouse, blink_ms, 0x8000_0000);
        gui
    }

    /// The chat input then the menu, painted into `list`.
    pub fn screens(&mut self, skin: &mut Skin, shown: Shown, size: [i32; 2], mouse: (i32, i32), now: Instant, list: &mut DrawList) {
        if let Some((chat, blink_ms)) = shown.chat {
            let gui = self.chat(chat, mouse, blink_ms);
            self.paint(skin, &|_| None, &gui, list);
        }
        if let Some((view, s)) = shown.menu {
            let gui = self.menu(&view, &s, size, mouse, now);
            self.paint(skin, &|_| None, &gui, list);
        }
    }
}

impl Ui {
    /// [`JavaGui::menu_input`].
    pub fn java_menu_input(&self, view: &View, s: &mut Interaction, size: [i32; 2], input: Input) -> Option<Reply> {
        self.java_gui.menu_input(view, s, size, input)
    }

    /// The Java look's font metrics, which the chat input measures with.
    pub fn java_font(&self) -> &dyn Metrics {
        &self.java_gui.fonts
    }
}
