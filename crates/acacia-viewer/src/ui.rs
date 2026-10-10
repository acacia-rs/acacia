//! The HUD and chat over the world, in the theme of the chosen look (acacia-ui).

mod screen;

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use acacia_bot::proto::types::GameMode;
use acacia_render::assets::image_file;
use std::sync::Arc;

use acacia_render::LookPack;
use acacia_render::blocks::BlockTable;
use acacia_render::item::{ItemIcons, banner_icon, block_icon, pattern_icon};
use acacia_ui::chat::{self, Chat};
use acacia_ui::effects::Active;
use acacia_render::glint::Foil;
use acacia_ui::hud::{self, HudState, Item, sprite};
use acacia_ui::lang::Lang;
use acacia_ui::overlay::{self, Boss, Titles};
use acacia_bot::events::{Title, TitleKind};
use acacia_ui::theme::{Theme, bedrock, java};
use acacia_ui::{DrawList, Quad, Sprite};

use crate::control::{Inventory, Stack};
use crate::me::Me;
use acacia_ui::inventory::Layout;
use acacia_ui::menu::Backdrop;

use crate::forms::FormScreen;
use crate::looks::Looks;
use crate::settings::LookChoice;

struct Skin {
    theme: Theme,
    icons: ItemIcons,
    root: std::path::PathBuf,
    /// Hotbar icons added to the atlas, by item name and aux; `None` for items without one.
    added: HashMap<(String, u32), Option<Sprite>>,
    /// The shown world's blocks in this look, for block items' icons.
    blocks: Option<(Arc<LookPack>, Arc<BlockTable>)>,
    /// Bumped per world: atlas names of block icons carry it, as runtime ids change meaning.
    generation: u32,
    foil: Foil,
}

/// What one frame's UI shows, over a window `size` pixels big at GUI `scale`.
pub struct Frame<'a> {
    pub look: LookChoice,
    pub me: Option<&'a Me>,
    /// The debug screen's two columns, when it is shown.
    pub debug: Option<(Vec<String>, Vec<String>)>,
    pub screen: Option<Screen<'a>>,
    /// An open menu: its title, button labels and what it is drawn over.
    pub menu: Option<(&'a str, &'a [String], Backdrop)>,
    /// An open server form.
    pub form: Option<&'a FormScreen>,
    pub sign: Option<acacia_ui::sign_editor::Editing<'a>>,
    pub book: Option<acacia_ui::book::Page<'a>>,
    /// The player list, while Tab is held.
    pub players: Option<&'a [String]>,
    /// Name tags, projected onto the screen.
    pub tags: Vec<acacia_ui::nametags::Tag>,
    /// GUI pixels.
    pub mouse: [f32; 2],
    pub size: [u32; 2],
    pub scale: u32,
    pub now: Instant,
}

/// An open inventory screen.
#[derive(Clone, Copy)]
pub struct Screen<'a> {
    pub inventory: &'a Inventory,
    pub layout: Layout,
    /// What is picked from its list, and the first entry the list shows.
    pub picked: [Option<usize>; 2],
    pub first: usize,
    /// The text in its name box, before the caret and after.
    pub name: Option<(&'a str, &'a str)>,
    /// The creative inventory's shown items, its tab, and how far it is scrolled.
    pub creative: Option<(&'a [crate::control::Stack], usize, f32)>,
}

pub struct Ui {
    bedrock: Skin,
    java: Skin,
    pub chat: Chat,
    titles: Titles,
    /// The scoreboard objective in the sidebar slot.
    pub sidebar: Option<acacia_ui::sidebar::Sidebar>,
    lang: Lang,
    quads: Vec<Quad>,
    /// The hotbar slot and item last seen selected: a change shows the item's name.
    selected: Option<(u8, String)>,
    /// When each effect (as the server sent it) was first seen, to count its time down.
    effects_seen: HashMap<(i32, i32, u64), Instant>,
}

impl Ui {
    /// The Java look's files lend the Bedrock theme their font; a Java look without HUD sprites of
    /// its own (not baked) draws Bedrock's, which are the same pixels.
    pub fn new(looks: &Looks) -> Ui {
        let (bedrock_root, java_root) = (looks.get(LookChoice::Bedrock).files().to_owned(), looks.get(LookChoice::Java).files().to_owned());
        let font_root = java_root.join("textures/font/ascii.png").is_file().then_some(java_root.as_path());
        let bedrock_theme = bedrock::load(&bedrock_root, font_root);
        let java_theme = java::load(&java_root);
        let java_theme = if java_theme.atlas.get(sprite::HOTBAR).is_some() {
            java_theme
        } else {
            Theme { style: java::STYLE, ..bedrock::load(&bedrock_root, font_root) }
        };
        tracing::info!(font = font_root.is_some(), "ui themes");
        let lang = Lang::load(&bedrock_root.join("texts/en_US.lang"));
        let foil = |look| looks.get(look).look.foil;
        let (bedrock, java) = (Skin::new(bedrock_theme, &bedrock_root, foil(LookChoice::Bedrock)), Skin::new(java_theme, &java_root, foil(LookChoice::Java)));
        Ui { bedrock, java, chat: Chat::default(), titles: Titles::default(), sidebar: None, lang, quads: Vec::new(), selected: None, effects_seen: HashMap::new() }
    }

    pub fn show_title(&mut self, title: Title) {
        let (text, now) = (self.lang.translate(&title.text, &[]), Instant::now());
        match title.kind {
            TitleKind::Title => self.titles.title(text, now),
            TitleKind::Subtitle => self.titles.subtitle(text),
            TitleKind::ActionBar => self.titles.action_bar(text, now),
            TitleKind::Clear => self.titles.clear(),
        }
    }

    /// The blocks of the world now shown in `look`, for block items' icons.
    pub fn set_world(&mut self, look: LookChoice, pack: Arc<LookPack>, table: Arc<BlockTable>) {
        let skin = self.skin(look);
        skin.blocks = Some((pack, table));
        skin.generation += 1;
        skin.added.clear();
    }

    pub fn theme(&self, look: LookChoice) -> &Theme {
        match look {
            LookChoice::Bedrock => &self.bedrock.theme,
            LookChoice::Java => &self.java.theme,
        }
    }

    pub fn theme_mut(&mut self, look: LookChoice) -> &mut Theme {
        &mut self.skin(look).theme
    }

    fn skin(&mut self, look: LookChoice) -> &mut Skin {
        match look {
            LookChoice::Bedrock => &mut self.bedrock,
            LookChoice::Java => &mut self.java,
        }
    }

    /// A chat message, translated; a player's is shown as `<name> text`.
    /// `rawtext` is the message as JSON when the server sent it so: translated here, where the
    /// language files are, in place of `message` (the bot's flattening, keys untranslated).
    pub fn push_chat(&mut self, sender: Option<&str>, message: &str, params: &[String], rawtext: Option<&str>) {
        tracing::debug!(target: "chat", message, ?params, rawtext, "raw");
        let text = rawtext.and_then(|json| self.lang.rawtext(json)).unwrap_or_else(|| self.lang.translate(message, params));
        let line = sender.map_or_else(|| text.clone(), |s| format!("<{s}> {text}"));
        tracing::info!(target: "chat", "{line}");
        self.chat.push(line, Instant::now());
    }

    /// This frame's quads for a window `size` pixels big at GUI `scale`, and the atlas they sample.
    /// This frame's quads and the atlas they sample.
    pub fn draw(&mut self, frame: Frame) -> (&acacia_ui::Atlas, &[Quad]) {
        let Frame { look, me, debug, screen, menu, form, sign, book, players, tags, mouse, size, scale, now } = frame;
        let skin = match look {
            LookChoice::Bedrock => &mut self.bedrock,
            LookChoice::Java => &mut self.java,
        };
        let mut list = DrawList::new(scale as f32);
        let gui = [(size[0] / scale) as f32, (size[1] / scale) as f32];
        // In the world, so under the HUD.
        acacia_ui::nametags::draw(&mut list, &skin.theme, &tags);
        if let Some(me) = me {
            let state = HudState { crosshair: screen.is_none() && menu.is_none() && form.is_none() && sign.is_none() && book.is_none(), ..skin.state(me) };
            hud::draw(&mut list, &skin.theme, &state, gui);
            let selected = me.items[me.hotbar as usize % 9].as_ref().map(|stack| (me.hotbar, stack.name.clone()));
            if selected != self.selected {
                if let Some((_, name)) = &selected {
                    self.titles.item_name(self.lang.item_name(name), now, state.survival);
                }
                self.selected = selected;
            }
            self.effects_seen.retain(|key, _| me.effects.iter().any(|&(id, ticks, tick, _)| *key == (id, ticks, tick)));
            let active: Vec<Active> = me
                .effects
                .iter()
                .map(|&(id, ticks, tick, ambient)| {
                    let seen = *self.effects_seen.entry((id, ticks, tick)).or_insert(now);
                    let elapsed = (now.saturating_duration_since(seen).as_secs_f32() * 20.0) as i32;
                    Active { id, ticks: if ticks < 0 { ticks } else { (ticks - elapsed).max(0) }, ambient }
                })
                .collect();
            acacia_ui::effects::draw(&mut list, &skin.theme, &active, gui[0]);
            // A vanilla boss's title is its name's key (`entity.ender_dragon.name`).
            let titles: Vec<String> = me.bosses.iter().map(|(title, ..)| self.lang.translate(title, &[])).collect();
            let bosses: Vec<Boss> = me.bosses.iter().zip(&titles).map(|((_, progress, colour), title)| Boss { title, progress: *progress, colour: *colour }).collect();
            overlay::draw_bosses(&mut list, &skin.theme, &bosses, gui);
        }
        if let Some(sidebar) = &self.sidebar {
            acacia_ui::sidebar::draw(&mut list, &skin.theme, sidebar, gui);
        }
        overlay::draw_titles(&mut list, &skin.theme, &self.titles, now, gui);
        chat::draw(&mut list, &skin.theme, &self.chat, now, gui);
        if let Some((left, right)) = debug {
            acacia_ui::debug::draw(&mut list, &skin.theme, &left, &right, gui[0]);
        }
        if let Some(screen) = screen {
            screen::draw(&mut list, skin, &self.lang, &screen, mouse, gui);
        }
        if let Some(names) = players {
            acacia_ui::players::draw(&mut list, &skin.theme, names, gui[0]);
        }
        if let Some(form) = form {
            form.draw(&mut list, &skin.theme, now);
        }
        if let Some(page) = book {
            acacia_ui::book::draw(&mut list, &skin.theme, page, mouse, gui);
        }
        if let Some(sign) = sign {
            acacia_ui::sign_editor::draw(&mut list, &skin.theme, sign, mouse, gui);
        }
        if let Some((title, buttons, backdrop)) = menu {
            acacia_ui::menu::draw(&mut list, &skin.theme, title, buttons, backdrop, mouse, gui);
        }
        self.quads = list.quads;
        (&skin.theme.atlas, &self.quads)
    }
}

impl Skin {
    fn new(theme: Theme, root: &Path, foil: Foil) -> Skin {
        Skin { theme, icons: ItemIcons::load(root), root: root.to_owned(), added: HashMap::new(), blocks: None, generation: 0, foil }
    }

    /// A stack as a slot shows it; `None` for an item without an icon.
    fn item(&mut self, s: &Stack) -> Option<Item> {
        Some(Item { icon: self.icon(&s.name, s.aux, s.block)?, count: s.count, glint: s.enchanted || self.foil.always(&s.name) })
    }

    fn state(&mut self, me: &Me) -> HudState {
        let hotbar = std::array::from_fn(|slot| self.item(me.items[slot].as_ref()?));
        HudState {
            health: me.health,
            max_health: me.max_health,
            food: me.food,
            armor: me.armor,
            air: me.air,
            xp_level: me.xp_level.max(0) as u32,
            xp_progress: me.xp_progress,
            selected: me.hotbar,
            hotbar,
            survival: matches!(me.game_mode, GameMode::Survival | GameMode::Adventure),
            crosshair: true,
        }
    }

    /// A loom pattern's button image, added on first use.
    fn pattern(&mut self, code: &str) -> Option<Sprite> {
        let key = (format!("pattern/{code}"), 0);
        if let Some(sprite) = self.added.get(&key) {
            return *sprite;
        }
        let image = pattern_icon(&self.root, code).or_else(|| pattern_icon(&acacia_render::assets::Pack::default_dir(), code));
        let sprite = image.map(|i| self.theme.atlas.add(&key.0, &i));
        self.added.insert(key, sprite);
        sprite
    }

    /// The item's icon (its first frame) in the atlas, added on first use; a block item without one
    /// shows its block.
    fn icon(&mut self, name: &str, aux: u32, block: u32) -> Option<Sprite> {
        let key = (name.to_owned(), aux);
        if let Some(sprite) = self.added.get(&key) {
            return *sprite;
        }
        let image = self.icons.path(name, aux).and_then(|p| image_file(&self.root, p)).and_then(|f| image::open(f).ok()).map(|i| i.to_rgba8());
        let banner = || banner_icon(&self.root, aux).or_else(|| banner_icon(&acacia_render::assets::Pack::default_dir(), aux));
        let image = image.or_else(|| (name == "minecraft:banner").then(banner).flatten());
        let sprite = match image {
            Some(i) => {
                let side = i.width().min(i.height());
                let frame = image::imageops::crop_imm(&i, 0, 0, side, side).to_image();
                Some(self.theme.atlas.add(&format!("item/{name}/{aux}"), &frame))
            }
            None => self.blocks.as_ref().filter(|_| block != 0).and_then(|(pack, table)| block_icon(table.get(block), &pack.atlas)).map(|icon| {
                self.theme.atlas.add(&format!("block/{}/{block}", self.generation), &icon)
            }),
        };
        self.added.insert(key, sprite);
        sprite
    }
}
