//! `EnchantmentNames`: the enchanting table's galactic option names, three or four words drawn
//! from a `RandomSource` seeded with the menu's enchantment seed each frame, cut to the option's
//! width in the `minecraft:alt` font.

use super::effects::head_by_width;
use crate::java::gui::Metrics;
use crate::java::mth::JavaRandom;
use crate::java::text::{Style, Text};

const WORDS: [&str; 62] = [
    "the", "elder", "scrolls", "klaatu", "berata", "niktu", "xyzzy", "bless", "curse", "light", "darkness", "fire", "air",
    "earth", "water", "hot", "dry", "cold", "wet", "ignite", "snuff", "embiggen", "twist", "shorten", "stretch", "fiddle",
    "destroy", "imbue", "galvanize", "enchant", "free", "limited", "range", "of", "towards", "inside", "sphere", "cube",
    "self", "other", "ball", "mental", "physical", "grow", "shrink", "demon", "elemental", "spirit", "animal", "creature",
    "beast", "humanoid", "undead", "fresh", "stale", "phnglui", "mglwnafh", "cthulhu", "rlyeh", "wgahnagl", "fhtagn",
    "baguette",
];

pub const ALT_FONT: &str = "minecraft:alt";

pub struct EnchantmentNames(JavaRandom);

impl EnchantmentNames {
    /// After `initSeed(seed)`.
    pub fn new(seed: i64) -> EnchantmentNames {
        EnchantmentNames(JavaRandom::new(seed))
    }

    /// `getRandomName(font, maxWidth)`.
    pub fn random_name(&mut self, font: &dyn Metrics, max_width: i32) -> Text {
        let count = self.0.next_int_bound(2) + 3;
        let words: Vec<&str> = (0..count).map(|_| WORDS[self.0.next_int_bound(WORDS.len() as i32) as usize]).collect();
        let style = Style { font: Some(ALT_FONT.to_owned()), ..Style::default() };
        head_by_width(font, &Text::styled(words.join(" "), &style), max_width as f32)
    }
}
