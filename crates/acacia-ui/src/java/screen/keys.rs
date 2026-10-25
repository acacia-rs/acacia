//! Key mappings as the key binds screen reads them: `InputConstants.Key` (type and value, its name
//! and display name), `KeyMapping` (category, default and order) and the `Options.keyMappings`
//! defaults. The OS key name (`SDL_GetKeyName`) is a US layout's: see `keys_table::us_layout_char`.

use std::cmp::Ordering;

use super::keys_table::{KEYBOARD, MOUSE, us_layout_char};
use crate::java::text::{Lang, Text};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyType {
    Keyboard,
    Mouse,
}

/// `InputConstants.Key`: equal by type and value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub kind: KeyType,
    pub value: i32,
}

/// `InputConstants.UNKNOWN`.
pub const UNKNOWN: Key = Key { kind: KeyType::Keyboard, value: 0 };

/// The platform `InputQuirks.keyboardTranslationKey` renames the Windows keys for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    Mac,
    Linux,
    Other,
}

impl Os {
    pub fn current() -> Os {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Mac
        } else if cfg!(target_os = "linux") {
            Os::Linux
        } else {
            Os::Other
        }
    }

    /// `InputQuirks.keyboardTranslationKey`.
    fn translation_key(self, name: &str) -> String {
        let renamed = match (self, name) {
            (Os::Mac, "key.keyboard.left.alt") => "key.keyboard.left.option",
            (Os::Mac, "key.keyboard.right.alt") => "key.keyboard.right.option",
            (Os::Mac, "key.keyboard.left.win") => "key.keyboard.left.command",
            (Os::Mac, "key.keyboard.right.win") => "key.keyboard.right.command",
            (Os::Windows, "key.keyboard.left.win") => "key.keyboard.left.windows",
            (Os::Windows, "key.keyboard.right.win") => "key.keyboard.right.windows",
            (Os::Linux, "key.keyboard.left.win") => "key.keyboard.left.meta",
            (Os::Linux, "key.keyboard.right.win") => "key.keyboard.right.meta",
            _ => name,
        };
        renamed.to_owned()
    }
}

impl Key {
    pub const fn keyboard(value: i32) -> Key {
        Key { kind: KeyType::Keyboard, value }
    }

    pub const fn mouse(value: i32) -> Key {
        Key { kind: KeyType::Mouse, value }
    }

    /// `getName`: a named key's, else `key.keyboard.<value>` / `key.mouse.<value>` (`getOrCreate`).
    pub fn name(&self) -> String {
        let (table, prefix) = match self.kind {
            KeyType::Keyboard => (KEYBOARD, "key.keyboard"),
            KeyType::Mouse => (MOUSE, "key.mouse"),
        };
        match table.iter().find(|(v, _)| *v == self.value) {
            Some((_, n)) => format!("{prefix}.{n}"),
            None => format!("{prefix}.{}", self.value),
        }
    }

    /// `getDisplayName`.
    pub fn display(&self, lang: Lang, os: Os) -> Text {
        let name = self.name();
        match self.kind {
            KeyType::Keyboard if name == "key.keyboard.unknown" => Text::translate(&name, &[], lang),
            KeyType::Keyboard => match us_layout_char(self.value) {
                Some(c) => Text::literal(c.to_string()),
                None => Text::translate(&os.translation_key(&name), &[], lang),
            },
            KeyType::Mouse if lang(&name).is_some() => Text::translate(&name, &[], lang),
            KeyType::Mouse => Text::translate("key.mouse", &[Text::literal(self.value.to_string())], lang),
        }
    }
}

/// `KeyMapping.Category`, in `SORT_ORDER`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    Movement,
    Misc,
    Multiplayer,
    Gameplay,
    Inventory,
    Creative,
    Spectator,
    Debug,
}

impl Category {
    /// `label()`'s key.
    pub fn key(self) -> &'static str {
        match self {
            Category::Movement => "key.category.minecraft.movement",
            Category::Misc => "key.category.minecraft.misc",
            Category::Multiplayer => "key.category.minecraft.multiplayer",
            Category::Gameplay => "key.category.minecraft.gameplay",
            Category::Inventory => "key.category.minecraft.inventory",
            Category::Creative => "key.category.minecraft.creative",
            Category::Spectator => "key.category.minecraft.spectator",
            Category::Debug => "key.category.minecraft.debug",
        }
    }
}

/// A `KeyMapping`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mapping {
    pub name: &'static str,
    pub category: Category,
    pub default: Key,
    pub key: Key,
    pub order: i32,
}

impl Mapping {
    pub fn is_default(&self) -> bool {
        self.key == self.default
    }

    pub fn is_unbound(&self) -> bool {
        self.key == UNKNOWN
    }
}

use Category::*;

const fn kb(name: &'static str, value: i32, category: Category, order: i32) -> (&'static str, Key, Category, i32) {
    (name, Key::keyboard(value), category, order)
}

const fn ms(name: &'static str, value: i32, category: Category) -> (&'static str, Key, Category, i32) {
    (name, Key::mouse(value), category, 0)
}

/// `Options.keyMappings` in its order, at their defaults.
const DEFAULTS: &[(&str, Key, Category, i32)] = &[
    ms("key.attack", 1, Gameplay), ms("key.use", 3, Gameplay), kb("key.forward", 26, Movement, 0), kb("key.left", 4, Movement, 0),
    kb("key.back", 22, Movement, 0), kb("key.right", 7, Movement, 0), kb("key.jump", 44, Movement, 0), kb("key.sneak", 225, Movement, 0),
    kb("key.sprint", 224, Movement, 0), kb("key.drop", 20, Inventory, 0), kb("key.inventory", 8, Inventory, 0), kb("key.chat", 23, Multiplayer, 0),
    kb("key.playerlist", 43, Multiplayer, 0), ms("key.pickItem", 2, Gameplay), kb("key.command", 56, Multiplayer, 0),
    kb("key.friends", 18, Multiplayer, 0), kb("key.socialInteractions", 19, Multiplayer, 0), kb("key.toggleGui", 58, Misc, 0),
    kb("key.toggleSpectatorShaderEffects", 61, Misc, 0), kb("key.screenshot", 59, Misc, 0), kb("key.togglePerspective", 62, Misc, 0),
    kb("key.smoothCamera", 0, Misc, 0), kb("key.fullscreen", 68, Misc, 0), kb("key.spectatorOutlines", 0, Spectator, 0),
    ms("key.spectatorHotbar", 2, Spectator), kb("key.swapOffhand", 9, Inventory, 0), kb("key.saveToolbarActivator", 6, Creative, 0),
    kb("key.loadToolbarActivator", 27, Creative, 0), kb("key.advancements", 15, Misc, 0), kb("key.quickActions", 10, Misc, 0),
    kb("key.debug.overlay", 60, Debug, -2), kb("key.debug.modifier", 60, Debug, -1),
    kb("key.hotbar.1", 30, Inventory, 0), kb("key.hotbar.2", 31, Inventory, 0), kb("key.hotbar.3", 32, Inventory, 0),
    kb("key.hotbar.4", 33, Inventory, 0), kb("key.hotbar.5", 34, Inventory, 0), kb("key.hotbar.6", 35, Inventory, 0),
    kb("key.hotbar.7", 36, Inventory, 0), kb("key.hotbar.8", 37, Inventory, 0), kb("key.hotbar.9", 38, Inventory, 0),
    kb("key.debug.reloadChunk", 4, Debug, 0), kb("key.debug.showHitboxes", 5, Debug, 0), kb("key.debug.clearChat", 7, Debug, 0),
    kb("key.debug.crash", 6, Debug, 0), kb("key.debug.showChunkBorders", 10, Debug, 0), kb("key.debug.showAdvancedTooltips", 11, Debug, 0),
    kb("key.debug.copyRecreateCommand", 12, Debug, 0), kb("key.debug.spectate", 17, Debug, 0), kb("key.debug.switchGameMode", 61, Debug, 0),
    kb("key.debug.debugOptions", 63, Debug, 0), kb("key.debug.focusPause", 19, Debug, 0), kb("key.debug.dumpDynamicTextures", 22, Debug, 0),
    kb("key.debug.reloadResourcePacks", 23, Debug, 0), kb("key.debug.profiling", 15, Debug, 0), kb("key.debug.copyLocation", 6, Debug, 0),
    kb("key.debug.dumpVersion", 25, Debug, 0), kb("key.debug.profilingChart", 30, Debug, 1), kb("key.debug.fpsCharts", 31, Debug, 2),
    kb("key.debug.networkCharts", 32, Debug, 3), kb("key.debug.lightmapTexture", 33, Debug, 4), kb("key.debug.improvedTransparency", 27, Debug, 0),
];

/// Java's key mappings at their defaults, in `Options.keyMappings` order.
pub fn defaults() -> Vec<Mapping> {
    DEFAULTS.iter().map(|&(name, key, category, order)| Mapping { name, category, default: key, key, order }).collect()
}

/// `String.compareTo`: by UTF-16 unit.
fn java_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `Arrays.sort` by `KeyMapping.compareTo`: category, then order, then translated name (stable).
pub fn sorted(mappings: &[Mapping], lang: Lang) -> Vec<Mapping> {
    let tr = |k: &str| lang(k).unwrap_or_else(|| k.to_owned());
    let mut out = mappings.to_vec();
    out.sort_by(|a, b| {
        if a.category != b.category {
            a.category.cmp(&b.category)
        } else if a.order == b.order {
            java_cmp(&tr(a.name), &tr(b.name))
        } else {
            a.order.cmp(&b.order)
        }
    });
    out
}
