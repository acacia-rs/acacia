//! The Java look's screens over the world: the menus (`acacia_ui::java::screen`) and the chat
//! screen (`java::chat::ChatScreen`, with the HUD's chat lines). A menu is rebuilt from the
//! viewer's state for every frame and every input, with its focus and scroll restored from the
//! caller's [`Interaction`].

use std::sync::Arc;
use std::time::Instant;

use acacia_ui::DrawList;
use acacia_ui::java::chat::{ChatLines, ChatScreen, Completion};
use acacia_ui::java::command::Tree;
use acacia_ui::java::gui::{Gui, Metrics};
use acacia_ui::java::screen::{Escape, extract};
use acacia_ui::java::tooltip::game::Game;
use acacia_ui::java::tooltip::item::Details;
use acacia_ui::java::widget::Ctx;
use acacia_ui::java::widget::input::{Fired, Interaction, Key};

pub use super::gui_size;
pub use super::options::{apply as apply_option, value as option_value};

use super::views::build;
use super::{JavaGui, PictureRequest};
use crate::settings::Settings;
use crate::ui::{Skin, Ui};

/// A menu the viewer shows.
#[derive(Clone, Debug, PartialEq)]
pub enum View {
    Pause,
    /// An options page (`pages::build`'s names, or `input::link`'s screens beside them) over these
    /// settings.
    Options { page: &'static str, settings: Settings },
    /// The credits roll (`WinScreen`), `ticks` after it opened.
    Win { ticks: f32 },
    /// `LevelLoadingScreen`: the player has not arrived yet.
    Loading,
    /// `SignEditScreen` (or the hanging sign's) on the lines typed so far; `cursor` is in bytes.
    Sign { lines: [String; 4], line: usize, cursor: usize, hanging: bool },
    /// `BookViewScreen`, or `BookEditScreen` while `writing` with the caret `caret` bytes in.
    Book { pages: Vec<String>, page: usize, writing: bool, caret: usize },
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
        let mut screen = build(self, &ctx, view, size);
        screen.restore(s);
        extract(screen.as_mut(), &mut ctx);
        gui
    }

    /// `input` given to the menu; `s` keeps its focus and scroll.
    pub fn menu_input(&self, view: &View, s: &mut Interaction, size: [i32; 2], input: Input) -> Option<Reply> {
        let mut gui = Gui::new(size[0], size[1]);
        let lang = self.lang();
        let ctx = Ctx::new(&mut gui, &self.fonts, &lang, (0, 0));
        let mut screen = build(self, &ctx, view, size);
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

    /// The chat screen's calls: the HUD's chat lines hovered by the mouse, the input and its
    /// suggestions, then a hover tooltip. Bedrock sends no registry facts and the viewer has no
    /// advanced tooltips, so hovered items show their plain tooltip and entities none.
    pub fn chat(&self, chat: &mut ChatScreen, mouse: (i32, i32), blink_ms: u64) -> Gui {
        let mut gui = Gui::new(chat.width, chat.height);
        let lang = self.lang();
        let game = Game::default();
        let details = Details { lang: &lang, advanced: false, creative: false, game: &game, player: None, key: None };
        let lines = ChatLines { chat: &self.hud.chat, ticks: self.hud.tick_count, details: &details };
        // Options.getBackgroundColor(Integer.MIN_VALUE): backgroundForChatOnly is on by default.
        chat.extract(&mut gui, &self.fonts, mouse, blink_ms, 0x8000_0000, Some(lines));
        gui
    }

    /// The chat input then the menu, painted into `list`.
    #[allow(clippy::too_many_arguments)]
    pub fn screens(&mut self, skin: &mut Skin, shown: Shown, size: [i32; 2], mouse: (i32, i32), now: Instant, list: &mut DrawList, pictures: &mut Vec<PictureRequest>) {
        if let Some((chat, blink_ms)) = shown.chat {
            let gui = self.chat(chat, mouse, blink_ms);
            self.paint(skin, &|_| None, &gui, list, pictures);
        }
        if let Some((view, s)) = shown.menu {
            let gui = self.menu(&view, &s, size, mouse, now);
            self.paint(skin, &|_| None, &gui, list, pictures);
        }
    }
}

/// The open menu, over everything else: the Bedrock look's (`menu`) or Java's screens.
#[allow(clippy::too_many_arguments)]
pub(in crate::ui) fn draw(java_gui: &mut JavaGui, skin: &mut Skin, menu: Option<(&str, &[String])>, java: Option<Shown>, mouse: [f32; 2], size: [u32; 2], scale: u32, now: Instant, list: &mut DrawList, pictures: &mut Vec<PictureRequest>) {
    if let Some((title, buttons)) = menu {
        acacia_ui::menu::draw(list, &skin.theme, title, buttons, mouse, [(size[0] / scale) as f32, (size[1] / scale) as f32]);
    }
    if let Some(shown) = java {
        java_gui.screens(skin, shown, gui_size(size, scale), (mouse[0] as i32, mouse[1] as i32), now, list, pictures);
    }
}

impl Ui {
    /// [`JavaGui::menu_input`].
    pub fn java_menu_input(&self, view: &View, s: &mut Interaction, size: [i32; 2], input: Input) -> Option<Reply> {
        self.java_gui.menu_input(view, s, size, input)
    }

    /// Whether the credits roll `ticks` after it opened has passed its end at `size`.
    pub fn java_credits_finished(&self, ticks: f32, size: [i32; 2]) -> bool {
        super::views::credits_roll(&self.java_gui, size, ticks).finished()
    }

    /// The Java look's font metrics, which the chat input measures with.
    pub fn java_font(&self) -> &dyn Metrics {
        &self.java_gui.fonts
    }

    /// What the chat input completes: the online players' names and the server's commands, whose
    /// errors read the Java look's translations.
    pub fn java_completion(&self, names: Vec<String>, commands: Arc<Tree>) -> Completion {
        Completion::new(names, commands, &self.java_gui.lang())
    }
}
