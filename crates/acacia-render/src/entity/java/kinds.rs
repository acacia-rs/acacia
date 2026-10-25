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
    /// Always, blended (`SlimeOuterLayer`).
    Translucent,
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

pub const OVERLAYS: [(&str, Overlay); 15] = [
    ("minecraft:breeze", overlay("eyes", "breeze/breeze_eyes", None, When::Eyes)),
    ("minecraft:copper_golem", overlay("main", "copper_golem/copper_golem_eyes", None, When::Eyes)),
    ("minecraft:ender_dragon", overlay("main", "enderdragon/dragon_eyes", None, When::Eyes)),
    ("minecraft:wither", overlay("armor", "wither/wither_armor", None, When::Powered)),
    ("minecraft:phantom", overlay("main", "phantom/phantom_eyes", None, When::Eyes)),
    ("minecraft:slime", overlay("outer", "slime/slime", None, When::Translucent)),
    ("minecraft:enderman", overlay("main", "enderman/enderman_eyes", None, When::Eyes)),
    ("minecraft:spider", overlay("main", "spider/spider_eyes", None, When::Eyes)),
    ("minecraft:cave_spider", overlay("main", "spider/spider_eyes", None, When::Eyes)),
    ("minecraft:sheep", overlay("wool", "sheep/sheep_wool", Some("sheep/sheep_wool_baby"), When::Wool)),
    ("minecraft:sheep", overlay("wool_undercoat", "sheep/sheep_wool_undercoat", None, When::Undercoat)),
    ("minecraft:drowned", overlay("outer", "zombie/drowned_outer_layer", Some("zombie/drowned_outer_layer_baby"), When::Always)),
    ("minecraft:stray", overlay("outer", "skeleton/stray_overlay", None, When::Always)),
    ("minecraft:bogged", overlay("outer", "skeleton/bogged_overlay", None, When::Always)),
    ("minecraft:creeper", overlay("armor", "creeper/creeper_armor", None, When::Powered)),
];

// TODO: climate variants (cold/warm cows, pigs, chickens), wolf and panda variants, wolf collars,
// llama decor, tropical fish shapes and patterns, puffed pufferfish, salmon sizes, horse markings and saddles, snowy sleeping foxes, breeze wind and eyes, creaking and warden layers, angry bees with nectar,
// shulker colours and attachment, cold striders, the wither's invulnerable blink.
pub const KINDS: [(&str, Kind); 89] = [
    ("minecraft:ender_dragon", adult("minecraft:ender_dragon#main", "enderdragon/dragon")),
    ("minecraft:happy_ghast", both("minecraft:happy_ghast#main", "ghast/happy_ghast", "minecraft:happy_ghast_baby#main", "ghast/happy_ghast_baby")),
    ("minecraft:bat", adult("minecraft:bat#main", "bat/bat")),
    ("minecraft:rabbit", both("minecraft:rabbit#main", "rabbit/rabbit_brown", "minecraft:rabbit_baby#main", "rabbit/rabbit_brown_baby")),
    ("minecraft:frog", adult("minecraft:frog#main", "frog/frog_temperate")),
    ("minecraft:armadillo", both("minecraft:armadillo#main", "armadillo/armadillo", "minecraft:armadillo_baby#main", "armadillo/armadillo_baby")),
    ("minecraft:camel", both("minecraft:camel#main", "camel/camel", "minecraft:camel_baby#main", "camel/camel_baby")),
    ("minecraft:camel_husk", adult("minecraft:camel#main", "camel/camel_husk")),
    ("minecraft:sniffer", both("minecraft:sniffer#main", "sniffer/sniffer", "minecraft:sniffer_baby#main", "sniffer/snifflet")),
    ("minecraft:breeze", adult("minecraft:breeze#main", "breeze/breeze")),
    ("minecraft:creaking", adult("minecraft:creaking#main", "creaking/creaking")),
    ("minecraft:warden", adult("minecraft:warden#main", "warden/warden")),
    ("minecraft:copper_golem", adult("minecraft:copper_golem#main", "copper_golem/copper_golem")),
    ("minecraft:nautilus", both("minecraft:nautilus#main", "nautilus/nautilus", "minecraft:nautilus_baby#main", "nautilus/nautilus_baby")),
    ("minecraft:zombie_nautilus", adult("minecraft:zombie_nautilus#main", "nautilus/zombie_nautilus")),
    ("minecraft:tadpole", adult("minecraft:tadpole#main", "tadpole/tadpole")),
    ("minecraft:zombie_villager_v2", both("minecraft:zombie_villager#main", "zombie_villager/zombie_villager", "minecraft:zombie_villager_baby#main", "zombie_villager/zombie_villager_baby")),
    ("minecraft:zombie_villager", both("minecraft:zombie_villager#main", "zombie_villager/zombie_villager", "minecraft:zombie_villager_baby#main", "zombie_villager/zombie_villager_baby")),
    ("minecraft:axolotl", both("minecraft:axolotl#main", "axolotl/axolotl_lucy", "minecraft:axolotl_baby#main", "axolotl/axolotl_lucy_baby")),
    ("minecraft:armor_stand", adult("minecraft:armor_stand#main", "armorstand/armorstand")),
    ("minecraft:strider", both("minecraft:strider#main", "strider/strider", "minecraft:strider_baby#main", "strider/strider_baby")),
    ("minecraft:ravager", adult("minecraft:ravager#main", "illager/ravager")),
    ("minecraft:shulker", adult("minecraft:shulker#main", "shulker/shulker")),
    ("minecraft:wither", adult("minecraft:wither#main", "wither/wither")),
    ("minecraft:bee", both("minecraft:bee#main", "bee/bee", "minecraft:bee_baby#main", "bee/bee_baby")),
    ("minecraft:parrot", adult("minecraft:parrot#main", "parrot/parrot_red_blue")),
    ("minecraft:fox", both("minecraft:fox#main", "fox/fox", "minecraft:fox_baby#main", "fox/fox_baby")),
    ("minecraft:horse", both("minecraft:horse#main", "horse/horse_white", "minecraft:horse_baby#main", "horse/horse_white_baby")),
    ("minecraft:donkey", both("minecraft:donkey#main", "horse/donkey", "minecraft:donkey_baby#main", "horse/donkey_baby")),
    ("minecraft:mule", both("minecraft:mule#main", "horse/mule", "minecraft:mule_baby#main", "horse/mule_baby")),
    ("minecraft:skeleton_horse", both("minecraft:skeleton_horse#main", "horse/horse_skeleton", "minecraft:skeleton_horse_baby#main", "horse/horse_skeleton_baby")),
    ("minecraft:zombie_horse", both("minecraft:zombie_horse#main", "horse/horse_zombie", "minecraft:zombie_horse_baby#main", "horse/horse_zombie_baby")),
    ("minecraft:cod", adult("minecraft:cod#main", "fish/cod")),
    ("minecraft:salmon", adult("minecraft:salmon#main", "fish/salmon")),
    ("minecraft:tropicalfish", adult("minecraft:tropical_fish_small#main", "fish/tropical_a")),
    ("minecraft:pufferfish", adult("minecraft:pufferfish_small#main", "fish/pufferfish")),
    ("minecraft:guardian", adult("minecraft:guardian#main", "guardian/guardian")),
    ("minecraft:elder_guardian", adult("minecraft:elder_guardian#main", "guardian/guardian_elder")),
    ("minecraft:phantom", adult("minecraft:phantom#main", "phantom/phantom")),
    ("minecraft:allay", adult("minecraft:allay#main", "allay/allay")),
    ("minecraft:cat", both("minecraft:cat#main", "cat/cat_tabby", "minecraft:cat_baby#main", "cat/cat_tabby_baby")),
    ("minecraft:ocelot", both("minecraft:ocelot#main", "cat/ocelot", "minecraft:ocelot_baby#main", "cat/ocelot_baby")),
    ("minecraft:polar_bear", both("minecraft:polar_bear#main", "bear/polarbear", "minecraft:polar_bear_baby#main", "bear/polarbear_baby")),
    ("minecraft:panda", both("minecraft:panda#main", "panda/panda", "minecraft:panda_baby#main", "panda/panda_baby")),
    ("minecraft:goat", both("minecraft:goat#main", "goat/goat", "minecraft:goat_baby#main", "goat/goat_baby")),
    ("minecraft:turtle", both("minecraft:turtle#main", "turtle/turtle", "minecraft:turtle_baby#main", "turtle/turtle_baby")),
    ("minecraft:hoglin", both("minecraft:hoglin#main", "hoglin/hoglin", "minecraft:hoglin_baby#main", "hoglin/hoglin_baby")),
    ("minecraft:zoglin", both("minecraft:zoglin#main", "hoglin/zoglin", "minecraft:zoglin_baby#main", "hoglin/zoglin_baby")),
    ("minecraft:wolf", both("minecraft:wolf#main", "wolf/wolf", "minecraft:wolf_baby#main", "wolf/wolf_baby")),
    ("minecraft:llama", both("minecraft:llama#main", "llama/llama_creamy", "minecraft:llama_baby#main", "llama/llama_creamy_baby")),
    ("minecraft:trader_llama", both("minecraft:trader_llama#main", "llama/llama_creamy", "minecraft:trader_llama_baby#main", "llama/llama_creamy_baby")),
    ("minecraft:squid", both("minecraft:squid#main", "squid/squid", "minecraft:squid_baby#main", "squid/squid_baby")),
    ("minecraft:glow_squid", both("minecraft:glow_squid#main", "squid/glow_squid", "minecraft:glow_squid_baby#main", "squid/glow_squid_baby")),
    ("minecraft:dolphin", both("minecraft:dolphin#main", "dolphin/dolphin", "minecraft:dolphin_baby#main", "dolphin/dolphin_baby")),
    ("minecraft:slime", adult("minecraft:slime#main", "slime/slime")),
    ("minecraft:magma_cube", adult("minecraft:magma_cube#main", "slime/magmacube")),
    ("minecraft:piglin", both("minecraft:piglin#main", "piglin/piglin", "minecraft:piglin_baby#main", "piglin/piglin_baby")),
    ("minecraft:piglin_brute", adult("minecraft:piglin_brute#main", "piglin/piglin_brute")),
    ("minecraft:zombie_pigman", both("minecraft:zombified_piglin#main", "piglin/zombified_piglin", "minecraft:zombified_piglin_baby#main", "piglin/zombified_piglin_baby")),
    ("minecraft:enderman", adult("minecraft:enderman#main", "enderman/enderman")),
    ("minecraft:witch", adult("minecraft:witch#main", "witch/witch")),
    ("minecraft:vex", adult("minecraft:vex#main", "illager/vex")),
    ("minecraft:vindicator", adult("minecraft:vindicator#main", "illager/vindicator")),
    ("minecraft:evocation_illager", adult("minecraft:evoker#main", "illager/evoker")),
    ("minecraft:pillager", adult("minecraft:pillager#main", "illager/pillager")),
    ("minecraft:silverfish", adult("minecraft:silverfish#main", "silverfish/silverfish")),
    ("minecraft:endermite", adult("minecraft:endermite#main", "endermite/endermite")),
    ("minecraft:blaze", adult("minecraft:blaze#main", "blaze/blaze")),
    ("minecraft:ghast", adult("minecraft:ghast#main", "ghast/ghast")),
    ("minecraft:iron_golem", adult("minecraft:iron_golem#main", "iron_golem/iron_golem")),
    ("minecraft:snow_golem", adult("minecraft:snow_golem#main", "snow_golem/snow_golem")),
    ("minecraft:villager_v2", both("minecraft:villager#main", "villager/villager", "minecraft:villager_baby#main", "villager/villager_baby")),
    ("minecraft:villager", both("minecraft:villager#main", "villager/villager", "minecraft:villager_baby#main", "villager/villager_baby")),
    ("minecraft:wandering_trader", adult("minecraft:wandering_trader#main", "wandering_trader/wandering_trader")),
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

/// Textures picked over the kind's own while a query answers a value: (kind, query, value,
/// adult's, own baby's).
pub const VARIANTS: [(&str, &str, i32, &str, &str); 46] = [
    ("minecraft:frog", "variant", 1, "frog/frog_cold", "frog/frog_cold"),
    ("minecraft:frog", "variant", 2, "frog/frog_warm", "frog/frog_warm"),
    ("minecraft:rabbit", "variant", 1, "rabbit/rabbit_white", "rabbit/rabbit_white_baby"),
    ("minecraft:rabbit", "variant", 2, "rabbit/rabbit_black", "rabbit/rabbit_black_baby"),
    ("minecraft:rabbit", "variant", 3, "rabbit/rabbit_white_splotched", "rabbit/rabbit_white_splotched_baby"),
    ("minecraft:rabbit", "variant", 4, "rabbit/rabbit_gold", "rabbit/rabbit_gold_baby"),
    ("minecraft:rabbit", "variant", 5, "rabbit/rabbit_salt", "rabbit/rabbit_salt_baby"),
    ("minecraft:axolotl", "variant", 1, "axolotl/axolotl_cyan", "axolotl/axolotl_cyan_baby"),
    ("minecraft:axolotl", "variant", 2, "axolotl/axolotl_gold", "axolotl/axolotl_gold_baby"),
    ("minecraft:axolotl", "variant", 3, "axolotl/axolotl_wild", "axolotl/axolotl_wild_baby"),
    ("minecraft:axolotl", "variant", 4, "axolotl/axolotl_blue", "axolotl/axolotl_blue_baby"),
    ("minecraft:parrot", "variant", 1, "parrot/parrot_blue", "parrot/parrot_blue"),
    ("minecraft:parrot", "variant", 2, "parrot/parrot_green", "parrot/parrot_green"),
    ("minecraft:parrot", "variant", 3, "parrot/parrot_yellow_blue", "parrot/parrot_yellow_blue"),
    ("minecraft:parrot", "variant", 4, "parrot/parrot_grey", "parrot/parrot_grey"),
    ("minecraft:fox", "is_sleeping", 1, "fox/fox_sleep", "fox/fox_sleep_baby"),
    ("minecraft:fox", "variant", 1, "fox/fox_snow", "fox/fox_snow_baby"),
    ("minecraft:bee", "is_angry", 1, "bee/bee_angry", "bee/bee_angry_baby"),
    ("minecraft:bee", "has_nectar", 1, "bee/bee_nectar", "bee/bee_nectar_baby"),
    ("minecraft:horse", "variant", 1, "horse/horse_creamy", "horse/horse_creamy_baby"),
    ("minecraft:horse", "variant", 2, "horse/horse_chestnut", "horse/horse_chestnut_baby"),
    ("minecraft:horse", "variant", 3, "horse/horse_brown", "horse/horse_brown_baby"),
    ("minecraft:horse", "variant", 4, "horse/horse_black", "horse/horse_black_baby"),
    ("minecraft:horse", "variant", 5, "horse/horse_gray", "horse/horse_gray_baby"),
    ("minecraft:horse", "variant", 6, "horse/horse_darkbrown", "horse/horse_darkbrown_baby"),
    // Bedrock's cat variants by number; Java's `black` is the tuxedo, `all_black` the black.
    ("minecraft:cat", "variant", 0, "cat/cat_white", "cat/cat_white_baby"),
    ("minecraft:cat", "variant", 1, "cat/cat_black", "cat/cat_black_baby"),
    ("minecraft:cat", "variant", 2, "cat/cat_red", "cat/cat_red_baby"),
    ("minecraft:cat", "variant", 3, "cat/cat_siamese", "cat/cat_siamese_baby"),
    ("minecraft:cat", "variant", 4, "cat/cat_british_shorthair", "cat/cat_british_shorthair_baby"),
    ("minecraft:cat", "variant", 5, "cat/cat_calico", "cat/cat_calico_baby"),
    ("minecraft:cat", "variant", 6, "cat/cat_persian", "cat/cat_persian_baby"),
    ("minecraft:cat", "variant", 7, "cat/cat_ragdoll", "cat/cat_ragdoll_baby"),
    ("minecraft:cat", "variant", 9, "cat/cat_all_black", "cat/cat_all_black_baby"),
    ("minecraft:cat", "variant", 10, "cat/cat_jellie", "cat/cat_jellie_baby"),
    ("minecraft:wolf", "is_angry", 1, "wolf/wolf_angry", "wolf/wolf_angry_baby"),
    ("minecraft:wolf", "is_tamed", 1, "wolf/wolf_tame", "wolf/wolf_tame_baby"),
    ("minecraft:llama", "variant", 1, "llama/llama_white", "llama/llama_white_baby"),
    ("minecraft:llama", "variant", 2, "llama/llama_brown", "llama/llama_brown_baby"),
    ("minecraft:llama", "variant", 3, "llama/llama_gray", "llama/llama_gray_baby"),
    ("minecraft:trader_llama", "variant", 1, "llama/llama_white", "llama/llama_white_baby"),
    ("minecraft:trader_llama", "variant", 2, "llama/llama_brown", "llama/llama_brown_baby"),
    ("minecraft:trader_llama", "variant", 3, "llama/llama_gray", "llama/llama_gray_baby"),
    ("minecraft:mooshroom", "variant", 1, "cow/mooshroom_brown", "cow/mooshroom_brown_baby"),
    ("minecraft:ghast", "is_charging", 1, "ghast/ghast_shooting", "ghast/ghast_shooting"),
    ("minecraft:vex", "is_charging", 1, "illager/vex_charging", "illager/vex_charging"),
];

/// The Java entity id (no namespace) of a Bedrock `identifier`.
pub fn java_id(identifier: &str) -> &str {
    match identifier.trim_start_matches("minecraft:") {
        "tropicalfish" => "tropical_fish",
        "zombie_pigman" => "zombified_piglin",
        "evocation_illager" => "evoker",
        "villager_v2" => "villager",
        "zombie_villager_v2" => "zombie_villager",
        id => id,
    }
}

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
