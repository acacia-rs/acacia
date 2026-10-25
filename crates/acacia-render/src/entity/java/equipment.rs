//! Worn armour on Java models, as `HumanoidArmorLayer` and `EquipmentLayerRenderer` draw it: the
//! wearer's slot model (`<layer>#helmet`...) once per layer of the item's equipment asset
//! (`java/equipment/<asset>.json`), each texture `textures/entity/equipment/<layer type>/<name>`,
//! a dyeable one in the stack's dye or its undyed colour.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use super::trims::{self, Trim, Trims};

/// `EquipmentSlot`s an armour layer has a model for, in `#<slot>` layer names.
pub const SLOTS: [&str; 4] = ["helmet", "chestplate", "leggings", "boots"];

#[derive(Deserialize)]
struct Dyeable {
    color_when_undyed: Option<i32>,
}

#[derive(Deserialize)]
struct LayerJson {
    texture: String,
    dyeable: Option<Dyeable>,
}

#[derive(Deserialize)]
struct Asset {
    layers: HashMap<String, Vec<LayerJson>>,
    #[serde(default)]
    trim_overrides: Vec<trims::Override>,
}

/// One draw: the texture (under the look's root) and the colour it is multiplied by (sRGB).
#[derive(Debug, Clone, PartialEq)]
pub struct Draw {
    pub texture: String,
    pub color: Option<[u8; 3]>,
}

#[derive(Default)]
pub struct Equipment(HashMap<String, Asset>);

impl Equipment {
    pub fn load(root: &Path) -> Equipment {
        let Ok(dir) = std::fs::read_dir(root.join("java/equipment")) else { return Equipment::default() };
        let assets = dir.flatten().filter_map(|entry| {
            let path = entry.path();
            let name = path.file_stem()?.to_str()?.to_owned();
            Some((name, serde_json::from_slice(&std::fs::read(&path).ok()?).ok()?))
        });
        Equipment(assets.collect())
    }

    /// Every humanoid layer's texture path, for registering up front.
    pub fn textures(&self) -> impl Iterator<Item = String> + '_ {
        let humanoid = self.0.values().flat_map(|asset| asset.layers.iter().filter(|(kind, _)| kind.starts_with("humanoid")));
        humanoid.flat_map(|(kind, layers)| layers.iter().map(move |layer| texture_path(kind, &layer.texture)))
    }

    /// What `item` draws in its slot on a humanoid (`baby`: the baby's layers); the slot's index
    /// in [`SLOTS`] and the draws, bottom first, its trim's last. `dye` and `trim` are the stack's.
    pub fn worn(&self, item: &str, baby: bool, dye: Option<[u8; 3]>, trim: Option<(&Trims, &Trim)>) -> Option<(usize, Vec<Draw>)> {
        let (asset, slot) = piece(item)?;
        let layer_type = if baby { "humanoid_baby" } else if slot == 2 { "humanoid_leggings" } else { "humanoid" };
        let asset = self.0.get(asset)?;
        let draws = asset.layers.get(layer_type)?.iter().filter_map(|layer| {
            let color = match &layer.dyeable {
                // An undyed layer without a colour of its own is skipped (Java's colour 0).
                Some(d) => Some(dye.or_else(|| d.color_when_undyed.map(rgb))?),
                None => None,
            };
            Some(Draw { texture: texture_path(layer_type, &layer.texture), color })
        });
        let trim = trim.filter(|_| !baby).and_then(|(trims, trim)| trims.texture(&asset.trim_overrides, layer_type, trim));
        Some((slot, draws.chain(trim.map(|texture| Draw { texture, color: None })).collect()))
    }

    /// The image `trim` draws on equipment asset `asset` (`iron`) for `layer_type`.
    pub fn trim_texture(&self, asset: &str, layer_type: &str, trims: &Trims, trim: &Trim) -> Option<String> {
        trims.texture(&self.0.get(asset)?.trim_overrides, layer_type, trim)
    }
}

/// `minecraft:leather` of `humanoid` to its image under the look's root.
fn texture_path(layer_type: &str, texture: &str) -> String {
    let name = texture.strip_prefix("minecraft:").unwrap_or(texture);
    format!("java/textures/entity/equipment/{layer_type}/{name}.png")
}

fn rgb(argb: i32) -> [u8; 3] {
    let [_, r, g, b] = argb.to_be_bytes();
    [r, g, b]
}

/// The equipment asset and slot of a vanilla armour item: `minecraft:golden_boots` is `gold`'s boots.
fn piece(item: &str) -> Option<(&str, usize)> {
    let name = item.strip_prefix("minecraft:").unwrap_or(item);
    if name == "turtle_helmet" {
        return Some(("turtle_scute", 0));
    }
    let (material, piece) = name.rsplit_once('_')?;
    let slot = SLOTS.iter().position(|s| *s == piece)?;
    let asset = match material {
        "golden" => "gold",
        "leather" | "chainmail" | "iron" | "copper" | "diamond" | "netherite" => material,
        _ => return None,
    };
    Some((asset, slot))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEATHER: &str = r#"{"layers": {"humanoid": [{"dyeable": {"color_when_undyed": -6265536}, "texture": "minecraft:leather"}, {"texture": "minecraft:leather_overlay"}],
        "humanoid_leggings": [{"dyeable": {"color_when_undyed": -6265536}, "texture": "minecraft:leather"}, {"texture": "minecraft:leather_overlay"}]}}"#;

    #[test]
    fn leather_takes_its_dye_or_the_undyed_brown() {
        let equipment = Equipment(HashMap::from([("leather".to_owned(), serde_json::from_str(LEATHER).unwrap())]));
        let (slot, draws) = equipment.worn("minecraft:leather_leggings", false, None, None).unwrap();
        assert_eq!(slot, 2);
        assert_eq!(draws[0], Draw { texture: "java/textures/entity/equipment/humanoid_leggings/leather.png".into(), color: Some([0xA0, 0x65, 0x40]) });
        assert_eq!(draws[1].color, None);
        let (_, dyed) = equipment.worn("minecraft:leather_helmet", false, Some([1, 2, 3]), None).unwrap();
        assert_eq!(dyed[0].color, Some([1, 2, 3]));
        assert!(equipment.worn("minecraft:leather_helmet", true, None, None).is_none());
    }

    #[test]
    fn an_unknown_trim_adds_nothing() {
        let equipment = Equipment(HashMap::from([("leather".to_owned(), serde_json::from_str(LEATHER).unwrap())]));
        let trims = Trims::default();
        let trim = Trim { material: "iron".into(), pattern: "coast".into() };
        let (_, draws) = equipment.worn("minecraft:leather_helmet", false, None, Some((&trims, &trim))).unwrap();
        assert_eq!(draws.len(), 2);
    }

    #[test]
    fn items_name_their_asset() {
        assert_eq!(piece("minecraft:golden_chestplate"), Some(("gold", 1)));
        assert_eq!(piece("minecraft:turtle_helmet"), Some(("turtle_scute", 0)));
        assert_eq!(piece("minecraft:diamond_sword"), None);
    }
}
