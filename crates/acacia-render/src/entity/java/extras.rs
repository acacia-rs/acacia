//! What kinds' own renderers change on the body and add over it beyond [`super::kinds`]' table:
//! the `entity_translucent` bodies, `PufferfishRenderer`'s model by puff, `TropicalFishRenderer`'s
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
pub const LAYERS: [&str; 11] = [
    "minecraft:pufferfish_small#main", "minecraft:pufferfish_medium#main", "minecraft:pufferfish_big#main",
    "minecraft:tropical_fish_small#main", "minecraft:tropical_fish_large#main", "minecraft:tropical_fish_small#pattern",
    "minecraft:tropical_fish_large#pattern", "minecraft:breeze#wind", "minecraft:creaking#eyes", "minecraft:warden#bioluminescent",
    "minecraft:warden#pulsating_spots",
];

/// Every texture [`skin`] and [`draws`] can name, for registering up front.
pub fn textures() -> impl Iterator<Item = String> {
    let patterns = ["a", "b"].into_iter().flat_map(|shape| (1..=6).map(move |i| format!("fish/tropical_{shape}_pattern_{i}")));
    let fixed = [
        "fish/tropical_a", "fish/tropical_b", "iron_golem/iron_golem_crackiness_low", "iron_golem/iron_golem_crackiness_medium",
        "iron_golem/iron_golem_crackiness_high", "breeze/breeze_wind", "creaking/creaking_eyes", "warden/warden_bioluminescent_layer",
        "warden/warden_pulsating_spots_1", "warden/warden_pulsating_spots_2",
    ];
    fixed.into_iter().map(str::to_owned).chain(patterns).map(|name| entity_texture(&name))
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
