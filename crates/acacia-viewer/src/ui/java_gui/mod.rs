//! The Java look's GUI as the game draws it: `acacia_ui::java` layouts emit the game's draw calls,
//! painted here with the pack's sprites and fonts in the game's order (`acacia_ui::java::paint`).

mod hud;
pub mod menus;
mod options;

pub use hud::hotbar_stack;

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use acacia_ui::java::font::{FontOptions, Fonts, GlyphTexture, Pen, Resources};
use acacia_ui::java::gui::{Gui, GuiDraw, Metrics};
use acacia_ui::java::hud::health::HealthHud;
use acacia_ui::java::mth::JavaRandom;
use acacia_ui::java::paint::assets::Images;
use acacia_ui::java::paint::blit::Scaling;
use acacia_ui::java::paint::{self, Glyphs, Source};
use acacia_ui::java::text::Text;
use acacia_ui::{Atlas, DrawList, Quad, Sprite};

use super::Skin;
use crate::control::Stack;

pub struct JavaGui {
    images: Images,
    pub fonts: Fonts,
    /// `Font.random`, for obfuscated text.
    random: JavaRandom,
    health: HealthHud,
    started: Instant,
    /// The game's `en_us` translations.
    lang: HashMap<String, String>,
}

impl JavaGui {
    /// The Java look's files at `root` (the jar's `assets/minecraft` layout).
    pub fn new(root: &Path) -> JavaGui {
        let fonts = Fonts::load(&Resources::dir("minecraft", root), FontOptions::default());
        for w in &fonts.warnings {
            tracing::debug!(target: "ui", "font: {w}");
        }
        let lang = acacia_ui::java::lang::load(root);
        if lang.is_empty() {
            tracing::warn!(looked = %root.display(), "no lang/en_us.json, so screens show translation keys: bake the Java look again (tools/lookbake)");
        }
        JavaGui { images: Images::new(root), fonts, random: JavaRandom::new(0), health: HealthHud::default(), started: Instant::now(), lang }
    }

    /// Whether the pack has the GUI's sprites.
    pub fn usable(&mut self, skin: &mut Skin) -> bool {
        self.images.sprite(&mut skin.theme.atlas, "hud/hotbar").is_some()
    }

    /// `Util.getMillis()`: milliseconds since the GUI was made.
    pub fn millis(&self, now: Instant) -> i64 {
        now.saturating_duration_since(self.started).as_millis() as i64
    }

    /// `gui`'s calls as quads into `list`; `stacks` names the stack behind each item call's key.
    pub fn paint(&mut self, skin: &mut Skin, stacks: &dyn Fn(i32) -> Option<Stack>, gui: &Gui, list: &mut DrawList) {
        let mut src = Src { skin, images: &mut self.images, fonts: &self.fonts, random: &mut self.random, stacks, started: self.started };
        let painted = paint::paint(&gui.draws, [gui.width, gui.height], list.scale, &mut src);
        list.quads.extend(painted.quads);
    }
}

/// `Window.getGuiScaledWidth` and height: rounded up.
pub fn gui_size(size: [u32; 2], scale: u32) -> [i32; 2] {
    [size[0].div_ceil(scale) as i32, size[1].div_ceil(scale) as i32]
}

struct Src<'a> {
    skin: &'a mut Skin,
    images: &'a mut Images,
    fonts: &'a Fonts,
    random: &'a mut JavaRandom,
    stacks: &'a dyn Fn(i32) -> Option<Stack>,
    started: Instant,
}

/// A glyph texture in the atlas, packed on first use.
fn glyph_sprite(atlas: &mut Atlas, fonts: &Fonts, texture: &GlyphTexture) -> Option<Sprite> {
    let name = format!("font/{texture:?}");
    if let Some(s) = atlas.get(&name) {
        return Some(s);
    }
    let (w, h, texels) = fonts.texels(texture)?;
    Some(atlas.add(&name, &image::RgbaImage::from_raw(w, h, texels)?))
}

impl Source for Src<'_> {
    fn sprite(&mut self, id: &str) -> Option<(Sprite, Scaling)> {
        self.images.sprite(&mut self.skin.theme.atlas, id)
    }

    fn block_sprite(&mut self, id: &str) -> Option<Sprite> {
        self.images.block_sprite(&mut self.skin.theme.atlas, id)
    }

    fn texture(&mut self, path: &str) -> Option<Sprite> {
        self.images.texture(&mut self.skin.theme.atlas, path)
    }

    fn white(&mut self) -> Sprite {
        self.skin.theme.atlas.white()
    }

    fn text(&mut self, text: &Text, x: i32, y: i32, color: u32, shadow: bool) -> Glyphs {
        let prepared = self.fonts.prepare(text, Pen::gui(x as f32, y as f32, color, shadow), self.random);
        let (atlas, fonts) = (&mut self.skin.theme.atlas, self.fonts);
        prepared.glyphs(|t| glyph_sprite(atlas, fonts, t))
    }

    fn width(&mut self, text: &Text) -> i32 {
        Metrics::width(self.fonts, text)
    }

    fn millis(&self) -> i64 {
        self.started.elapsed().as_millis() as i64
    }

    fn item(&mut self, key: i32, x: i32, y: i32, _: i32) -> Vec<Quad> {
        let Some(stack) = (self.stacks)(key) else { return Vec::new() };
        let Some(item) = self.skin.item(&stack) else { return Vec::new() };
        let mut list = DrawList::new(1.0);
        list.icon(item.icon, [x as f32, y as f32, x as f32 + 16.0, y as f32 + 16.0], item.glint);
        list.quads
    }

    /// `itemDecorations`: the count (bars and cooldowns are not known here).
    fn decorations(&mut self, key: i32, x: i32, y: i32, count: Option<&str>) -> Vec<GuiDraw> {
        let Some(stack) = (self.stacks)(key) else { return Vec::new() };
        if stack.count == 1 && count.is_none() {
            return Vec::new();
        }
        let amount = count.map_or_else(|| stack.count.to_string(), str::to_owned);
        let width = Metrics::width(self.fonts, &Text::literal(amount.as_str()));
        vec![GuiDraw::Text { text: Text::literal(amount), x: x + 19 - 2 - width, y: y + 6 + 3, color: 0xFFFF_FFFF, shadow: true }]
    }
}
