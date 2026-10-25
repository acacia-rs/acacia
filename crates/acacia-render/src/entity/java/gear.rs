//! What the render layers for saddles, body armour, carpets, harnesses, collars and markings add to
//! a kind drawn by a Java model, in their renderers' order: `SimpleEquipmentLayer`,
//! `WolfArmorLayer` (with its cracks), `LlamaDecorLayer`, `CatCollarLayer`, `WolfCollarLayer`,
//! `HorseMarkingLayer` and the happy ghast's `RopesLayer`.

use super::Family;
use super::equipment::{self, Equipment};
use crate::entity::{Blend, DYES};

/// What an entity wears, as its render state carries it.
#[derive(Debug, Clone, Default)]
pub struct Gear<'a> {
    pub baby: bool,
    pub invisible: bool,
    pub saddled: bool,
    pub body: Option<Body<'a>>,
    /// The collar's dye (white first), on a tame wolf or cat.
    pub collar: Option<usize>,
    /// `Markings` (none, white, white field, white dots, black dots).
    pub markings: usize,
    pub leash_holder: bool,
}

/// The body slot's stack.
#[derive(Debug, Clone, Default)]
pub struct Body<'a> {
    pub item: &'a str,
    pub dye: Option<[u8; 3]>,
    /// Durability left, `(max - damage) / max`; `None` for an undamageable item.
    pub durability: Option<f32>,
}

/// One draw: the model layer (`None`: the body's own model), the texture under the look's root,
/// its colour and how it blends.
#[derive(Debug, Clone, PartialEq)]
pub struct Draw {
    pub layer: Option<&'static str>,
    pub texture: String,
    pub color: Option<[u8; 3]>,
    pub blend: Blend,
}

#[derive(Clone, Copy)]
enum From {
    Saddle,
    Body,
}

/// A `SimpleEquipmentLayer`: where its stack comes from, the layer type, and the adult's and
/// baby's model layers (no baby model: babies draw none).
type Slot = (From, &'static str, &'static str, Option<&'static str>);

const HORSE_BODY: Slot = (From::Body, "horse_body", "minecraft:horse_armor#main", None);
const UNDEAD_BODY: Slot = (From::Body, "horse_body", "minecraft:undead_horse_armor#main", None);
const NAUTILUS: [Slot; 2] = [(From::Body, "nautilus_body", "minecraft:nautilus_armor#main", None), (From::Saddle, "nautilus_saddle", "minecraft:nautilus#saddle", None)];

fn slots(kind: &str) -> &'static [Slot] {
    match kind {
        "pig" => &[(From::Saddle, "pig_saddle", "minecraft:pig#saddle", None)],
        "strider" => &[(From::Saddle, "strider_saddle", "minecraft:strider#saddle", None)],
        "horse" => &[HORSE_BODY, (From::Saddle, "horse_saddle", "minecraft:horse#saddle", None)],
        "donkey" => &[(From::Saddle, "donkey_saddle", "minecraft:donkey#saddle", None)],
        "mule" => &[(From::Saddle, "mule_saddle", "minecraft:mule#saddle", None)],
        "skeleton_horse" => &[UNDEAD_BODY, (From::Saddle, "skeleton_horse_saddle", "minecraft:skeleton_horse#saddle", None)],
        "zombie_horse" => &[UNDEAD_BODY, (From::Saddle, "zombie_horse_saddle", "minecraft:zombie_horse#saddle", None)],
        "camel" => &[(From::Saddle, "camel_saddle", "minecraft:camel#saddle", None)],
        "camel_husk" => &[(From::Saddle, "camel_husk_saddle", "minecraft:camel_husk#saddle", None)],
        "nautilus" | "zombie_nautilus" => &NAUTILUS,
        "happy_ghast" => &[(From::Body, "happy_ghast_body", "minecraft:happy_ghast_harness#main", Some("minecraft:happy_ghast_baby_harness#main"))],
        _ => &[],
    }
}

/// Every model layer [`draws`] can name, for baking up front.
pub const LAYERS: [&str; 22] = [
    "minecraft:pig#saddle", "minecraft:strider#saddle", "minecraft:horse_armor#main", "minecraft:horse#saddle", "minecraft:donkey#saddle",
    "minecraft:mule#saddle", "minecraft:undead_horse_armor#main", "minecraft:skeleton_horse#saddle", "minecraft:zombie_horse#saddle",
    "minecraft:camel#saddle", "minecraft:camel_husk#saddle", "minecraft:nautilus_armor#main", "minecraft:nautilus#saddle",
    "minecraft:happy_ghast_harness#main", "minecraft:happy_ghast_baby_harness#main", "minecraft:happy_ghast_ropes#main",
    "minecraft:happy_ghast_baby_ropes#main", "minecraft:wolf_armor#main", "minecraft:llama#decor", "minecraft:llama_baby#decor",
    "minecraft:cat#collar", "minecraft:cat_baby#collar",
];

/// Every texture [`draws`] can name outside the equipment assets, for registering up front.
pub const TEXTURES: [&str; 16] = [
    "wolf/wolf_armor_crackiness_low", "wolf/wolf_armor_crackiness_medium", "wolf/wolf_armor_crackiness_high", "wolf/wolf_collar",
    "wolf/wolf_collar_baby", "cat/cat_collar", "cat/cat_collar_baby", "horse/horse_markings_white", "horse/horse_markings_white_baby",
    "horse/horse_markings_whitefield", "horse/horse_markings_whitefield_baby", "horse/horse_markings_whitedots",
    "horse/horse_markings_whitedots_baby", "horse/horse_markings_blackdots", "horse/horse_markings_blackdots_baby", "ghast/happy_ghast_ropes",
];

/// The `setupAnim` of a [`LAYERS`] model.
pub fn family(layer: &str) -> Option<Family> {
    Some(match layer {
        "minecraft:horse#saddle" | "minecraft:donkey#saddle" | "minecraft:mule#saddle" | "minecraft:skeleton_horse#saddle" | "minecraft:zombie_horse#saddle" => {
            Family::EquineSaddle
        }
        "minecraft:camel#saddle" | "minecraft:camel_husk#saddle" => Family::CamelSaddle,
        "minecraft:happy_ghast_harness#main" | "minecraft:happy_ghast_baby_harness#main" => Family::HappyGhastHarness,
        "minecraft:happy_ghast_ropes#main" | "minecraft:happy_ghast_baby_ropes#main" => Family::HappyGhast,
        "minecraft:horse_armor#main" | "minecraft:undead_horse_armor#main" => Family::Horse,
        "minecraft:nautilus_armor#main" | "minecraft:nautilus#saddle" => Family::Nautilus,
        "minecraft:wolf_armor#main" => Family::Wolf,
        "minecraft:llama#decor" | "minecraft:llama_baby#decor" => Family::Llama,
        "minecraft:cat#collar" | "minecraft:cat_baby#collar" => Family::Feline,
        "minecraft:pig#saddle" => Family::Quadruped,
        "minecraft:strider#saddle" => Family::Strider,
        _ => return None,
    })
}

pub fn entity_texture(name: &str) -> String {
    format!("java/textures/entity/{name}.png")
}

/// The draws a `kind` (Java id, `minecraft:` optional) wearing `gear` adds over its body, in order.
pub fn draws(kind: &str, gear: &Gear, equipment: &Equipment) -> Vec<Draw> {
    let kind = kind.trim_start_matches("minecraft:");
    let mut out = Vec::new();
    let wear = |out: &mut Vec<Draw>, layer: &'static str, layer_type: &str, asset: &str, dye: Option<[u8; 3]>| {
        for d in equipment.draws(asset, layer_type, dye).into_iter().flatten() {
            out.push(Draw { layer: Some(layer), texture: d.texture, color: d.color, blend: Blend::Opaque });
        }
    };
    let body = gear.body.as_ref().and_then(|b| Some((b, equipment::asset(b.item)?)));
    if kind == "horse" {
        markings(&mut out, gear);
    }
    for &(from, layer_type, adult, baby) in slots(kind) {
        let model = if gear.baby { baby } else { Some(adult) };
        let (Some(model), worn) = (model, match from {
            From::Saddle => gear.saddled.then_some(("saddle", None)),
            From::Body => body.map(|(b, asset)| (asset, b.dye)),
        }) else {
            continue;
        };
        if let Some((asset, dye)) = worn {
            wear(&mut out, model, layer_type, asset, dye);
        }
    }
    match kind {
        "happy_ghast" if gear.leash_holder && body.is_some_and(|(b, _)| b.item.trim_start_matches("minecraft:").ends_with("_harness")) => {
            let layer = if gear.baby { "minecraft:happy_ghast_baby_ropes#main" } else { "minecraft:happy_ghast_ropes#main" };
            out.push(Draw { layer: Some(layer), texture: entity_texture("ghast/happy_ghast_ropes"), color: None, blend: Blend::Opaque });
        }
        "wolf" => {
            if let Some((b, asset)) = body.filter(|_| !gear.baby) {
                wear(&mut out, "minecraft:wolf_armor#main", "wolf_body", asset, b.dye);
                if let Some(level) = b.durability.and_then(cracks) {
                    let texture = entity_texture(&format!("wolf/wolf_armor_crackiness_{level}"));
                    out.push(Draw { layer: Some("minecraft:wolf_armor#main"), texture, color: None, blend: Blend::Alpha });
                }
            }
            collar(&mut out, gear, "wolf/wolf_collar", None);
        }
        "llama" | "trader_llama" => {
            let model = if gear.baby { "minecraft:llama_baby#decor" } else { "minecraft:llama#decor" };
            match (body.filter(|_| !gear.baby), kind) {
                (Some((b, asset)), _) => wear(&mut out, model, "llama_body", asset, b.dye),
                (None, "trader_llama") => wear(&mut out, model, "llama_body", if gear.baby { "trader_llama_baby" } else { "trader_llama" }, None),
                _ => {}
            }
        }
        "cat" => collar(&mut out, gear, "cat/cat_collar", Some(if gear.baby { "minecraft:cat_baby#collar" } else { "minecraft:cat#collar" })),
        _ => {}
    }
    out
}

/// `Crackiness.WOLF_ARMOR.byFraction`'s level name, `None` while barely worn.
fn cracks(left: f32) -> Option<&'static str> {
    if left < 0.32 {
        Some("high")
    } else if left < 0.69 {
        Some("medium")
    } else {
        (left < 0.95).then_some("low")
    }
}

/// A collar in its dye's diffuse colour, hidden while invisible; `layer` `None`: on the body's model.
fn collar(out: &mut Vec<Draw>, gear: &Gear, texture: &str, layer: Option<&'static str>) {
    let Some(dye) = gear.collar.filter(|_| !gear.invisible) else { return };
    let [_, r, g, b] = (DYES[dye % DYES.len()]).to_be_bytes();
    let texture = entity_texture(&format!("{texture}{}", if gear.baby { "_baby" } else { "" }));
    out.push(Draw { layer, texture, color: Some([r, g, b]), blend: Blend::Opaque });
}

fn markings(out: &mut Vec<Draw>, gear: &Gear) {
    const NAMES: [&str; 5] = ["", "white", "whitefield", "whitedots", "blackdots"];
    let Some(name) = NAMES.get(gear.markings).filter(|n| !n.is_empty() && !gear.invisible) else { return };
    let texture = entity_texture(&format!("horse/horse_markings_{name}{}", if gear.baby { "_baby" } else { "" }));
    out.push(Draw { layer: None, texture, color: None, blend: Blend::Alpha });
}
