//! Which Java model layer and texture draw a Bedrock entity kind, as each Java renderer names
//! them (`CowRenderer`, `ZombieRenderer`...). Textures are under the look's
//! `java/textures/entity/`, without the extension. Kinds not listed keep Bedrock geometry.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    /// `minecraft:cow#main`; its `setupAnim` is [`super::Family::of_layer`]'s.
    pub layer: &'static str,
    pub texture: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Kind {
    pub adult: Look,
    /// For kinds whose babies have their own model and texture.
    pub baby: Option<Look>,
}

const fn adult(layer: &'static str, texture: &'static str) -> Kind {
    Kind { adult: Look { layer, texture }, baby: None }
}

const fn both(layer: &'static str, texture: &'static str, baby_layer: &'static str, baby_texture: &'static str) -> Kind {
    Kind { adult: Look { layer, texture }, baby: Some(Look { layer: baby_layer, texture: baby_texture }) }
}

/// When a second layer draws.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum When {
    Always,
    /// `SheepWoolLayer`: unsheared, in the wool's colour.
    Wool,
    /// `SheepWoolUndercoatLayer`: an adult's, unsheared or not, unless white.
    Undercoat,
    /// `CreeperPowerLayer`: charged, as a swirl.
    Powered,
    /// `EyesLayer`: always, unlit (`suffix` is the body's own).
    Eyes,
}

/// A second layer over the body: its own model (`<layer>#<suffix>`, beside the body's) and texture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Overlay {
    pub suffix: &'static str,
    pub texture: &'static str,
    /// A baby with its own model draws the overlay only when this is given.
    pub baby_texture: Option<&'static str>,
    pub when: When,
}

const fn overlay(suffix: &'static str, texture: &'static str, baby_texture: Option<&'static str>, when: When) -> Overlay {
    Overlay { suffix, texture, baby_texture, when }
}

pub const OVERLAYS: [(&str, Overlay); 8] = [
    ("minecraft:spider", overlay("main", "spider/spider_eyes", None, When::Eyes)),
    ("minecraft:cave_spider", overlay("main", "spider/spider_eyes", None, When::Eyes)),
    ("minecraft:sheep", overlay("wool", "sheep/sheep_wool", Some("sheep/sheep_wool_baby"), When::Wool)),
    ("minecraft:sheep", overlay("wool_undercoat", "sheep/sheep_wool_undercoat", None, When::Undercoat)),
    ("minecraft:drowned", overlay("outer", "zombie/drowned_outer_layer", Some("zombie/drowned_outer_layer_baby"), When::Always)),
    ("minecraft:stray", overlay("outer", "skeleton/stray_overlay", None, When::Always)),
    ("minecraft:bogged", overlay("outer", "skeleton/bogged_overlay", None, When::Always)),
    ("minecraft:creeper", overlay("armor", "creeper/creeper_armor", None, When::Powered)),
];

// TODO: climate variants (cold/warm cows, pigs, chickens), the brown mooshroom.
pub const KINDS: [(&str, Kind); 15] = [
    ("minecraft:cow", both("minecraft:cow#main", "cow/cow_temperate", "minecraft:cow_baby#main", "cow/cow_temperate_baby")),
    ("minecraft:mooshroom", both("minecraft:mooshroom#main", "cow/mooshroom_red", "minecraft:mooshroom_baby#main", "cow/mooshroom_red_baby")),
    ("minecraft:pig", both("minecraft:pig#main", "pig/pig_temperate", "minecraft:pig_baby#main", "pig/pig_temperate_baby")),
    ("minecraft:sheep", both("minecraft:sheep#main", "sheep/sheep", "minecraft:sheep_baby#main", "sheep/sheep_baby")),
    ("minecraft:chicken", both("minecraft:chicken#main", "chicken/chicken_temperate", "minecraft:chicken_baby#main", "chicken/chicken_temperate_baby")),
    ("minecraft:creeper", adult("minecraft:creeper#main", "creeper/creeper")),
    ("minecraft:spider", adult("minecraft:spider#main", "spider/spider")),
    ("minecraft:cave_spider", adult("minecraft:cave_spider#main", "spider/cave_spider")),
    ("minecraft:zombie", both("minecraft:zombie#main", "zombie/zombie", "minecraft:zombie_baby#main", "zombie/zombie_baby")),
    ("minecraft:husk", both("minecraft:husk#main", "zombie/husk", "minecraft:husk_baby#main", "zombie/husk_baby")),
    ("minecraft:drowned", both("minecraft:drowned#main", "zombie/drowned", "minecraft:drowned_baby#main", "zombie/drowned_baby")),
    ("minecraft:skeleton", adult("minecraft:skeleton#main", "skeleton/skeleton")),
    ("minecraft:stray", adult("minecraft:stray#main", "skeleton/stray")),
    ("minecraft:wither_skeleton", adult("minecraft:wither_skeleton#main", "skeleton/wither_skeleton")),
    ("minecraft:bogged", adult("minecraft:bogged#main", "skeleton/bogged")),
];

/// Textures a Bedrock `variant` picks over the kind's own: (kind, variant, adult's, own baby's).
pub const VARIANTS: [(&str, i32, &str, &str); 1] = [("minecraft:mooshroom", 1, "cow/mooshroom_brown", "cow/mooshroom_brown_baby")];

pub fn of(identifier: &str) -> Option<&'static Kind> {
    KINDS.iter().find(|(id, _)| *id == identifier).map(|(_, k)| k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_layer_listed_has_a_setup_anim() {
        for (id, kind) in &KINDS {
            for look in [Some(kind.adult), kind.baby].into_iter().flatten() {
                assert!(super::super::Family::of_layer(look.layer).is_some(), "{id}: {}", look.layer);
            }
        }
    }
}
