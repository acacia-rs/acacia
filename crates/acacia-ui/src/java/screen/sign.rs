//! `AbstractSignEditScreen` as `SignEditScreen` and `HangingSignEditScreen`: the sign's texture
//! scaled up under its four lines, the edited line's cursor and selection, and Done.

use std::borrow::Cow;

use super::{Env, Screen, arena_children, render_all, transparent_background};
use crate::java::gui::{Blend, GuiDraw, PoseOp};
use crate::java::text::Text;
use crate::java::widget::button::Button;
use crate::java::widget::input::{Child, Key};
use crate::java::widget::layout::Arena;
use crate::java::widget::measure::width_str;
use crate::java::widget::{Ctx, Widget};
use crate::java::gui::Metrics;

/// `DyeColor`'s names in order, with their text colours.
pub const DYES: [(&str, u32); 16] = [
    ("white", 0xFF_FFFF), ("orange", 0xFF_681F), ("magenta", 0xFF_00FF), ("light_blue", 0x9A_C0CD), ("yellow", 0xFF_FF00), ("lime", 0xBF_FF00),
    ("pink", 0xFF_69B4), ("gray", 0x80_8080), ("light_gray", 0xD3_D3D3), ("cyan", 0x00_FFFF), ("purple", 0xA0_20F0), ("blue", 0x00_00FF),
    ("brown", 0x8B_4513), ("green", 0x00_FF00), ("red", 0xFF_0000), ("black", 0x00_0000),
];

/// `DyeColor.getTextColor` (opaque) by name; black when unknown.
pub fn dye_text_color(name: &str) -> u32 {
    0xFF00_0000 | DYES.iter().find(|d| d.0 == name).map_or(0, |d| d.1)
}

/// `ARGB.scaleRGB(color, scale)`.
fn scale_rgb(c: u32, s: f32) -> u32 {
    let ch = |shift: u32| (((c >> shift & 0xFF) as f32 * s) as i32).clamp(0, 255) as u32;
    (c & 0xFF00_0000) | ch(16) << 16 | ch(8) << 8 | ch(0)
}

/// The colour the edit screen draws a sign's text in: its dye glowing, else
/// `AbstractSignRenderer.getDarkColor` (whose glowing-black case the screen never reaches).
pub fn text_color(dye: &str, glowing: bool) -> u32 {
    let c = dye_text_color(dye);
    if glowing {
        c
    } else {
        scale_rgb(c, 0.4)
    }
}

/// The sign block being edited.
#[derive(Clone, Debug)]
pub struct Sign {
    /// `WoodType.name()`: `oak`, `dark_oak`, `crimson`...
    pub wood: String,
    pub hanging: bool,
    /// A standing sign shows its post (26 rows of the texture), a wall sign 12.
    pub wall: bool,
    /// The text's dye (`DyeColor` name).
    pub dye: String,
    pub glowing: bool,
}

impl Sign {
    /// `getTextLineHeight`.
    pub fn line_height(&self) -> i32 {
        if self.hanging { 9 } else { 10 }
    }

    /// `getMaxTextLineWidth`.
    pub fn max_line_width(&self) -> i32 {
        if self.hanging { 60 } else { 90 }
    }
}

/// What the screen edits: the four messages, the line and `TextFieldHelper`'s cursor and selection
/// (byte offsets), and when the caret started blinking (`Util.getMillis()` at `init`).
#[derive(Clone, Debug, Default)]
pub struct SignEdit {
    pub lines: [String; 4],
    pub line: usize,
    pub cursor: usize,
    pub selection: usize,
    pub blink_start: i64,
}

impl SignEdit {
    /// A new screen on `lines`: the cursor at the end of the first.
    pub fn new(lines: [String; 4], now: i64) -> SignEdit {
        let cursor = lines[0].len();
        SignEdit { lines, line: 0, cursor, selection: cursor, blink_start: now }
    }

    fn message(&self) -> &str {
        &self.lines[self.line]
    }

    fn to_end(&mut self, selecting: bool) {
        self.cursor = self.message().len();
        if !selecting {
            self.selection = self.cursor;
        }
    }

    fn step(&self, dir: i32) -> usize {
        let s = self.message();
        let at = self.cursor.min(s.len());
        if dir < 0 { s[..at].char_indices().next_back().map_or(0, |(i, _)| i) } else { s[at..].chars().next().map_or(at, |c| at + c.len_utf8()) }
    }

    fn delete_selection(&mut self) -> String {
        let (a, b) = (self.cursor.min(self.selection), self.cursor.max(self.selection));
        let s = self.message();
        let out = format!("{}{}", &s[..a], &s[b..]);
        self.cursor = a;
        self.selection = a;
        out
    }

    /// `TextFieldHelper.insertText`, kept only when the line still fits the sign.
    pub fn insert(&mut self, m: &dyn Metrics, sign: &Sign, text: &str) {
        let message = if self.selection != self.cursor { self.delete_selection() } else { self.message().to_owned() };
        let at = self.cursor.min(message.len());
        let new = format!("{}{}{}", &message[..at], text, &message[at..]);
        if width_str(m, &new) <= sign.max_line_width() {
            self.cursor = (at + text.len()).min(new.len());
            self.selection = self.cursor;
            self.lines[self.line] = new;
        }
    }

    /// `charTyped`.
    pub fn char_typed(&mut self, m: &dyn Metrics, sign: &Sign, c: char) {
        if crate::java::widget::multiline::allowed(c) {
            self.insert(m, sign, c.encode_utf8(&mut [0; 4]));
        }
    }

    /// `keyPressed`: up and down (and Enter) change line, the rest edit it; true when used.
    pub fn key(&mut self, key: Key, shift: bool) -> bool {
        match key {
            Key::Up | Key::Down | Key::Enter => {
                self.line = if key == Key::Up { (self.line + 3) & 3 } else { (self.line + 1) & 3 };
                self.to_end(false);
            }
            Key::A => {
                self.selection = 0;
                self.cursor = self.message().len();
            }
            Key::Backspace | Key::Delete => {
                if self.message().is_empty() {
                    return true;
                }
                let new = if self.selection != self.cursor {
                    self.delete_selection()
                } else {
                    let other = self.step(if key == Key::Backspace { -1 } else { 1 });
                    let (a, b) = (other.min(self.cursor), other.max(self.cursor));
                    let s = self.message();
                    let new = format!("{}{}", &s[..a], &s[b..]);
                    if key == Key::Backspace {
                        self.cursor = a;
                        self.selection = a;
                    }
                    new
                };
                self.lines[self.line] = new;
            }
            Key::Left | Key::Right => {
                self.cursor = self.step(if key == Key::Left { -1 } else { 1 });
                if !shift {
                    self.selection = self.cursor;
                }
            }
            Key::Home => {
                self.cursor = 0;
                if !shift {
                    self.selection = 0;
                }
            }
            Key::End => self.to_end(shift),
            _ => return false,
        }
        true
    }
}

pub struct SignEditScreen {
    pub env: Env,
    sign: Sign,
    edit: SignEdit,
    widgets: Arena,
}

impl SignEditScreen {
    /// `init()`: the Done button.
    pub fn new(ctx: &Ctx, env: Env, sign: Sign, edit: SignEdit) -> SignEditScreen {
        let mut done = Button::named(ctx, 200, "gui.done");
        (done.base.x, done.base.y) = (env.width / 2 - 100, env.height / 4 + 144);
        SignEditScreen { env, sign, edit, widgets: vec![Box::new(done) as Box<dyn Widget>] }
    }

    fn background_texture(&self, ctx: &mut Ctx) {
        let s = &self.sign;
        let (ops, texture, x, y, h, tex) = if s.hanging {
            (vec![PoseOp::Translate(0.0, -13.0), PoseOp::Scale(4.5, 4.5)], format!("gui/hanging_signs/{}", s.wood), -8, -8, 16, [16, 16])
        } else {
            (vec![PoseOp::Translate(0.0, 27.0), PoseOp::Scale(3.9, 3.9)], format!("gui/signs/{}", s.wood), -12, -13, if s.wall { 12 } else { 26 }, [24, 26])
        };
        let w = tex[0];
        ctx.push(ops);
        ctx.gui.draws.push(GuiDraw::Blit { texture: Cow::Owned(texture), blend: Blend::Textured, x, y, w, h, uv: [0.0, 0.0], src: [w, h], tex, color: 0xFFFF_FFFF });
        ctx.pop();
    }

    /// `extractSignText` in the sign's text space.
    fn text(&self, ctx: &mut Ctx) {
        let (e, m) = (&self.edit, ctx.font);
        let color = text_color(&self.sign.dye, self.sign.glowing);
        let lh = self.sign.line_height();
        let show = (ctx.millis - e.blink_start) / 300 % 2 == 0;
        let mid = 4 * lh / 2;
        let cursor_y = e.line as i32 * lh - mid;
        let x_of = |line: &str, i: usize| width_str(m, &line[..i]) - width_str(m, line) / 2;
        for (i, line) in e.lines.iter().enumerate() {
            ctx.gui.text(Text::literal(line.as_str()), -width_str(m, line) / 2, i as i32 * lh - mid, color, false);
            if i == e.line && e.cursor >= line.len() && show {
                ctx.gui.text(Text::literal("_"), x_of(line, line.len()), cursor_y, color, false);
            }
        }
        let line = &e.lines[e.line];
        if e.cursor < line.len() && show {
            let x = x_of(line, e.cursor);
            ctx.gui.fill(x, cursor_y - 1, x + 1, cursor_y + lh, color | 0xFF00_0000);
        }
        if e.selection != e.cursor {
            let (a, b) = (x_of(line, e.cursor.min(e.selection)), x_of(line, e.cursor.max(e.selection)));
            ctx.gui.highlight(a.min(b), cursor_y, a.max(b), cursor_y + lh, true);
        }
    }

    pub fn edit(&self) -> &SignEdit {
        &self.edit
    }
}

impl Screen for SignEditScreen {
    fn background(&mut self, ctx: &mut Ctx) {
        transparent_background(&self.env, ctx);
    }

    fn foreground(&mut self, ctx: &mut Ctx) {
        render_all(&mut self.widgets, ctx);
        let title = ctx.tr(if self.sign.hanging { "hanging_sign.edit" } else { "sign.edit" });
        ctx.centered_text(title, self.env.width / 2, 40, 0xFFFF_FFFF);
        let (ox, oy) = (self.env.width as f32 / 2.0, if self.sign.hanging { 125.0 } else { 90.0 });
        ctx.push(vec![PoseOp::Translate(ox, oy)]);
        self.background_texture(ctx);
        ctx.pop();
        let scale = if self.sign.hanging { 1.0 } else { 0.976_562_8 };
        ctx.push(vec![PoseOp::Translate(ox, oy), PoseOp::Scale(scale, scale)]);
        self.text(ctx);
        ctx.pop();
    }

    fn children(&mut self) -> Vec<Child<'_>> {
        arena_children(&mut self.widgets)
    }
}
