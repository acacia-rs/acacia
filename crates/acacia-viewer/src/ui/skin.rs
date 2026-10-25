//! One look's UI theme and item icons, added to its atlas on first use.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use acacia_bot::proto::types::GameMode;
use acacia_render::LookPack;
use acacia_render::blocks::BlockTable;
use acacia_render::glint::Foil;
use acacia_render::item::{ItemIcons, JavaItems, block_icon, java_icon, leather, model_icon, pattern_icon};
use acacia_ui::Sprite;
use acacia_ui::hud::{HudState, Item};
use acacia_ui::theme::Theme;

use crate::control::Stack;
use crate::me::Me;

pub(super) struct Skin {
    pub(super) theme: Theme,
    icons: ItemIcons,
    /// A Java look's own items, drawn as its GUI draws them.
    pub(super) java_items: Option<Arc<JavaItems>>,
    pub(super) root: std::path::PathBuf,
    /// Hotbar icons added to the atlas, by item name and aux; `None` for items without one.
    pub(super) added: HashMap<(String, u32), Option<Sprite>>,
    /// The shown world's blocks in this look, for block items' icons.
    pub(super) blocks: Option<(Arc<LookPack>, Arc<BlockTable>)>,
    /// Bumped per world: atlas names of block icons carry it, as runtime ids change meaning.
    pub(super) generation: u32,
    pub(super) foil: Foil,
}

impl Skin {
    pub(super) fn new(theme: Theme, root: &Path, foil: Foil) -> Skin {
        Skin { theme, icons: ItemIcons::load(root), java_items: JavaItems::shared(root), root: root.to_owned(), added: HashMap::new(), blocks: None, generation: 0, foil }
    }

    /// A stack as a slot shows it; `None` for an item without an icon.
    pub(super) fn item(&mut self, s: &Stack) -> Option<Item> {
        Some(Item { icon: self.icon(s)?, count: s.count, glint: s.enchanted || self.foil.always(&s.name) })
    }

    pub(super) fn state(&mut self, me: &Me) -> HudState {
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
    pub(super) fn pattern(&mut self, code: &str) -> Option<Sprite> {
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
    pub(super) fn icon(&mut self, s: &Stack) -> Option<Sprite> {
        let (name, aux, block) = (s.name.as_str(), s.aux, s.block);
        let dyed = leather::dyeable(name).then_some(s.dye);
        let (banner, key) = (s.banner.as_deref(), (format!("{name}/{}", crate::item_look::variant(dyed.flatten(), s.banner.as_deref())), aux));
        if let Some(sprite) = self.added.get(&key) {
            return *sprite;
        }
        let java = self.java_items.as_deref().filter(|_| banner.is_none()).and_then(|items| java_icon::icon(items, name, aux, acacia_render::item::first_tint(name, aux, s.dye)));
        let model = java.or_else(|| model_icon(&self.root, name, banner)).or_else(|| model_icon(&acacia_render::assets::Pack::default_dir(), name, banner));
        let image = model.or_else(|| self.icons.path(name, aux).and_then(|p| leather::icon(&self.root, p, dyed)));
        let sprite = match image {
            Some(i) => {
                let side = i.width().min(i.height());
                let frame = image::imageops::crop_imm(&i, 0, 0, side, side).to_image();
                Some(self.theme.atlas.add(&format!("item/{}/{aux}", key.0), &frame))
            }
            None => self.blocks.as_ref().filter(|_| block != 0).and_then(|(pack, table)| block_icon(table.get(block), &pack.atlas)).map(|icon| {
                self.theme.atlas.add(&format!("block/{}/{block}", self.generation), &icon)
            }),
        };
        self.added.insert(key, sprite);
        sprite
    }
}
