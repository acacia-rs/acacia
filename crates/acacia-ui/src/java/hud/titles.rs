//! `Hud`'s titles, subtitle and action bar (`extractTitle`, `extractOverlayMessage` and their
//! timers in `Hud.tick`) and the selected item name (`extractSelectedItemName`,
//! `toolHighlightTimer`). Ported from Horizon's `hud_overlays.rs` (MIT).

use crate::java::gui::{Gui, Metrics, PoseOp};
use crate::java::mth::{color_from_float, hsv_to_argb, multiply, with_alpha};
use crate::java::text::{Style, Text};

/// `Options.getBackgroundColor(defaultOpacity)`: `chat_only` is `backgroundForChatOnly`,
/// `opacity` `textBackgroundOpacity`.
pub fn background_color(chat_only: bool, opacity: f32, default: f32) -> u32 {
    color_from_float(if chat_only { default } else { opacity }, 0.0, 0.0, 0.0)
}

/// `GuiGraphicsExtractor.textWithBackdrop`; `background` is `getBackgroundColor(0.0F)`.
pub fn text_with_backdrop(gui: &mut Gui, text: Text, x: i32, y: i32, width: i32, color: u32, background: u32) {
    if background != 0 {
        gui.fill(x - 2, y - 2, x + width + 2, y + 9 + 2, multiply(background, color));
    }
    gui.text(text, x, y, color, true);
}

/// `Hud.title`, `subtitle`, `titleTime`, the fade times, `overlayMessageString` and
/// `overlayMessageTime`.
#[derive(Clone, Debug, PartialEq)]
pub struct Titles {
    pub title: Option<Text>,
    pub subtitle: Option<Text>,
    pub time: i32,
    pub fade_in: i32,
    pub stay: i32,
    pub fade_out: i32,
    pub overlay: Option<Text>,
    pub overlay_time: i32,
    /// `animateOverlayMessageColor`: the "Now Playing" rainbow.
    pub overlay_animated: bool,
}

impl Default for Titles {
    fn default() -> Titles {
        Titles { title: None, subtitle: None, time: 0, fade_in: 10, stay: 70, fade_out: 20, overlay: None, overlay_time: 0, overlay_animated: false }
    }
}

impl Titles {
    pub fn set_title(&mut self, title: Text) {
        self.title = Some(title);
        self.time = self.fade_in + self.stay + self.fade_out;
    }

    pub fn set_subtitle(&mut self, subtitle: Text) {
        self.subtitle = Some(subtitle);
    }

    /// `setTimes`: a negative time keeps the current one.
    pub fn set_times(&mut self, fade_in: i32, stay: i32, fade_out: i32) {
        if fade_in >= 0 {
            self.fade_in = fade_in;
        }
        if stay >= 0 {
            self.stay = stay;
        }
        if fade_out >= 0 {
            self.fade_out = fade_out;
        }
        if self.time > 0 {
            self.time = self.fade_in + self.stay + self.fade_out;
        }
    }

    /// `clearTitles`.
    pub fn clear(&mut self) {
        self.title = None;
        self.subtitle = None;
        self.time = 0;
    }

    /// `resetTitleTimes`.
    pub fn reset_times(&mut self) {
        (self.fade_in, self.stay, self.fade_out) = (10, 70, 20);
    }

    /// `setOverlayMessage`.
    pub fn set_overlay(&mut self, text: Text, animate: bool) {
        self.overlay = Some(text);
        self.overlay_time = 60;
        self.overlay_animated = animate;
    }

    /// The title and action bar part of `Hud.tick`.
    pub fn tick(&mut self) {
        if self.overlay_time > 0 {
            self.overlay_time -= 1;
        }
        if self.time > 0 {
            self.time -= 1;
            if self.time <= 0 {
                self.title = None;
                self.subtitle = None;
            }
        }
    }

    /// `Hud.extractOverlayMessage`.
    pub fn overlay_message(&self, gui: &mut Gui, font: &dyn Metrics, partial: f32, background: u32) {
        let Some(message) = self.overlay.as_ref().filter(|_| self.overlay_time > 0) else { return };
        let t = self.overlay_time as f32 - partial;
        let alpha = ((t * 255.0 / 20.0) as i32).min(255);
        if alpha <= 0 {
            return;
        }
        gui.push(vec![PoseOp::Translate((gui.width / 2) as f32, (gui.height - 68) as f32)]);
        let color = if self.overlay_animated { hsv_to_argb(t / 50.0, 0.7, 0.6, alpha as u32) } else { with_alpha(alpha as u32, 0xFF_FFFF) };
        let width = font.width(message);
        text_with_backdrop(gui, message.clone(), -width / 2, -4, width, color, background);
        gui.pop();
    }

    /// `Hud.extractTitle`.
    pub fn title(&self, gui: &mut Gui, font: &dyn Metrics, partial: f32, background: u32) {
        let Some(title) = self.title.as_ref().filter(|_| self.time > 0) else { return };
        let t = self.time as f32 - partial;
        let mut alpha = 255;
        if self.time > self.fade_out + self.stay {
            let time = (self.fade_in + self.stay + self.fade_out) as f32 - t;
            alpha = (time * 255.0 / self.fade_in as f32) as i32;
        }
        if self.time <= self.fade_out {
            alpha = (t * 255.0 / self.fade_out as f32) as i32;
        }
        let alpha = alpha.clamp(0, 255);
        if alpha <= 0 {
            return;
        }
        gui.push(vec![PoseOp::Translate((gui.width / 2) as f32, (gui.height / 2) as f32)]);
        gui.push(vec![PoseOp::Scale(4.0, 4.0)]);
        let color = with_alpha(alpha as u32, 0xFF_FFFF);
        let width = font.width(title);
        text_with_backdrop(gui, title.clone(), -width / 2, -10, width, color, background);
        gui.pop();
        if let Some(subtitle) = &self.subtitle {
            gui.push(vec![PoseOp::Scale(2.0, 2.0)]);
            let width = font.width(subtitle);
            text_with_backdrop(gui, subtitle.clone(), -width / 2, 5, width, color, background);
            gui.pop();
        }
        gui.pop();
    }
}

/// The selected stack as `Hud.tick` compares it.
#[derive(Clone, Debug, PartialEq)]
pub struct HeldName {
    pub item: String,
    /// `ItemStack.getHoverName`.
    pub hover_name: Text,
    /// `getRarity().color()`.
    pub rarity_color: u32,
    /// The stack has `CUSTOM_NAME`.
    pub custom_name: bool,
}

/// `Hud.toolHighlightTimer` and `lastToolHighlight`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolHighlight {
    pub timer: i32,
    pub last: Option<HeldName>,
}

impl ToolHighlight {
    /// The selected-item part of `Hud.tick`; `display_time` is `notificationDisplayTime`.
    pub fn tick(&mut self, selected: Option<HeldName>, display_time: f64) {
        match &selected {
            None => self.timer = 0,
            Some(s) => {
                if self.last.as_ref().is_none_or(|l| l.item != s.item || l.hover_name != s.hover_name) {
                    self.timer = (40.0 * display_time) as i32;
                } else if self.timer > 0 {
                    self.timer -= 1;
                }
            }
        }
        self.last = selected;
    }

    /// `Hud.extractSelectedItemName`; `can_hurt` is `gameMode.canHurtPlayer()`.
    pub fn draw(&self, gui: &mut Gui, font: &dyn Metrics, can_hurt: bool, background: u32) {
        let Some(last) = self.last.as_ref().filter(|_| self.timer > 0) else { return };
        let mut name = last.hover_name.clone().with_color(last.rarity_color);
        if last.custom_name {
            name = name.with_style(&Style { italic: Some(true), ..Style::default() });
        }
        let width = font.width(&name);
        let x = (gui.width - width) / 2;
        let y = gui.height - 59 + if can_hurt { 0 } else { 14 };
        let alpha = ((self.timer as f32 * 256.0 / 10.0) as i32).min(255);
        if alpha > 0 {
            text_with_backdrop(gui, name, x, y, width, with_alpha(alpha as u32, 0xFF_FFFF), background);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java::gui::{Fixed, GuiDraw};

    #[test]
    fn title_fades_in_and_out() {
        let mut t = Titles::default();
        t.set_title(Text::literal("Hi"));
        assert_eq!(t.time, 100);
        let mut gui = Gui::new(320, 240);
        t.title(&mut gui, &Fixed(6.0), 0.5, 0);
        // 0.5 ticks into a 10-tick fade in.
        assert!(matches!(&gui.draws[2], GuiDraw::Text { color: 0x0CFF_FFFF, x: -6, .. }));
        for _ in 0..100 {
            t.tick();
        }
        assert!(t.title.is_none());
    }

    #[test]
    fn highlight_restarts_on_a_new_name() {
        let held = |n: &str| Some(HeldName { item: "stone".into(), hover_name: Text::literal(n), rarity_color: 0xFFFFFF, custom_name: false });
        let mut h = ToolHighlight::default();
        h.tick(held("Stone"), 1.0);
        h.tick(held("Stone"), 1.0);
        assert_eq!(h.timer, 39);
        h.tick(held("Rock"), 1.0);
        assert_eq!(h.timer, 40);
        h.tick(None, 1.0);
        assert_eq!(h.timer, 0);
    }

    #[test]
    fn backdrop_takes_the_text_alpha() {
        let mut gui = Gui::new(320, 240);
        text_with_backdrop(&mut gui, Text::literal("a"), 0, 0, 6, 0x80FF_FFFF, background_color(true, 0.5, 0.0));
        assert_eq!(gui.draws.len(), 1);
        text_with_backdrop(&mut gui, Text::literal("a"), 0, 0, 6, 0x80FF_FFFF, background_color(false, 0.5, 0.0));
        assert_eq!(gui.draws[1], GuiDraw::Fill { x0: -2, y0: -2, x1: 8, y1: 11, color: 0x3F00_0000 });
    }
}
