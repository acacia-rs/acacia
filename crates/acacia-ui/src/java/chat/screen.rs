//! `ChatScreen` without the chat lines (`ChatComponent` draws those): the input along the bottom,
//! its background, sent-message history on Up/Down, and the suggestion list. Ported from
//! Horizon's `chat.rs` (MIT).

use super::edit_box::{EditBox, Key};
use super::suggestions::CommandSuggestions;
use crate::java::gui::{Gui, Metrics};
use crate::java::text::Text;

/// What a key did to the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatAction {
    None,
    /// Escape: the screen closes.
    Close,
}

#[derive(Clone, Debug)]
pub struct ChatScreen {
    pub input: EditBox,
    pub suggestions: CommandSuggestions,
    pub width: i32,
    pub height: i32,
    history_buffer: String,
    history_pos: usize,
    /// The names a message completes to (`getCustomTabSuggestions`: the online players).
    pub names: Vec<String>,
}

impl ChatScreen {
    /// `ChatScreen.init`: the input across the bottom, focused, holding `initial`.
    pub fn new(m: &dyn Metrics, initial: &str, history_len: usize, names: Vec<String>, width: i32, height: i32) -> ChatScreen {
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
            names,
        };
        // The responder is set after setValue, so only the explicit update runs here.
        screen.input.set_value(m, initial);
        screen.suggestions.update(&mut screen.input, &screen.names);
        screen
    }

    /// `onEdited`, the input's responder.
    fn edited(&mut self) {
        self.suggestions.set_allow(true);
        self.suggestions.update(&mut self.input, &self.names);
    }

    fn set_value(&mut self, m: &dyn Metrics, value: &str) {
        self.input.set_value(m, value);
        self.edited();
    }

    /// A typed character (`charTyped` on the focused input).
    pub fn char_typed(&mut self, m: &dyn Metrics, c: char) {
        if self.input.char_typed(m, c) {
            self.edited();
        }
    }

    /// `ChatScreen.keyPressed` with `history` the sent messages (`getRecentChat`).
    pub fn key(&mut self, m: &dyn Metrics, history: &[String], key: Key, shift: bool, ctrl: bool) -> ChatAction {
        if self.suggestions_key(m, key, shift) {
            return ChatAction::None;
        }
        match key {
            Key::Escape => return ChatAction::Close,
            Key::Up => self.move_in_history(m, history, -1),
            Key::Down => self.move_in_history(m, history, 1),
            _ => {
                if self.input.key(m, key, shift, ctrl) == Some(true) {
                    self.edited();
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
    fn use_suggestion(&mut self, m: &dyn Metrics, mut list: super::suggestions::SuggestionsList) {
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

    /// `ChatScreen.extractRenderState` without the chat lines: the input's background
    /// (`background`, `Options.getBackgroundColor(Integer.MIN_VALUE)`), the input, then the
    /// suggestion list. `blink_ms` is the time since the input was focused.
    pub fn extract(&mut self, gui: &mut Gui, m: &dyn Metrics, mouse: (i32, i32), blink_ms: u64, background: u32) {
        let (w, h) = (self.width, self.height);
        gui.fill(2, h - 14, w - 2, h - 2, background);
        self.input.extract(gui, m, blink_ms, &|s, _| Text::literal(s));
        if let Some(list) = self.suggestions.list.as_mut() {
            list.extract(gui, &mut self.input, mouse);
        }
    }
}
