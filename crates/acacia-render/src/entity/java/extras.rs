//! What kinds' own renderers change on the body and add over it beyond [`super::kinds`]' table:
//! the `entity_translucent` bodies, climate variants (Bedrock's `climate_variant` property), the
//! coral zombie nautilus, `PufferfishRenderer`'s model by puff, `TropicalFishRenderer`'s
//! shape and colour with its `TropicalFishPatternLayer`, `IronGolemCrackinessLayer`,
//! `BreezeWindLayer`, the creaking's eyes and the warden's emissive layers.

use super::Family;
use super::gear::{Draw, entity_texture};
use crate::entity::molang::Value;
use crate::entity::{Blend, DYES, Fade};

/// What a kind draws its body with in place of [`super::kinds`]' look (`None`: that look's).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Skin {
    pub layer: Option<&'static str>,
    pub texture: Option<String>,
    pub color: Option<[u8; 3]>,
    pub blend: Blend,
}

/// Every model layer [`skin`] and [`draws`] can name, for baking up front.
pub const LAYERS: [&str; 18] = [
    "minecraft:cold_cow#main", "minecraft:cold_cow_baby#main", "minecraft:warm_cow#main", "minecraft:warm_cow_baby#main",
    "minecraft:cold_pig#main", "minecraft:cold_chicken#main", "minecraft:zombie_nautilus_coral#main", "minecraft:pufferfish_small#main", "minecraft:pufferfish_medium#main", "minecraft:pufferfish_big#main",
    "minecraft:tropical_fish_small#main", "minecraft:tropical_fish_large#main", "minecraft:tropical_fish_small#pattern",
    "minecraft:tropical_fish_large#pattern", "minecraft:breeze#wind", "minecraft:creaking#eyes", "minecraft:warden#bioluminescent",
    "minecraft:warden#pulsating_spots",
];

/// Every texture [`skin`] and [`draws`] can name, for registering up front.
pub fn textures() -> impl Iterator<Item = String> {
    let patterns = ["a", "b"].into_iter().flat_map(|shape| (1..=6).map(move |i| format!("fish/tropical_{shape}_pattern_{i}")));
    let climates = ["cow", "pig", "chicken"].into_iter().flat_map(|kind| {
        ["warm", "cold"].into_iter().flat_map(move |climate| [format!("{kind}/{kind}_{climate}"), format!("{kind}/{kind}_{climate}_baby")])
    });
    let copper = OXIDATION.into_iter().flat_map(|(_, suffix)| {
        [format!("copper_golem/copper_golem{suffix}"), format!("copper_golem/copper_golem_eyes{suffix}")]
    });
    let fixed = [
        "nautilus/zombie_nautilus_coral",
        "fish/tropical_a", "fish/tropical_b", "iron_golem/iron_golem_crackiness_low", "iron_golem/iron_golem_crackiness_medium",
        "iron_golem/iron_golem_crackiness_high", "breeze/breeze_wind", "creaking/creaking_eyes", "warden/warden_bioluminescent_layer",
        "warden/warden_pulsating_spots_1", "warden/warden_pulsating_spots_2",
    ];
    let wolves = WOLVES.into_iter().flat_map(|variant| {
        ["", "_tame", "_angry"].into_iter().flat_map(move |state| ["", "_baby"].map(|baby| format!("wolf/wolf{variant}{state}{baby}")))
    });
    fixed.into_iter().map(str::to_owned).chain(patterns).chain(climates).chain(copper).chain(wolves).map(|name| entity_texture(&name))
}

/// Bedrock's wolf `variant`s, as `WolfVariants` names their textures (`wolf_ashen`; the pale is `wolf`).
const WOLVES: [&str; 9] = ["", "_ashen", "_black", "_chestnut", "_rusty", "_snowy", "_spotted", "_striped", "_woods"];

/// `Wolf.getTexture`: the variant's tame texture over its angry one over its wild one.
fn wolf(query: &dyn Fn(&str) -> Value) -> String {
    let variant = WOLVES[(query("variant").num().max(0.0) as usize).min(WOLVES.len() - 1)];
    let state = if query("is_tamed").truthy() {
        "_tame"
    } else if query("is_angry").truthy() {
        "_angry"
    } else {
        ""
    };
    entity_texture(&format!("wolf/wolf{variant}{state}{}", if query("is_baby").truthy() { "_baby" } else { "" }))
}

/// Bedrock's `oxidation_level` property values and `CopperGolemOxidationLevels`' texture suffixes.
const OXIDATION: [(&str, &str); 4] = [("unoxidized", ""), ("exposed", "_exposed"), ("weathered", "_weathered"), ("oxidized", "_oxidized")];

fn oxidation(query: &dyn Fn(&str) -> Value) -> &'static str {
    let level = query("property:minecraft:oxidation_level");
    OXIDATION.iter().find(|(name, _)| matches!(&level, Value::Text(t) if t == name)).map_or("", |(_, suffix)| suffix)
}

/// `CowVariants`, `PigVariants` and `ChickenVariants`: a climate's model (`None`: the kind's own)
/// and texture, adult then baby. Pigs and chickens have no warm model, and no climate a baby model.
fn climate(kind: &str, climate: &str, baby: bool) -> Option<Skin> {
    let layer = match (kind, climate, baby) {
        ("cow", "warm", false) => Some("minecraft:warm_cow#main"),
        ("cow", "warm", true) => Some("minecraft:warm_cow_baby#main"),
        ("cow", "cold", false) => Some("minecraft:cold_cow#main"),
        ("cow", "cold", true) => Some("minecraft:cold_cow_baby#main"),
        ("pig", "cold", false) => Some("minecraft:cold_pig#main"),
        ("chicken", "cold", false) => Some("minecraft:cold_chicken#main"),
        (_, "warm" | "cold", _) => None,
        _ => return None,
    };
    let texture = entity_texture(&format!("{kind}/{kind}_{climate}{}", if baby { "_baby" } else { "" }));
    Some(Skin { layer, texture: Some(texture), ..Skin::default() })
}

/// The `setupAnim` of a [`LAYERS`] model: its body's.
pub fn family(layer: &str) -> Option<Family> {
    Family::of_layer(&format!("{}#main", layer.split('#').next()?))
}

fn dye(index: f32) -> [u8; 3] {
    let [_, r, g, b] = DYES[index.max(0.0) as usize % DYES.len()].to_be_bytes();
    [r, g, b]
}

fn tropical_shape(query: &dyn Fn(&str) -> Value) -> (&'static str, &'static str) {
    if query("variant").num() as i32 == 1 { ("large", "b") } else { ("small", "a") }
}

/// How a `kind` (Java id) draws its body, when its renderer changes [`super::kinds`]' look.
pub fn skin(kind: &str, query: &dyn Fn(&str) -> Value) -> Option<Skin> {
    match kind.trim_start_matches("minecraft:") {
        "allay" | "vex" | "breeze" | "piglin" | "piglin_brute" | "zombified_piglin" => Some(Skin { blend: Blend::Translucent, ..Skin::default() }),
        kind @ ("cow" | "pig" | "chicken") => match query("property:minecraft:climate_variant") {
            Value::Text(text) => climate(kind, &text, query("is_baby").truthy()),
            Value::Num(_) => None,
        },
        "wolf" => Some(Skin { texture: Some(wolf(query)), ..Skin::default() }),
        "copper_golem" => Some(Skin { texture: Some(entity_texture(&format!("copper_golem/copper_golem{}", oxidation(query)))), ..Skin::default() }),
        "zombie_nautilus" if matches!(query("property:minecraft:variant"), Value::Text(v) if v == "coral") => Some(Skin {
            layer: Some("minecraft:zombie_nautilus_coral#main"),
            texture: Some(entity_texture("nautilus/zombie_nautilus_coral")),
            ..Skin::default()
        }),
        "pufferfish" => {
            const PUFFS: [&str; 3] = ["minecraft:pufferfish_small#main", "minecraft:pufferfish_medium#main", "minecraft:pufferfish_big#main"];
            Some(Skin { layer: Some(PUFFS[(query("variant").num().max(0.0) as usize).min(2)]), ..Skin::default() })
        }
        "tropical_fish" => {
            let (shape, letter) = tropical_shape(query);
            let layer = if shape == "large" { "minecraft:tropical_fish_large#main" } else { "minecraft:tropical_fish_small#main" };
            let texture = entity_texture(&format!("fish/tropical_{letter}"));
            Some(Skin { layer: Some(layer), texture: Some(texture), color: Some(dye(query("color").num())), blend: Blend::Opaque })
        }
        _ => None,
    }
}

fn draw(layer: Option<&'static str>, texture: &str, color: Option<[u8; 3]>, blend: Blend, fade: Fade) -> Draw {
    Draw { layer, texture: entity_texture(texture), color, blend, fade }
}

/// The draws a `kind` (Java id) adds over its body, in its renderer's order.
pub fn draws(kind: &str, query: &dyn Fn(&str) -> Value) -> Vec<Draw> {
    let invisible = query("is_invisible").truthy();
    match kind.trim_start_matches("minecraft:") {
        "tropical_fish" => {
            let (shape, letter) = tropical_shape(query);
            let layer = if shape == "large" { "minecraft:tropical_fish_large#pattern" } else { "minecraft:tropical_fish_small#pattern" };
            let pattern = (query("mark_variant").num().max(0.0) as usize).min(5) + 1;
            let texture = format!("fish/tropical_{letter}_pattern_{pattern}");
            vec![draw(Some(layer), &texture, Some(dye(query("color2").num())), Blend::Opaque, Fade::Full)]
        }
        "iron_golem" if !invisible => {
            // Bedrock's render controller picks by `health / 25`; 0 is a health never sent, not a dying golem.
            let health = query("health").num();
            let level = match health {
                h if h <= 0.0 || h >= 75.0 => return Vec::new(),
                h if h < 25.0 => "high",
                h if h < 50.0 => "medium",
                _ => "low",
            };
            vec![draw(None, &format!("iron_golem/iron_golem_crackiness_{level}"), None, Blend::Opaque, Fade::Full)]
        }
        "copper_golem" if !invisible => {
            vec![draw(None, &format!("copper_golem/copper_golem_eyes{}", oxidation(query)), None, Blend::Eyes, Fade::Full)]
        }
        "breeze" => vec![draw(Some("minecraft:breeze#wind"), "breeze/breeze_wind", None, Blend::Wind, Fade::Full)],
        "creaking" if !invisible && creaking_eyes(query) => {
            vec![draw(Some("minecraft:creaking#eyes"), "creaking/creaking_eyes", None, Blend::Eyes, Fade::Full)]
        }
        "warden" if !invisible => {
            let spots = Some("minecraft:warden#pulsating_spots");
            vec![
                draw(Some("minecraft:warden#bioluminescent"), "warden/warden_bioluminescent_layer", None, Blend::Eyes, Fade::Full),
                draw(spots, "warden/warden_pulsating_spots_1", None, Blend::Eyes, Fade::Pulse(0.0)),
                draw(spots, "warden/warden_pulsating_spots_2", None, Blend::Eyes, Fade::Pulse(std::f32::consts::PI)),
            ]
        }
        _ => Vec::new(),
    }
}

/// Java's `Creaking.isActive`: Bedrock's controller glows the eyes in the hostile states.
fn creaking_eyes(query: &dyn Fn(&str) -> Value) -> bool {
    matches!(query("property:minecraft:creaking_state"), Value::Text(s) if s.starts_with("hostile"))
}
