//! `WinScreen`: the credits roll (and, after the dragon, the end poem around it) scrolling up over
//! the logo, with the credits vignette on top.

use serde_json::Value;

use super::{Env, Screen, blit, blurred, panorama};
use crate::java::gui::{Blend, GuiDraw, PoseOp};
use crate::java::mth::JavaRandom;
use crate::java::picture::Picture;
use crate::java::text::{Style, Text};
use crate::java::widget::Ctx;
use crate::java::widget::measure::split;

const YELLOW: u32 = 0xFF_FF55;
const GRAY: u32 = 0xAA_AAAA;
const WHITE: u32 = 0xFF_FFFF;
const OBFUSCATE_TOKEN: &str = "§f§k§a§b";

/// The roll's lines: each with whether it is centred.
#[derive(Clone, Debug, Default)]
pub struct Lines(pub Vec<(Text, bool)>);

impl Lines {
    fn empty(&mut self) {
        self.0.push((Text::default(), false));
    }

    fn add(&mut self, t: Text, centered: bool) {
        self.0.push((t, centered));
    }

    /// `addPoemFile`: each line split to 256 then a blank, the player's name in, the obfuscated
    /// token swapped for 3 to 6 obfuscated Xs from a fixed seed; 8 blanks after.
    pub fn poem(&mut self, ctx: &Ctx, file: &str, player: &str) {
        let mut random = JavaRandom::new(8_124_371);
        for line in file.lines() {
            let mut line = line.replace("PLAYERNAME", player);
            while let Some(pos) = line.find(OBFUSCATE_TOKEN) {
                let n = random.next_int_bound(4) as usize + 3;
                line = format!("{}§f§k{}{}", &line[..pos], &"XXXXXXXX"[..n], &line[pos + OBFUSCATE_TOKEN.len()..]);
            }
            for l in split(ctx.font, &Text::literal(line), 256) {
                self.add(l, false);
            }
            self.empty();
        }
        for _ in 0..8 {
            self.empty();
        }
    }

    /// `addCreditsFile`: each section's heading, its disciplines and their titles with the names.
    pub fn credits(&mut self, json: &Value) {
        let heading = || Text::styled("============", &Style::color(WHITE));
        let s = |v: &Value| v.as_str().unwrap_or_default().to_owned();
        for section in json.as_array().into_iter().flatten() {
            self.add(heading(), true);
            self.add(Text::styled(s(&section["section"]), &Style::color(YELLOW)), true);
            self.add(heading(), true);
            self.empty();
            self.empty();
            for discipline in section["disciplines"].as_array().into_iter().flatten() {
                let name = s(&discipline["discipline"]);
                if !name.is_empty() {
                    self.add(Text::styled(name, &Style::color(YELLOW)), true);
                    self.empty();
                    self.empty();
                }
                for title in discipline["titles"].as_array().into_iter().flatten() {
                    self.add(Text::styled(s(&title["title"]), &Style::color(GRAY)), false);
                    for name in title["names"].as_array().into_iter().flatten() {
                        self.add(Text::literal("           ").append(s(name)).with_style(&Style::color(WHITE)), false);
                    }
                    self.empty();
                    self.empty();
                }
            }
        }
    }
}

/// `init`'s lines: the poem (`end.txt`), the credits (`credits.json`) and the post-credits poem.
pub fn lines(ctx: &Ctx, poem: Option<(&str, &str, &str)>, credits: &Value) -> Lines {
    let mut out = Lines::default();
    if let Some((end, _, player)) = poem {
        out.poem(ctx, end, player);
    }
    out.credits(credits);
    if let Some((_, post, player)) = poem {
        out.poem(ctx, post, player);
    }
    out
}

pub struct WinScreen {
    pub env: Env,
    pub poem: bool,
    pub lines: Lines,
    pub scroll: f32,
    pub speed: f32,
    /// The frame's partial tick, which advances the scroll as the frame is drawn.
    pub partial: f32,
}

/// `calculateScrollSpeed`: Space speeds the roll up 5 times, each Ctrl held 15 times more; Up reverses it.
pub fn scroll_speed(poem: bool, speedup: bool, modifiers: usize, reverse: bool) -> f32 {
    let base = if poem { 0.5 } else { 0.75 };
    let direction = if reverse { -1.0 } else { 1.0 };
    if speedup { base * (5.0 + modifiers as f32 * 15.0) * direction } else { base * direction }
}

impl WinScreen {
    pub fn new(env: Env, poem: bool, lines: Lines) -> WinScreen {
        WinScreen { env, poem, lines, scroll: 0.0, speed: scroll_speed(poem, false, 0, false), partial: 0.0 }
    }

    /// `tick`: whether the roll is over (the owner then respawns or closes it).
    pub fn finished(&self) -> bool {
        self.scroll > (self.lines.0.len() as i32 * 12 + self.env.height * 2 + 24) as f32
    }
}

impl Screen for WinScreen {
    fn background(&mut self, ctx: &mut Ctx) {
        let (w, h) = (self.env.width, self.env.height);
        if self.poem {
            ctx.gui.draws.push(GuiDraw::Picture(Picture::EndPortal { rect: [0, 0, w, h] }));
            return;
        }
        if !self.env.in_world {
            panorama(&self.env, ctx);
        }
        blurred(&self.env, ctx);
        blit(ctx, "gui/menu_background", Blend::Textured, [0, 0, w, h], [0.0, self.scroll * 0.5], [w, h], [32, 32]);
    }

    fn foreground(&mut self, ctx: &mut Ctx) {
        let (w, h) = (self.env.width, self.env.height);
        blit(ctx, "misc/credits_vignette", Blend::Vignette, [0, 0, w, h], [0.0, 0.0], [w, h], [w, h]);
        self.scroll = (self.scroll + self.partial * self.speed).max(0.0);
        let (logo_x, logo_y) = (w / 2 - 128, h + 50);
        let y_offs = -self.scroll;
        ctx.push(vec![PoseOp::Translate(0.0, y_offs)]);
        ctx.gui.next_stratum();
        blit(ctx, "gui/title/minecraft", Blend::Textured, [logo_x, logo_y, 256, 44], [0.0, 0.0], [256, 44], [256, 64]);
        blit(ctx, "gui/title/edition", Blend::Textured, [w / 2 - 64, logo_y + 44 - 7, 128, 14], [0.0, 0.0], [128, 14], [128, 16]);
        let mut y = logo_y + 100;
        let count = self.lines.0.len();
        for (i, (line, centered)) in self.lines.0.iter().enumerate() {
            if i == count - 1 {
                let diff = y as f32 + y_offs - (h / 2 - 6) as f32;
                if diff < 0.0 {
                    // Java translates the pushed pose again: the same floats as one push of both.
                    ctx.pop();
                    ctx.push(vec![PoseOp::Translate(0.0, y_offs), PoseOp::Translate(0.0, -diff)]);
                }
            }
            if y as f32 + y_offs + 12.0 + 8.0 > 0.0 && (y as f32 + y_offs) < h as f32 {
                if *centered {
                    ctx.centered_text(line.clone(), logo_x + 128, y, 0xFFFF_FFFF);
                } else {
                    ctx.gui.text(line.clone(), logo_x, y, 0xFFFF_FFFF, true);
                }
            }
            y += 12;
        }
        ctx.pop();
    }
}
