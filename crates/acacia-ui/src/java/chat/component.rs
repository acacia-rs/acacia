//! `ChatComponent`: messages wrapped to the chat width into the 100 newest lines, each fading 200
//! ticks after it arrived, drawn scaled at the bottom left with its tag's indicator; with the chat
//! screen open (focused) every line shows, the page is taller and a scroll bar marks the offset.
//! Ported from Horizon's `chat.rs` (MIT).

use crate::java::gui::{Gui, Metrics, PoseOp};
use crate::java::mth::{as8, ceil, floor, floor_d, white, with_alpha};
use crate::java::split::wrap_components;
use crate::java::text::Text;

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

/// `GuiMessageTag`: the indicator bar's colour and an optional icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tag {
    pub indicator: u32,
    pub icon: Option<TagIcon>,
}

impl Tag {
    pub const SYSTEM: Tag = Tag { indicator: 0xD0D0D0, icon: None };
    pub const CHAT_NOT_SECURE: Tag = Tag { indicator: 0xD0D0D0, icon: None };
    pub const CHAT_ERROR: Tag = Tag { indicator: 0xFF5555, icon: None };
    pub const CHAT_MODIFIED: Tag = Tag { indicator: 0x606060, icon: Some(CHAT_MODIFIED) };
}

#[derive(Clone, Debug)]
struct Message {
    added: i32,
    content: Text,
    tag: Option<Tag>,
}

/// `GuiMessage.Line`.
#[derive(Clone, Debug)]
struct Line {
    added: i32,
    content: Text,
    tag: Option<Tag>,
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
    lines: Vec<Line>,
    scroll: i32,
    new_since_scroll: bool,
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
        if let Some(icon) = message.tag.and_then(|t| t.icon) {
            width -= icon.width + 4 + 2;
        }
        for line in wrap_components(m, &message.content, width, self.options.colors) {
            if focused && self.scroll > 0 {
                self.new_since_scroll = true;
                self.scroll_chat(1, focused);
            }
            self.lines.insert(0, Line { added: message.added, content: line, tag: message.tag });
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

    /// `ChatComponent.extractRenderState` in `BACKGROUND` mode, or `FOREGROUND` when `focused`,
    /// at tick `ticks`. The mouse is not tracked, so tag tooltips and icons (shown on hover) are
    /// not drawn; nor are the delayed-message queue line and the restricted prompt.
    pub fn extract(&self, gui: &mut Gui, ticks: i32, focused: bool) {
        let total = self.lines.len() as i32;
        if total == 0 {
            return;
        }
        let o = &self.options;
        let scale = o.scale as f32;
        let max_width = ceil(o.width_px() as f32 / scale);
        let chat_bottom = floor((gui.height - 40) as f32 / scale);
        let text_opacity = o.opacity as f32 * 0.9 + 0.1;
        let background_opacity = o.background_opacity as f32;
        let entry_height = o.line_height();
        let bottom_to_text = (8.0 * (o.line_spacing + 1.0) - 4.0 * o.line_spacing).round() as i32;
        // forEachLine: oldest visible first, the transparent ones skipped.
        let per_page = self.lines_per_page(focused);
        let visible: Vec<(i32, &Line, f32)> = (0..(total - self.scroll).min(per_page))
            .rev()
            .map(|i| (i, &self.lines[(i + self.scroll) as usize]))
            .map(|(i, line)| (i, line, if focused { 1.0 } else { time_alpha(ticks, line.added) }))
            .filter(|l| l.2 > 1.0e-5)
            .collect();
        gui.push(vec![PoseOp::Scale(scale, scale), PoseOp::Translate(4.0, 0.0)]);
        for &(i, _, alpha) in &visible {
            let bottom = chat_bottom - i * entry_height;
            gui.fill(-4, bottom - entry_height, max_width + 4 + 4, bottom, as8(alpha * background_opacity) << 24);
        }
        for &(i, line, alpha) in &visible {
            let bottom = chat_bottom - i * entry_height;
            let opacity = alpha * text_opacity;
            gui.text(line.content.clone(), 0, bottom - bottom_to_text, white(opacity), true);
            if let Some(tag) = line.tag {
                gui.fill(-4, bottom - entry_height, -2, bottom, with_alpha(as8(opacity), tag.indicator));
            }
        }
        if focused {
            let chat_height = visible.len() as i32 * entry_height;
            let virtual_height = total * entry_height;
            let y = self.scroll * chat_height / total - chat_bottom;
            let height = chat_height * chat_height / virtual_height;
            if virtual_height != chat_height {
                let alpha = if y > 0 { 170 } else { 96 };
                let color = if self.new_since_scroll { 0xCC3333 } else { 0x3333AA };
                let x = max_width + 4;
                gui.fill(x, -y, x + 2, -y - height, with_alpha(alpha, color));
                gui.fill(x + 2, -y, x + 1, -y - height, with_alpha(alpha, 0xCCCCCC));
            }
        }
        gui.pop();
    }
}
