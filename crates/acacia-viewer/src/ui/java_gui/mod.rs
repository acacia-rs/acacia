//! The Java look's GUI as the game draws it: `acacia_ui::java` layouts emit the game's draw calls,
//! painted here with the pack's sprites and fonts in the game's order (`acacia_ui::java::paint`).

mod beside;
mod hud;
mod inputs;
mod items;
mod map;
pub mod menus;
mod options;
mod picture;
pub mod screens;
mod views;
#[cfg(test)]
mod views_tests;

pub use hud::{HudView, hotbar_stack};
pub use inputs::screen_info;
pub use picture::PictureRequest;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use acacia_ui::java::font::{FontOptions, Fonts, GlyphTexture, Pen, Resources};
use acacia_ui::java::gui::{Gui, GuiDraw, Metrics};
use acacia_ui::java::hud::decor;
use acacia_ui::java::hud::frame::Hud;
use acacia_ui::java::mth::JavaRandom;
use acacia_ui::java::paint::assets::Images;
use acacia_ui::java::paint::blit::Scaling;
use acacia_ui::java::paint::{self, Glyphs, Source};
use acacia_ui::java::picture::Picture;
use acacia_ui::java::screen::win::Lines;
use acacia_ui::java::text::Text;
use acacia_ui::{Atlas, DrawList, Quad, Sprite};

use super::Skin;
use crate::control::Stack;

pub struct JavaGui {
    root: PathBuf,
    images: Images,
    pub fonts: Fonts,
    /// `Font.random`, for obfuscated text.
    random: JavaRandom,
    hud: Hud,
    /// The game ticks run so far, and `Util.getMillis()` at the last HUD frame.
    ticks: i64,
    last_frame: i64,
    started: Instant,
    /// The game's `en_us` translations.
    lang: HashMap<String, String>,
    /// The registry facts the screens' tooltips read.
    game: acacia_ui::java::tooltip::game::Game,
    /// The credits roll's lines from `texts/credits.json`, read when it first opens.
    credits: std::cell::OnceCell<Lines>,
    map: map::MapPicture,
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
        JavaGui { root: root.to_owned(), images: Images::new(root), fonts, random: JavaRandom::new(0), hud: Hud::default(), ticks: 0, last_frame: 0, started: Instant::now(), lang, game: items::facts(), credits: Default::default(), map: Default::default() }
    }

    /// The screens' environment: the fonts, the translations, the registry facts.
    fn with_env<R>(&self, f: impl FnOnce(&acacia_ui::java::container::Env) -> R) -> R {
        let lang = |key: &str| self.lang.get(key).cloned();
        f(&acacia_ui::java::container::Env { font: &self.fonts, lang: &lang, advanced: false, game: &self.game, op: false })
    }

    /// [`JavaGui::credits`]' lines.
    pub fn credits(&self) -> &Lines {
        self.credits.get_or_init(|| {
            let json = std::fs::read_to_string(self.root.join("texts/credits.json")).ok().and_then(|s| serde_json::from_str(&s).ok());
            if json.is_none() {
                tracing::warn!(looked = %self.root.display(), "no texts/credits.json, so the credits roll is empty: bake the Java look again (tools/lookbake)");
            }
            let mut lines = Lines::default();
            lines.credits(&json.unwrap_or_default());
            lines
        })
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
    /// Entities to draw as pictures go to `pictures`.
    pub fn paint(&mut self, skin: &mut Skin, stacks: &dyn Fn(i32) -> Option<Stack>, gui: &Gui, list: &mut DrawList, pictures: &mut Vec<PictureRequest>) {
        let mut src = Src { skin, root: &self.root, images: &mut self.images, fonts: &self.fonts, random: &mut self.random, stacks, started: self.started, pictures, gui_scale: list.scale, map: &mut self.map };
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
    root: &'a Path,
    images: &'a mut Images,
    fonts: &'a Fonts,
    random: &'a mut JavaRandom,
    stacks: &'a dyn Fn(i32) -> Option<Stack>,
    started: Instant,
    pictures: &'a mut Vec<PictureRequest>,
    gui_scale: f32,
    map: &'a mut map::MapPicture,
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
        if path == map::TEXTURE {
            return self.map.sprite(&self.skin.theme.atlas);
        }
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

    /// `itemDecorations`: the count and the damage bar (cooldowns are not tracked).
    fn decorations(&mut self, key: i32, x: i32, y: i32, count: Option<&str>) -> Vec<GuiDraw> {
        let Some(stack) = (self.stacks)(key) else { return Vec::new() };
        let mut gui = Gui::new(0, 0);
        decor::item_decorations(&mut gui, self.fonts, x, y, &items::decorations(&stack, key, count));
        gui.draws
    }

    fn atlas_sprite(&mut self, _: &str, sprite: &str) -> Option<Sprite> {
        picture::atlas_sprite(self.images, &mut self.skin.theme.atlas, sprite)
    }

    fn picture(&mut self, p: &Picture, window: [f32; 4]) -> Option<Quad> {
        picture::quad(&mut self.skin.theme.atlas, self.root, self.pictures, self.gui_scale, p, window)
    }

    // TODO: the bundle tooltip's contents.
    fn picture_draws(&mut self, p: &Picture) -> Vec<GuiDraw> {
        let Picture::Map { key: Some(key), .. } = p else { return Vec::new() };
        let Some(picture) = (self.stacks)(*key).and_then(|s| s.map) else { return Vec::new() };
        self.map.show(&mut self.skin.theme.atlas, self.root, &picture);
        map::draws()
    }
}
