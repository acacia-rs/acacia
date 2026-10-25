//! `ChatScreen`: the input along the bottom, its background, sent-message history on Up/Down, the
//! suggestions, and the chat's lines (`ChatComponent`) with hover tooltips. Ported from Horizon's
//! `chat.rs` (MIT).

use super::component::Chat;
use super::component_draw::Focus;
use super::edit_box::{EditBox, Key};
use super::suggestions::{CommandSuggestions, Completion, Sources, java_whitespace};
use super::suggestions_list::SuggestionsList;
use crate::java::gui::{Gui, Metrics};
use crate::java::text::Text;
use crate::java::tooltip::item::Details;

/// The chat the screen draws above its input: its lines at `ticks` (`Hud.getGuiTicks`); `details`
/// describe hovered items and whether tooltips are advanced.
pub struct ChatLines<'a> {
    pub chat: &'a Chat,
    pub ticks: i32,
    pub details: &'a Details<'a>,
}

/// What a key did to the screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatAction {
    None,
    /// Escape: the screen closes.
    Close,
    /// Enter: the screen closes and `handleChatInput` sends this (`/` a command) and adds it to the
    /// recent chat, unless it is empty.
    Send(String),
}

/// `ChatScreen.normalizeChatMessage`: trimmed, runs of whitespace one space (Apache's
/// `normalizeSpace`, which also turns no-break spaces into spaces), cut to 256 units.
pub fn normalize(message: &str) -> String {
    let words: Vec<String> = message.split(java_whitespace).filter(|w| !w.is_empty()).map(|w| w.replace('\u{a0}', " ")).collect();
    let joined = words.join(" ");
    let units: Vec<u16> = joined.trim_matches(|c| c <= ' ').encode_utf16().take(256).collect();
    String::from_utf16_lossy(&units)
}

#[derive(Clone, Debug)]
pub struct ChatScreen {
    pub input: EditBox,
    pub suggestions: CommandSuggestions,
    pub width: i32,
    pub height: i32,
    history_buffer: String,
    history_pos: usize,
    pub completion: Completion,
}

impl ChatScreen {
    /// `ChatScreen.init`: the input across the bottom, focused, holding `initial`.
    pub fn new(m: &dyn Metrics, initial: &str, history_len: usize, completion: Completion, width: i32, height: i32) -> ChatScreen {
        let mut input = EditBox::new(4, height - 12, width - 4, 12);
        input.max_length = 256;
        input.bordered = false;
        input.focused = true;
        let mut screen = ChatScreen {
            input,
            suggestions: CommandSuggestions::default(),
            width,
            height,
            history_buffer: String::new(),
            history_pos: history_len,
            completion,
        };
        // The responder is set after setValue, so only the explicit update runs here.
        screen.input.set_value(m, initial);
        screen.update(m);
        screen
    }

    /// `commandSuggestions.updateCommandInfo()`.
    fn update(&mut self, m: &dyn Metrics) {
        let s = Sources { m, c: &self.completion, screen_height: self.height };
        self.suggestions.update(&s, &mut self.input);
    }

    /// `onEdited`, the input's responder.
    fn edited(&mut self, m: &dyn Metrics) {
        self.suggestions.set_allow(true);
        self.update(m);
    }

    fn set_value(&mut self, m: &dyn Metrics, value: &str) {
        self.input.set_value(m, value);
        self.edited(m);
    }

    /// A typed character (`charTyped` on the focused input).
    pub fn char_typed(&mut self, m: &dyn Metrics, c: char) {
        if self.input.char_typed(m, c) {
            self.edited(m);
        }
    }

    /// `ChatScreen.keyPressed` with `history` the sent messages (`getRecentChat`).
    pub fn key(&mut self, m: &dyn Metrics, history: &[String], key: Key, shift: bool, ctrl: bool) -> ChatAction {
        if self.suggestions_key(m, key, shift) {
            return ChatAction::None;
        }
        match key {
            Key::Escape => return ChatAction::Close,
            Key::Enter => return ChatAction::Send(normalize(&self.input.value())),
            Key::Up => self.move_in_history(m, history, -1),
            Key::Down => self.move_in_history(m, history, 1),
            _ => {
                if self.input.key(m, key, shift, ctrl) == Some(true) {
                    self.edited(m);
                }
            }
        }
        ChatAction::None
    }

    /// `CommandSuggestions.keyPressed`: the list takes Up, Down, Tab and Escape; Tab without a
    /// list shows it.
    fn suggestions_key(&mut self, m: &dyn Metrics, key: Key, shift: bool) -> bool {
        let Some(mut list) = self.suggestions.list.take() else {
            if key == Key::Tab {
                self.suggestions.show(m, &mut self.input, self.height);
                return true;
            }
            return false;
        };
        match key {
            Key::Up | Key::Down => {
                list.cycle(&mut self.input, if key == Key::Up { -1 } else { 1 });
                list.set_tab_cycles(false);
                self.suggestions.list = Some(list);
                true
            }
            Key::Tab => {
                if list.tab_cycles() {
                    list.cycle(&mut self.input, if shift { -1 } else { 1 });
                }
                self.use_suggestion(m, list);
                true
            }
            Key::Escape => {
                self.input.suggestion = None;
                true
            }
            _ => {
                self.suggestions.list = Some(list);
                false
            }
        }
    }

    /// `SuggestionsList.useSuggestion`: the completion applied, the list kept.
    fn use_suggestion(&mut self, m: &dyn Metrics, mut list: SuggestionsList) {
        let (value, end) = list.chosen();
        self.suggestions.keep_suggestions = true;
        self.suggestions.list = Some(list.clone());
        self.set_value(m, &value);
        self.input.set_cursor(m, end);
        self.input.set_highlight(m, end);
        list.select(&mut self.input, list.current as isize);
        list.set_tab_cycles(true);
        self.suggestions.keep_suggestions = false;
        self.suggestions.list = Some(list);
    }

    /// `ChatScreen.moveInHistory`.
    fn move_in_history(&mut self, m: &dyn Metrics, history: &[String], dir: isize) {
        let max = history.len();
        let pos = (self.history_pos as isize + dir).clamp(0, max as isize) as usize;
        if pos == self.history_pos {
            return;
        }
        if pos == max {
            self.history_pos = max;
            let buffer = self.history_buffer.clone();
            self.set_value(m, &buffer);
        } else {
            if self.history_pos == max {
                self.history_buffer = self.input.value();
            }
            self.set_value(m, &history[pos]);
            self.suggestions.set_allow(false);
            self.history_pos = pos;
        }
    }

    /// A left click (`mouseClicked`): a line of the suggestion list, else the input's caret.
    pub fn click(&mut self, m: &dyn Metrics, x: f64, y: f64) {
        if let Some(list) = self.suggestions.list.as_mut() {
            let (xi, yi) = (x as i32, y as i32);
            if list.contains(xi, yi) {
                if let Some(line) = list.line_at(yi) {
                    let mut list = self.suggestions.list.take().expect("checked above");
                    list.select(&mut self.input, line as isize);
                    self.use_suggestion(m, list);
                }
                return;
            }
        }
        if self.input.contains(x, y) {
            self.input.click(m, x, false);
        }
    }

    /// `mouseScrolled` with the mouse at `mouse`: the list scrolls when under it; otherwise the
    /// chat lines to scroll (`scrollChat`), seven a notch unless Shift is held.
    pub fn scroll(&mut self, mouse: (i32, i32), amount: f64, shift: bool) -> i32 {
        let amount = amount.clamp(-1.0, 1.0);
        if let Some(list) = self.suggestions.list.as_mut().filter(|l| l.contains(mouse.0, mouse.1)) {
            list.scroll(amount);
            return 0;
        }
        (if shift { amount } else { amount * 7.0 }) as i32
    }

    /// `ChatScreen.extractRenderState` then `extractDeferredElements`: the input's background
    /// (`background`, `Options.getBackgroundColor(Integer.MIN_VALUE)`), the chat's `lines` when
    /// given, the input, the suggestions, then the hovered text's or tag's tooltip. `blink_ms`
    /// is the time since the input was focused.
    pub fn extract(&mut self, gui: &mut Gui, m: &dyn Metrics, mouse: (i32, i32), blink_ms: u64, background: u32, lines: Option<ChatLines>) {
        let (w, h) = (self.width, self.height);
        gui.fill(2, h - 14, w - 2, h - 2, background);
        let mut focus = lines.as_ref().map(|l| Focus::new(m, l.details.lang, mouse));
        if let (Some(l), Some(f)) = (&lines, &mut focus) {
            l.chat.extract_focused(gui, l.ticks, f);
        }
        let suggestions = &self.suggestions;
        self.input.extract(gui, m, blink_ms, &|s, offset| suggestions.format(s, offset).unwrap_or_else(|| Text::literal(s).as_forward()));
        self.suggestions.extract(gui, &mut self.input, mouse, h);
        let (Some(l), Some(mut f)) = (lines, focus) else { return };
        let hover = f.hovered.as_ref().and_then(|s| s.hover.clone());
        if let Some(tooltip) = hover.and_then(|e| e.tooltip(m, w, mouse, l.details)) {
            f.tooltip.set(tooltip, false);
        }
        f.tooltip.extract(gui, m);
    }
}
