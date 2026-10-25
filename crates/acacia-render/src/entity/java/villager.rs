//! `VillagerProfessionLayer` (26.3 client) for villagers and zombie villagers: over the body, the
//! biome type's clothes (the no-hat model when the profession's hat covers the type's), then the
//! profession's and its level's badge, adults only. Bedrock's `variant` is the profession,
//! `mark_variant` the biome type and `trade_tier` the level less one.

use std::collections::HashMap;
use std::path::Path;

use crate::entity::molang::Value;
use crate::entity::{Layer, ModelId, TextureId};

const PROFESSIONS: [&str; 15] = [
    "none", "farmer", "fisherman", "shepherd", "fletcher", "librarian", "cartographer", "cleric", "armorer", "weaponsmith", "toolsmith",
    "butcher", "leatherworker", "mason", "nitwit",
];
const TYPES: [&str; 7] = ["plains", "desert", "jungle", "savanna", "snow", "swamp", "taiga"];
const LEVELS: [&str; 5] = ["stone", "iron", "gold", "emerald", "diamond"];

/// `VillagerMetadataSection.Hat`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Hat {
    None,
    Partial,
    Full,
}

pub struct Professions {
    /// The adult's and the baby's no-hat model.
    no_hat: [Option<ModelId>; 2],
    /// By path under the kind's directory (`type/snow`): the texture and its hat.
    textures: HashMap<String, (TextureId, Hat)>,
}

/// The `villager.hat` of an image's `.mcmeta`; `None` without one.
fn hat(file: &Path) -> Hat {
    let Ok(text) = std::fs::read_to_string(file) else { return Hat::None };
    let json: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    match json["villager"]["hat"].as_str() {
        Some("full") => Hat::Full,
        Some("partial") => Hat::Partial,
        _ => Hat::None,
    }
}

impl Professions {
    /// The textures under `java/textures/entity/{dir}` of the look at `root`.
    pub fn load(root: &Path, dir: &str, no_hat: [Option<ModelId>; 2], texture: &mut dyn FnMut(&str) -> Option<TextureId>) -> Professions {
        let mut textures = HashMap::new();
        let names = TYPES.iter().map(|t| ("type", *t)).chain(TYPES.iter().map(|t| ("baby", *t)));
        let names = names.chain(PROFESSIONS[1..].iter().map(|p| ("profession", *p))).chain(LEVELS.iter().map(|l| ("profession_level", *l)));
        for (folder, name) in names {
            let path = format!("java/textures/entity/{dir}/{folder}/{name}");
            if let Some(id) = texture(&path) {
                textures.insert(format!("{folder}/{name}"), (id, hat(&root.join(format!("{path}.png.mcmeta")))));
            }
        }
        Professions { no_hat, textures }
    }

    /// The layers over a body drawn with `model`.
    pub fn layers(&self, model: ModelId, baby: bool, query: &dyn Fn(&str) -> Value) -> Vec<Layer> {
        let index = |name: &str, len: usize| (query(name).num().max(0.0) as usize).min(len - 1);
        let kind = TYPES[index("mark_variant", TYPES.len())];
        let profession = PROFESSIONS[index("variant", PROFESSIONS.len())];
        let get = |path: String| self.textures.get(&path).copied();
        let type_hat = get(format!("type/{kind}")).map_or(Hat::None, |(_, h)| h);
        let profession_hat = get(format!("profession/{profession}")).map_or(Hat::None, |(_, h)| h);
        let hat_visible = profession_hat == Hat::None || profession_hat == Hat::Partial && type_hat != Hat::Full;
        let mut out = Vec::new();
        let clothes = get(format!("{}/{kind}", if baby { "baby" } else { "type" }));
        let clothes_model = if hat_visible { Some(model) } else { self.no_hat[usize::from(baby)] };
        if let (Some((texture, _)), Some(model)) = (clothes, clothes_model) {
            out.push(Layer::plain(model, texture));
        }
        if profession != "none" && !baby {
            out.extend(get(format!("profession/{profession}")).map(|(t, _)| Layer::plain(model, t)));
            if profession != "nitwit" {
                let level = LEVELS[index("trade_tier", LEVELS.len())];
                out.extend(get(format!("profession_level/{level}")).map(|(t, _)| Layer::plain(model, t)));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_profession_hat_takes_the_type_hat_off() {
        let textures = [("type/snow", 1, Hat::Full), ("type/plains", 2, Hat::None), ("profession/librarian", 3, Hat::Full)];
        let textures = textures.into_iter().map(|(p, t, h)| (p.to_owned(), (t, h))).chain(LEVELS.iter().map(|l| (format!("profession_level/{l}"), (9, Hat::None))));
        let professions = Professions { no_hat: [Some(7), None], textures: textures.collect() };
        let query = |variant: f32, mark: f32| move |name: &str| Value::Num(match name {
            "variant" => variant,
            "mark_variant" => mark,
            _ => 0.0,
        });
        let drawn = |layers: Vec<Layer>| layers.iter().map(|l| (l.model, l.textures[0])).collect::<Vec<_>>();
        assert_eq!(drawn(professions.layers(5, false, &query(5.0, 4.0))), [(7, 1), (5, 3), (5, 9)]);
        assert_eq!(drawn(professions.layers(5, false, &query(0.0, 0.0))), [(5, 2)]);
        assert_eq!(drawn(professions.layers(5, true, &query(5.0, 4.0))), []);
    }
}
