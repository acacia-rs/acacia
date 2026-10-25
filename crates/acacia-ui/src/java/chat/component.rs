//! `ChatComponent`: messages wrapped to the chat width into the 100 newest lines, each fading 200
//! ticks after it arrived, drawn scaled at the bottom left with its tag's indicator (`draw`); with
//! the chat screen open (focused) every line shows, the page is taller and a scroll bar marks the
//! offset. Ported from Horizon's `chat.rs` (MIT).

use crate::java::gui::Metrics;
use crate::java::mth::floor_d;
use crate::java::split::wrap_components;
use crate::java::text::{Lang, Text};

/// The chat options, doubles as `Options` keeps them.
#[derive(Clone, Copy, Debug)]
pub struct ChatOptions {
    pub opacity: f64,
    /// `textBackgroundOpacity`.
    pub background_opacity: f64,
    pub scale: f64,
    pub width: f64,
    pub height_focused: f64,
    pub height_unfocused: f64,
    pub line_spacing: f64,
    /// `chatColors`: legacy codes format the text, or are stripped.
    pub colors: bool,
}

impl Default for ChatOptions {
    fn default() -> Self {
        ChatOptions {
            opacity: 1.0,
            background_opacity: 0.5,
            scale: 1.0,
            width: 1.0,
            height_focused: 1.0,
            height_unfocused: 70.0 / 160.0,
            line_spacing: 0.0,
            colors: true,
        }
    }
}

impl ChatOptions {
    /// `ChatComponent.getWidth`.
    pub fn width_px(&self) -> i32 {
        floor_d(self.width * 280.0 + 40.0)
    }

    /// `ChatComponent.getHeight`.
    pub fn height_px(&self, focused: bool) -> i32 {
        floor_d(if focused { self.height_focused } else { self.height_unfocused } * 160.0 + 20.0)
    }

    /// `getLineHeight`.
    pub fn line_height(&self) -> i32 {
        (9.0 * (self.line_spacing + 1.0)) as i32
    }
}

/// `GuiMessageTag.Icon`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TagIcon {
    pub sprite: &'static str,
    pub width: i32,
    pub height: i32,
}

pub const CHAT_MODIFIED: TagIcon = TagIcon { sprite: "icon/chat_modified", width: 9, height: 9 };

/// A tag's tooltip text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagText {
    Key(&'static str),
    /// `chatModified`: the notice, then the original message in grey.
    Modified(String),
}

/// `GuiMessageTag`: the indicator bar's colour, an optional icon and tooltip text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    pub indicator: u32,
    pub icon: Option<TagIcon>,
    pub text: Option<TagText>,
}

impl Tag {
    pub const SYSTEM: Tag = Tag { indicator: 0xD0D0D0, icon: None, text: Some(TagText::Key("chat.tag.system")) };
    /// `addServerSystemMessage`'s.
    pub const SYSTEM_SINGLE_PLAYER: Tag = Tag { indicator: 0xD0D0D0, icon: None, text: Some(TagText::Key("chat.tag.system_single_player")) };
    pub const CHAT_NOT_SECURE: Tag = Tag { indicator: 0xD0D0D0, icon: None, text: Some(TagText::Key("chat.tag.not_secure")) };
    pub const CHAT_ERROR: Tag = Tag { indicator: 0xFF5555, icon: None, text: Some(TagText::Key("chat.tag.error")) };

    pub fn modified(original: &str) -> Tag {
        Tag { indicator: 0x606060, icon: Some(CHAT_MODIFIED), text: Some(TagText::Modified(original.to_owned())) }
    }

    /// `text()`.
    pub fn text(&self, lang: Lang) -> Option<Text> {
        Some(match self.text.as_ref()? {
            TagText::Key(key) => Text::translate(key, &[], lang),
            TagText::Modified(original) => {
                Text::translate("chat.tag.modified", &[], lang).append("\n").append(Text::literal(original.clone()).with_color(0xAAAAAA))
            }
        })
    }
}

#[derive(Clone, Debug)]
struct Message {
    added: i32,
    content: Text,
    tag: Option<Tag>,
}

/// `GuiMessage.Line`.
#[derive(Clone, Debug)]
pub(super) struct Line {
    pub added: i32,
    pub content: Text,
    pub tag: Option<Tag>,
    /// The message's last line (`endOfEntry`).
    pub end: bool,
}

/// `AlphaCalculator.timeBased`: opaque for 180 ticks, then fading out quadratically over 20.
pub fn time_alpha(ticks: i32, added: i32) -> f32 {
    let t = ((1.0 - f64::from(ticks.wrapping_sub(added)) / 200.0) * 10.0).clamp(0.0, 1.0);
    (t * t) as f32
}

#[derive(Clone, Debug, Default)]
pub struct Chat {
    /// Newest first.
    messages: Vec<Message>,
    /// Wrapped lines, newest first.
    pub(super) lines: Vec<Line>,
    pub(super) scroll: i32,
    pub(super) new_since_scroll: bool,
    pub options: ChatOptions,
}

const MAX_LINES: usize = 100;

impl Chat {
    pub fn new(options: ChatOptions) -> Chat {
        Chat { options, ..Chat::default() }
    }

    pub fn lines_per_page(&self, focused: bool) -> i32 {
        self.options.height_px(focused) / self.options.line_height()
    }

    /// `ChatComponent.addMessage` at `Hud.tickCount` `ticks`; `focused` is `isChatFocused`.
    pub fn add(&mut self, m: &dyn Metrics, content: Text, tag: Option<Tag>, ticks: i32, focused: bool) {
        let message = Message { added: ticks, content, tag };
        self.add_lines(m, &message, focused);
        self.messages.insert(0, message);
        self.messages.truncate(MAX_LINES);
    }

    /// `addMessageToDisplayQueue`.
    fn add_lines(&mut self, m: &dyn Metrics, message: &Message, focused: bool) {
        let mut width = floor_d(f64::from(self.options.width_px()) / self.options.scale);
        if let Some(icon) = message.tag.as_ref().and_then(|t| t.icon) {
            width -= icon.width + 4 + 2;
        }
        let lines = wrap_components(m, &message.content, width, self.options.colors);
        let count = lines.len();
        for (i, line) in lines.into_iter().enumerate() {
            if focused && self.scroll > 0 {
                self.new_since_scroll = true;
                self.scroll_chat(1, focused);
            }
            self.lines.insert(0, Line { added: message.added, content: line, tag: message.tag.clone(), end: i + 1 == count });
        }
        self.lines.truncate(MAX_LINES);
    }

    /// `rescaleChat`: the lines wrapped again after a width or scale change.
    pub fn rescale(&mut self, m: &dyn Metrics) {
        self.reset_scroll();
        self.lines.clear();
        for message in self.messages.clone().iter().rev() {
            self.add_lines(m, message, false);
        }
    }

    pub fn clear(&mut self) {
        self.messages.clear();
        self.lines.clear();
    }

    pub fn reset_scroll(&mut self) {
        self.scroll = 0;
        self.new_since_scroll = false;
    }

    /// `scrollChat`.
    pub fn scroll_chat(&mut self, dir: i32, focused: bool) {
        self.scroll += dir;
        let max = self.lines.len() as i32 - self.lines_per_page(focused);
        if self.scroll > max {
            self.scroll = max;
        }
        if self.scroll <= 0 {
            self.reset_scroll();
        }
    }
}
