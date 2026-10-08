//! The HUD and chat over the world, in the theme of the chosen look (acacia-ui).

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use acacia_bot::proto::types::GameMode;
use acacia_render::assets::image_file;
use acacia_render::item::ItemIcons;
use acacia_ui::chat::{self, Chat};
use acacia_ui::hud::{self, HudState, sprite};
use acacia_ui::lang::Lang;
use acacia_ui::theme::{Theme, bedrock, java};
use acacia_ui::{DrawList, Quad, Sprite};

use crate::control::Me;
use crate::looks::Looks;
use crate::settings::LookChoice;

struct Skin {
    theme: Theme,
    icons: ItemIcons,
    root: std::path::PathBuf,
    /// Hotbar icons added to the atlas, by item name and aux; `None` for items without one.
    added: HashMap<(String, u32), Option<Sprite>>,
}

pub struct Ui {
    bedrock: Skin,
    java: Skin,
    pub chat: Chat,
    lang: Lang,
    quads: Vec<Quad>,
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
        let (bedrock, java) = (Skin::new(bedrock_theme, &bedrock_root), Skin::new(java_theme, &java_root));
        Ui { bedrock, java, chat: Chat::default(), lang, quads: Vec::new() }
    }

    /// A chat message, translated; a player's is shown as `<name> text`.
    pub fn push_chat(&mut self, sender: Option<&str>, message: &str, params: &[String]) {
        let text = self.lang.translate(message, params);
        let line = sender.map_or_else(|| text.clone(), |s| format!("<{s}> {text}"));
        self.chat.push(line, Instant::now());
    }

    /// This frame's quads for a window `size` pixels big at GUI `scale`, and the atlas they sample.
    /// `debug` is the debug screen's two columns, when it is shown.
    pub fn draw(&mut self, look: LookChoice, me: Option<&Me>, debug: Option<(Vec<String>, Vec<String>)>, size: [u32; 2], scale: u32, now: Instant) -> (&acacia_ui::Atlas, &[Quad]) {
        let skin = match look {
            LookChoice::Bedrock => &mut self.bedrock,
            LookChoice::Java => &mut self.java,
        };
        let mut list = DrawList::new(scale as f32);
        let gui = [(size[0] / scale) as f32, (size[1] / scale) as f32];
        if let Some(me) = me {
            let state = skin.state(me);
            hud::draw(&mut list, &skin.theme, &state, gui);
        }
        chat::draw(&mut list, &skin.theme, &self.chat, now, gui);
        if let Some((left, right)) = debug {
            acacia_ui::debug::draw(&mut list, &skin.theme, &left, &right, gui[0]);
        }
        self.quads = list.quads;
        (&skin.theme.atlas, &self.quads)
    }
}

impl Skin {
    fn new(theme: Theme, root: &Path) -> Skin {
        Skin { theme, icons: ItemIcons::load(root), root: root.to_owned(), added: HashMap::new() }
    }

    fn state(&mut self, me: &Me) -> HudState {
        let hotbar = std::array::from_fn(|slot| {
            let stack = me.items[slot].as_ref()?;
            Some((self.icon(&stack.name, stack.aux)?, stack.count))
        });
        HudState {
            health: me.health,
            max_health: me.max_health,
            food: me.food,
            armor: 0,
            air: None,
            xp_level: me.xp_level.max(0) as u32,
            xp_progress: me.xp_progress,
            selected: me.hotbar,
            hotbar,
            survival: matches!(me.game_mode, GameMode::Survival | GameMode::Adventure),
        }
    }

    /// The item's icon (its first frame) in the atlas, added on first use.
    fn icon(&mut self, name: &str, aux: u32) -> Option<Sprite> {
        let key = (name.to_owned(), aux);
        if let Some(sprite) = self.added.get(&key) {
            return *sprite;
        }
        let image = self.icons.path(name, aux).and_then(|p| image_file(&self.root, p)).and_then(|f| image::open(f).ok()).map(|i| i.to_rgba8());
        let sprite = image.map(|i| {
            let side = i.width().min(i.height());
            let frame = image::imageops::crop_imm(&i, 0, 0, side, side).to_image();
            self.theme.atlas.add(&format!("item/{name}/{aux}"), &frame)
        });
        self.added.insert(key, sprite);
        sprite
    }
}
