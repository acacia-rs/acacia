//! Worn armour from the pack's attachables (`attachables/<item>.json`): the item's default geometry
//! (`geometry.humanoid.armor.*`, bones named as a humanoid's) and texture. Drawn as a second
//! instance with the wearer's pose; the `.player` variants are left out (players take the humanoid one).

use std::path::Path;

use crate::assets::json;

/// An item worn as armour: its identifier, geometry and texture path.
pub(super) struct Piece {
    pub item: String,
    pub geometry: String,
    pub texture: String,
}

pub(super) fn pieces(root: &Path) -> Vec<Piece> {
    let Ok(dir) = std::fs::read_dir(root.join("attachables")) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in dir.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if name.ends_with(".player.json") || !name.ends_with(".json") {
            continue;
        }
        let Ok(file) = json::read(&path) else { continue };
        let d = &file["minecraft:attachable"]["description"];
        let (Some(item), Some(geometry), Some(texture)) = (d["identifier"].as_str(), d["geometry"]["default"].as_str(), d["textures"]["default"].as_str()) else { continue };
        if geometry.contains(".armor.") {
            out.push(Piece { item: item.to_owned(), geometry: geometry.to_owned(), texture: texture.to_owned() });
        }
    }
    out
}
