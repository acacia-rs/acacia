//! The Java look's HUD: viewer state into `acacia_ui::java::hud::frame`, driven at the game's 20
//! ticks a second.

use std::time::Instant;

use acacia_bot::events::TitleKind;
use acacia_ui::effects::Active;
use acacia_ui::java::chat::component::Tag;
use acacia_ui::java::gui::Gui;
use acacia_ui::java::hud::frame::ScreenInfo;
use acacia_ui::java::hud::hotbar::OFFHAND_KEY;
use acacia_ui::java::text::Text;
use acacia_ui::lang::Lang;
use acacia_ui::sidebar::Sidebar;

use super::{JavaGui, inputs};
use crate::control::Stack;
use crate::me::Me;

/// `Options.notificationDisplayTime`'s default.
const DISPLAY_TIME: f64 = 1.0;
/// Ticks caught up at most after a stall, enough to run out every timer.
const MAX_CATCH_UP: i64 = 1200;

/// What the viewer shows this frame, for the HUD.
pub struct HudView<'a> {
    pub me: &'a Me,
    /// `me.effects` with their time counted down.
    pub effects: &'a [Active],
    pub screen: Option<ScreenInfo>,
    pub chat_open: bool,
    /// The debug screen (F3) is shown.
    pub debug: bool,
    pub sidebar: Option<&'a Sidebar>,
    /// The player list's names while Tab is held.
    pub players: Option<&'a [String]>,
    pub lang: &'a Lang,
}

impl JavaGui {
    /// `Hud.extractRenderState` for this frame, after the ticks due by `now`.
    pub fn hud(&mut self, v: &HudView, size: [i32; 2], now: Instant) -> Gui {
        let millis = self.millis(now);
        let due = millis / 50;
        let held = inputs::held_name(v.me, v.lang);
        for _ in 0..(due - self.ticks).clamp(0, MAX_CATCH_UP) {
            self.hud.tick(held.clone(), DISPLAY_TIME);
        }
        self.ticks = due;
        let delta = (millis - self.last_frame) as f32 / 50.0;
        self.last_frame = millis;
        let i = inputs::inputs(v, size, millis, delta);
        let mut gui = Gui::new(size[0], size[1]);
        self.hud.extract(&mut gui, &self.fonts, &|_| None, &i);
        gui
    }

    /// `Hud.extractDeferredSubtitles`, drawn over the screens.
    pub fn deferred_subtitles(&mut self, size: [i32; 2]) -> Gui {
        let mut gui = Gui::new(size[0], size[1]);
        self.hud.extract_deferred_subtitles(&mut gui, &self.fonts);
        gui
    }

    /// `ChatComponent.addServerSystemMessage`, for every Bedrock line (player chat included: the
    /// viewer gets it as text).
    pub fn add_chat(&mut self, line: &str, focused: bool) {
        self.hud.chat.add(&self.fonts, Text::literal(line), Some(Tag::SYSTEM_SINGLE_PLAYER), self.hud.tick_count, focused);
    }

    /// `Hud.setTitle`, `setSubtitle`, `setOverlayMessage` and `clearTitles`.
    pub fn show_title(&mut self, kind: TitleKind, text: &str) {
        let titles = &mut self.hud.titles;
        match kind {
            TitleKind::Title => titles.set_title(Text::literal(text)),
            TitleKind::Subtitle => titles.set_subtitle(Text::literal(text)),
            TitleKind::ActionBar => titles.set_overlay(Text::literal(text), false),
            TitleKind::Clear => titles.clear(),
        }
    }
}

/// The hotbar's item keys: slot numbers, and `OFFHAND_KEY`.
pub fn hotbar_stack(me: &Me, key: i32) -> Option<Stack> {
    match key {
        OFFHAND_KEY => me.offhand.clone(),
        k => me.items.get(usize::try_from(k).ok()?)?.clone(),
    }
}
