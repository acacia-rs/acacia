//! What bots wear: real skins captured from the vanilla game, one per account. `acacia-mitm` saves
//! every joining player's skin to `<out>/skins/<SkinId>.json`; files copied into `assets/skins/` are
//! embedded at build time (build.rs). Without any, bots fall back to a flat 64x64 skin.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CAPTURED: &[&str] = include!(concat!(env!("OUT_DIR"), "/skins.rs"));
const FLAT_GEOMETRY: &str = include_str!("../../assets/skin_geometry.json");
const FLAT_RESOURCE_PATCH: &str = include_str!("../../assets/skin_resource_patch.json");

/// Every `PieceType` BDS 1.26.52 reads in `PersonaPieces` and `PieceTintColors`; any other name
/// fails the whole Login (README, "Persona piece types"). `hand` is the wire enum's `Hands`.
pub const PERSONA_PIECE_TYPES: [&str; 27] = [
    "persona_skeleton", "persona_body", "persona_skin", "persona_bottom", "persona_feet", "persona_dress",
    "persona_top", "persona_high_pants", "persona_hand", "persona_outerwear", "persona_facial_hair",
    "persona_mouth", "persona_eyes", "persona_hair", "persona_hood", "persona_back", "persona_face_accessory",
    "persona_head", "persona_legs", "persona_left_leg", "persona_right_leg", "persona_arms", "persona_left_arm",
    "persona_right_arm", "persona_capes", "persona_classic_skin", "persona_emote",
];

/// The skin claims of [`super::ClientData`], named as the mitm dump names them.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct Skin {
    pub animated_image_data: Vec<serde_json::Value>,
    pub arm_size: String,
    pub cape_data: String,
    #[serde(rename = "CapeId")]
    pub cape_id: String,
    pub cape_image_height: u32,
    pub cape_image_width: u32,
    pub cape_on_classic_skin: bool,
    pub override_skin: bool,
    pub persona_pieces: Vec<serde_json::Value>,
    pub persona_skin: bool,
    #[serde(rename = "PieceTintColors")]
    pub piece_tint_colours: Vec<serde_json::Value>,
    pub premium_skin: bool,
    pub skin_animation_data: String,
    #[serde(rename = "SkinColor")]
    pub skin_colour: String,
    pub skin_data: String,
    #[serde(rename = "SkinGeometryData")]
    pub skin_geometry: String,
    #[serde(rename = "SkinGeometryDataEngineVersion")]
    pub skin_geometry_version: String,
    #[serde(rename = "SkinId")]
    pub skin_id: String,
    pub skin_image_height: u32,
    pub skin_image_width: u32,
    pub skin_resource_patch: String,
    pub trusted_skin: bool,
}

impl Skin {
    /// The same captured skin for `account` on every join, spread evenly over the pool.
    pub fn for_account(account: &str) -> Self {
        // Salt predates the Acacia rename; changing it gives every account a new skin.
        let digest = Sha256::digest(format!("bedrock-client skin:{account}"));
        let Some(json) = pick(CAPTURED, u64::from_le_bytes(digest[..8].try_into().unwrap())) else {
            return Self::flat();
        };
        let mut skin: Skin = serde_json::from_str(json).expect("assets/skins holds mitm skin dumps");
        skin.personalise(&hex::encode(&digest[8..16]));
        skin
    }

    /// Renames the persona (`persona-<16 hex>-<slot>`, also in the geometry names). The hex belongs
    /// to the player's account (one account kept it across characters, only the slot changed), so
    /// each bot gets its own and pool files store zeros.
    pub fn personalise(&mut self, hex: &str) {
        let Some(old) = self.skin_id.strip_prefix("persona-").and_then(|rest| rest.split('-').next()) else {
            return;
        };
        let old = old.to_owned();
        self.skin_id = self.skin_id.replace(&old, hex);
        for field in [&mut self.skin_resource_patch, &mut self.skin_geometry] {
            if let Ok(text) = STANDARD.decode(&*field).map(String::from_utf8) {
                *field = STANDARD.encode(text.unwrap_or_default().replace(&old, hex));
            }
        }
    }

    /// The first piece or tint `PieceType` outside [`PERSONA_PIECE_TYPES`] (a missing one is `""`).
    pub fn unreadable_piece_type(&self) -> Option<&str> {
        self.persona_pieces
            .iter()
            .chain(&self.piece_tint_colours)
            .map(|piece| piece["PieceType"].as_str().unwrap_or_default())
            .find(|name| !PERSONA_PIECE_TYPES.contains(name))
    }

    /// The flat classic skin wearing a cape of RGBA8 pixels (64×32 is the game's size). Persona
    /// skins carry capes as pieces instead.
    pub fn classic_with_cape(width: u32, height: u32, rgba: &[u8]) -> Self {
        Self {
            cape_data: STANDARD.encode(rgba),
            cape_id: uuid::Uuid::new_v4().to_string(),
            cape_image_width: width,
            cape_image_height: height,
            cape_on_classic_skin: true,
            ..Self::flat()
        }
    }

    fn flat() -> Self {
        Self {
            animated_image_data: Vec::new(),
            arm_size: "wide".into(),
            cape_data: String::new(),
            cape_id: String::new(),
            cape_image_height: 0,
            cape_image_width: 0,
            cape_on_classic_skin: false,
            override_skin: false,
            persona_pieces: Vec::new(),
            persona_skin: false,
            piece_tint_colours: Vec::new(),
            premium_skin: false,
            skin_animation_data: String::new(),
            skin_colour: String::new(),
            skin_data: STANDARD.encode(flat_texture()),
            skin_geometry: STANDARD.encode(FLAT_GEOMETRY),
            skin_geometry_version: STANDARD.encode("0.0.0"),
            skin_id: uuid::Uuid::new_v4().to_string(),
            skin_image_height: 64,
            skin_image_width: 64,
            skin_resource_patch: STANDARD.encode(FLAT_RESOURCE_PATCH),
            trusted_skin: false,
        }
    }
}

fn pick<'a>(pool: &[&'a str], seed: u64) -> Option<&'a str> {
    (!pool.is_empty()).then(|| pool[(seed % pool.len() as u64) as usize])
}

/// 64x64 RGBA: skin-tone base, cyan shirt, blue trousers (Steve's palette, not his texture).
fn flat_texture() -> Vec<u8> {
    const SKIN: [u8; 4] = [0xB4, 0x84, 0x6C, 0xFF];
    const SHIRT: [u8; 4] = [0x00, 0xA8, 0xA8, 0xFF];
    const TROUSERS: [u8; 4] = [0x46, 0x3A, 0xA5, 0xFF];
    let mut img = Vec::with_capacity(64 * 64 * 4);
    for y in 0..64 {
        for x in 0..64 {
            let px = match (x, y) {
                (0..16, 16..32) | (16..32, 48..64) => TROUSERS,
                (16..40, 16..32) => SHIRT,
                _ => SKIN,
            };
            img.extend_from_slice(&px);
        }
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;

    fn persona() -> Skin {
        let mut skin = Skin::flat();
        skin.skin_id = "persona-dfd8de8cea28558c-2".into();
        skin.skin_resource_patch = STANDARD.encode(r#"{"geometry":{"default":"geometry.persona_dfd8de8cea28558c-2"}}"#);
        skin.skin_geometry = STANDARD.encode(r#"{"minecraft:geometry":[{"description":{"identifier":"geometry.persona_dfd8de8cea28558c-2"}}]}"#);
        skin
    }

    #[test]
    fn personalise_renames_the_persona_everywhere() {
        let mut skin = persona();
        skin.personalise("0123456789abcdef");
        assert_eq!(skin.skin_id, "persona-0123456789abcdef-2");
        for field in [&skin.skin_resource_patch, &skin.skin_geometry] {
            let text = String::from_utf8(STANDARD.decode(field).unwrap()).unwrap();
            assert!(text.contains("persona_0123456789abcdef-2") && !text.contains("dfd8de8c"), "{text}");
        }
    }

    #[test]
    fn classic_skins_keep_their_id() {
        let mut skin = Skin::flat();
        let id = skin.skin_id.clone();
        skin.personalise("0123456789abcdef");
        assert_eq!(skin.skin_id, id);
    }

    #[test]
    fn pick_is_stable_and_spreads() {
        let pool = ["a", "b", "c"];
        assert_eq!(pick(&[], 5), None);
        assert_eq!(pick(&pool, 7), pick(&pool, 7));
        let picked: std::collections::HashSet<_> = (0..30).filter_map(|s| pick(&pool, s)).collect();
        assert_eq!(picked.len(), 3);
    }

    #[test]
    fn every_embedded_skin_names_piece_types_bds_reads() {
        for json in CAPTURED {
            let skin: Skin = serde_json::from_str(json).unwrap();
            assert_eq!(skin.unreadable_piece_type(), None, "{}", skin.skin_id);
        }
    }

    #[test]
    fn piece_types_outside_the_list_are_reported() {
        let mut skin = Skin::flat();
        assert_eq!(skin.unreadable_piece_type(), None);
        skin.persona_pieces = vec![serde_json::json!({"PieceType": "persona_hand"}), serde_json::json!({"PieceType": "persona_hands"})];
        assert_eq!(skin.unreadable_piece_type(), Some("persona_hands"));
        skin.persona_pieces.pop();
        skin.piece_tint_colours = vec![serde_json::json!({"Colors": []})];
        assert_eq!(skin.unreadable_piece_type(), Some(""));
    }

    #[test]
    fn every_embedded_skin_parses_and_matches_its_size() {
        for json in CAPTURED {
            let skin: Skin = serde_json::from_str(json).unwrap();
            let pixels = STANDARD.decode(&skin.skin_data).unwrap().len() as u32;
            assert_eq!(pixels, skin.skin_image_width * skin.skin_image_height * 4, "{}", skin.skin_id);
        }
    }
}
