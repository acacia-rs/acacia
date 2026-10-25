//! Which Java model layer and texture draw a Bedrock entity kind, as each Java renderer names
//! them (`CowRenderer`, `ZombieRenderer`...). Textures are under the look's `java/` directory.
//! Kinds not listed keep Bedrock geometry (README "Java entity models" has the list).

use super::anim::Family;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    /// `minecraft:cow#main`.
    pub layer: &'static str,
    /// Under `java/textures/entity/`, without the extension.
    pub texture: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Kind {
    pub adult: Look,
    /// The baby model and texture, for kinds whose babies have their own.
    pub baby: Option<Look>,
    pub family: Family,
}

const fn look(layer: &'static str, texture: &'static str) -> Look {
    Look { layer, texture }
}

const fn kind(family: Family, adult: Look, baby: Option<Look>) -> Kind {
    Kind { adult, baby, family }
}

use Family::*;

// TODO: climate variants (cold/warm cows, pigs, chickens), mooshroom brown, the second layers
// (sheep wool, drowned and stray overlays, spider eyes, creeper aura).
pub const KINDS: [(&str, Kind); 15] = [
    ("minecraft:cow", kind(Quadruped, look("minecraft:cow#main", "cow/cow_temperate"), Some(look("minecraft:cow_baby#main", "cow/cow_temperate_baby")))),
    ("minecraft:mooshroom", kind(Quadruped, look("minecraft:mooshroom#main", "cow/mooshroom_red"), Some(look("minecraft:mooshroom_baby#main", "cow/mooshroom_red_baby")))),
    ("minecraft:pig", kind(Quadruped, look("minecraft:pig#main", "pig/pig_temperate"), Some(look("minecraft:pig_baby#main", "pig/pig_temperate_baby")))),
    ("minecraft:sheep", kind(Sheep, look("minecraft:sheep#main", "sheep/sheep"), Some(look("minecraft:sheep_baby#main", "sheep/sheep_baby")))),
    ("minecraft:chicken", kind(Chicken, look("minecraft:chicken#main", "chicken/chicken_temperate"), Some(look("minecraft:chicken_baby#main", "chicken/chicken_temperate_baby")))),
    ("minecraft:creeper", kind(Creeper, look("minecraft:creeper#main", "creeper/creeper"), None)),
    ("minecraft:spider", kind(Spider, look("minecraft:spider#main", "spider/spider"), None)),
    ("minecraft:cave_spider", kind(Spider, look("minecraft:cave_spider#main", "spider/cave_spider"), None)),
    ("minecraft:zombie", kind(Zombie, look("minecraft:zombie#main", "zombie/zombie"), Some(look("minecraft:zombie_baby#main", "zombie/zombie_baby")))),
    ("minecraft:husk", kind(Zombie, look("minecraft:husk#main", "zombie/husk"), Some(look("minecraft:husk_baby#main", "zombie/husk_baby")))),
    ("minecraft:drowned", kind(Zombie, look("minecraft:drowned#main", "zombie/drowned"), Some(look("minecraft:drowned_baby#main", "zombie/drowned_baby")))),
    ("minecraft:skeleton", kind(Skeleton, look("minecraft:skeleton#main", "skeleton/skeleton"), None)),
    ("minecraft:stray", kind(Skeleton, look("minecraft:stray#main", "skeleton/stray"), None)),
    ("minecraft:wither_skeleton", kind(Skeleton, look("minecraft:wither_skeleton#main", "skeleton/wither_skeleton"), None)),
    ("minecraft:bogged", kind(Skeleton, look("minecraft:bogged#main", "skeleton/bogged"), None)),
];

pub fn of(identifier: &str) -> Option<&'static Kind> {
    KINDS.iter().find(|(id, _)| *id == identifier).map(|(_, k)| k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_animates_as_its_layer_family() {
        for (id, kind) in &KINDS {
            assert_eq!(Family::of_layer(kind.adult.layer), Some(kind.family), "{id}");
            if let Some(baby) = kind.baby {
                assert_eq!(Family::of_layer(baby.layer), Some(kind.family), "{id} baby");
            }
        }
    }
}
