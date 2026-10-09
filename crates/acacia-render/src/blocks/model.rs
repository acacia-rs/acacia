//! Blocks drawn as entity models instead of quads: chests, beds, signs, heads and banners. See README
//! "Block models".

use std::sync::Arc;

use acacia_world::BlockState;
use glam::{DVec3, IVec3};

use super::shape::short_name;
use crate::banner::Banner;

pub const CHEST: &str = "geometry.acacia.chest";
pub const DOUBLE_CHEST: &str = "geometry.acacia.double_chest";
pub const SIGN: &str = "geometry.acacia.sign";
pub const WALL_SIGN: &str = "geometry.acacia.wall_sign";
/// Hanging signs: under a block by two chains, by chains meeting in the middle (`attached_bit`),
/// and under a wall's bracket.
pub const HANGING_SIGNS: [&str; 3] = ["geometry.acacia.hanging_sign", "geometry.acacia.hanging_sign.attached", "geometry.acacia.hanging_sign.wall"];
pub const BED: &str = "geometry.acacia.bed";
/// Floor and wall geometry per head shape.
pub const MOB_HEAD: [&str; 2] = ["geometry.acacia.mob_head", "geometry.acacia.mob_head.wall"];
pub const PLAYER_HEAD: [&str; 2] = ["geometry.acacia.player_head", "geometry.acacia.player_head.wall"];
pub const DRAGON_HEAD: [&str; 2] = ["geometry.acacia.dragon_head", "geometry.acacia.dragon_head.wall"];
/// Standing and on a wall.
pub const BANNERS: [&str; 2] = ["geometry.acacia.banner", "geometry.acacia.banner.wall"];
/// What a banner draws until its texture is composed ([`crate::banner`]).
pub const BANNER_TEXTURE: &str = "textures/entity/banner/banner_base";

/// Bed textures by the `color` of the block entity, white first.
const BED_COLORS: [&str; 16] =
    ["white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "silver", "cyan", "purple", "blue", "brown", "green", "red", "black"];
const RED: u8 = 14;
/// A floor head without a turn faces north, as in Java; assumed for Bedrock's `Rotation`.
const FLOOR_HEAD_YAW: f32 = 180.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Chest,
    BedHead,
    /// Drawn by its head piece.
    BedFoot,
    Sign,
    FloorHead,
    WallHead,
    Banner,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockModel {
    pub kind: Kind,
    pub geometry: &'static str,
    /// Image path in the pack, without extension.
    pub texture: String,
    /// Degrees, like an entity's: 0 faces south.
    pub yaw: f32,
}

/// What a block entity's data adds to the block state; the caller reads it from the NBT.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BlockData {
    /// A banner's cloth and patterns.
    pub banner: Option<Arc<Banner>>,
    /// A bed's `color`.
    pub color: Option<u8>,
    /// A floor head's `Rotation` in degrees.
    pub rotation: Option<f32>,
    /// The other half of a double chest (`pairx`, `pairz`) and whether this half draws both (`pairlead`).
    pub pair: Option<([i32; 2], bool)>,
}

/// A model to draw: its origin sits at `position`.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    pub geometry: &'static str,
    pub texture: String,
    pub position: DVec3,
    pub yaw: f32,
    /// Composed into the texture drawn in place of `texture`; plain white without its block entity.
    pub banner: Option<Arc<Banner>>,
}

impl BlockModel {
    /// `None` for the half of a bed or double chest that the other half draws.
    pub fn place(&self, pos: IVec3, data: Option<&BlockData>) -> Option<Placement> {
        let centre = pos.as_dvec3() + DVec3::new(0.5, 0.0, 0.5);
        let mut placed = Placement { geometry: self.geometry, texture: self.texture.clone(), position: centre, yaw: self.yaw, banner: None };
        match self.kind {
            Kind::BedFoot => return None,
            Kind::Banner => placed.banner = Some(data.and_then(|d| d.banner.clone()).unwrap_or_default()),
            Kind::BedHead => {
                let color = data.and_then(|d| d.color).unwrap_or(RED);
                placed.texture = format!("textures/entity/bed/{}", BED_COLORS[usize::from(color) % BED_COLORS.len()]);
            }
            Kind::FloorHead => placed.yaw = data.and_then(|d| d.rotation).map_or(self.yaw, |turn| turn + FLOOR_HEAD_YAW),
            Kind::Chest => {
                if let Some(([x, z], lead)) = data.and_then(|d| d.pair) {
                    if !lead {
                        return None;
                    }
                    placed.geometry = DOUBLE_CHEST;
                    placed.texture = double_texture(&self.texture);
                    placed.position = (centre + DVec3::new(f64::from(x) + 0.5, centre.y, f64::from(z) + 0.5)) / 2.0;
                }
            }
            Kind::Sign | Kind::WallHead => {}
        }
        Some(placed)
    }
}

/// `chest/normal` to `chest/double_normal`; the others append `_double`.
fn double_texture(single: &str) -> String {
    match single.strip_suffix("normal") {
        Some(dir) => format!("{dir}double_normal"),
        None => format!("{single}_double"),
    }
}

pub fn classify(state: &BlockState) -> Option<BlockModel> {
    let name = short_name(state.name);
    let int = |key: &str| state.property(key).and_then(|v| v.parse::<u8>().ok()).unwrap_or(0);
    let model = |kind, geometry, texture: String, yaw| Some(BlockModel { kind, geometry, texture, yaw });
    if name == "bed" {
        let kind = if int("head_piece_bit") == 1 { Kind::BedHead } else { Kind::BedFoot };
        return model(kind, BED, String::new(), f32::from(int("direction")) * 90.0);
    }
    if let Some(texture) = chest_texture(name) {
        let yaw = match state.property("minecraft:cardinal_direction") {
            Some("west") => 90.0,
            Some("north") => 180.0,
            Some("east") => 270.0,
            _ => 0.0,
        };
        return model(Kind::Chest, CHEST, format!("textures/entity/chest/{texture}"), yaw);
    }
    if let Some(wood) = name.strip_suffix("standing_sign") {
        return model(Kind::Sign, SIGN, sign_texture(wood), f32::from(int("ground_sign_direction")) * 22.5);
    }
    if let Some(wood) = name.strip_suffix("wall_sign") {
        return model(Kind::Sign, WALL_SIGN, sign_texture(wood), facing_yaw(int("facing_direction")));
    }
    if name.ends_with("hanging_sign") {
        let texture = format!("textures/entity/{name}");
        return match (int("hanging"), int("attached_bit")) {
            (1, 1) => model(Kind::Sign, HANGING_SIGNS[1], texture, f32::from(int("ground_sign_direction")) * 22.5),
            (hanging, _) => model(Kind::Sign, HANGING_SIGNS[usize::from(hanging == 0) * 2], texture, facing_yaw(int("facing_direction"))),
        };
    }
    match name {
        "standing_banner" => return model(Kind::Banner, BANNERS[0], BANNER_TEXTURE.into(), f32::from(int("ground_sign_direction")) * 22.5),
        "wall_banner" => return model(Kind::Banner, BANNERS[1], BANNER_TEXTURE.into(), facing_yaw(int("facing_direction"))),
        _ => {}
    }
    let (shape, texture) = match name {
        "skeleton_skull" => (MOB_HEAD, "skulls/skeleton"),
        "wither_skeleton_skull" => (MOB_HEAD, "skulls/wither_skeleton"),
        "zombie_head" => (MOB_HEAD, "skulls/zombie"),
        "creeper_head" => (MOB_HEAD, "skulls/creeper"),
        "player_head" => (PLAYER_HEAD, "steve"),
        "dragon_head" => (DRAGON_HEAD, "dragon/dragon"),
        _ => return None,
    };
    let facing = int("facing_direction");
    let (kind, geometry, yaw) = if facing < 2 { (Kind::FloorHead, shape[0], FLOOR_HEAD_YAW) } else { (Kind::WallHead, shape[1], facing_yaw(facing)) };
    model(kind, geometry, format!("textures/entity/{texture}"), yaw)
}

/// `facing_direction` 2..=5 is north, south, west, east.
fn facing_yaw(facing: u8) -> f32 {
    match facing {
        2 => 180.0,
        4 => 90.0,
        5 => 270.0,
        _ => 0.0,
    }
}

/// The file under `textures/entity/chest` for a chest block.
fn chest_texture(name: &str) -> Option<String> {
    let name = name.strip_prefix("waxed_").unwrap_or(name);
    match name {
        "chest" => Some("normal".into()),
        "trapped_chest" => Some("trapped".into()),
        "ender_chest" => Some("ender".into()),
        "copper_chest" => Some("copper_default".into()),
        _ => name.strip_suffix("_copper_chest").map(|stage| format!("copper_{stage}")),
    }
}

/// `wood` is the block name's prefix (`spruce_`, empty for oak). Older woods are `sign_<wood>`,
/// newer ones `<wood>_sign`.
fn sign_texture(wood: &str) -> String {
    match wood.strip_suffix('_') {
        None => "textures/entity/sign".into(),
        Some(wood) if NEW_WOODS.contains(&wood) => format!("textures/entity/{wood}_sign"),
        Some(wood) => format!("textures/entity/sign_{wood}"),
    }
}

const NEW_WOODS: [&str; 4] = ["mangrove", "cherry", "bamboo", "pale_oak"];

#[cfg(test)]
mod tests {
    use super::*;

    fn model(name: &str, props: &[&str]) -> Option<BlockModel> {
        let reg = acacia_world::BlockRegistry::vanilla();
        let mut states = (0..reg.len() as u32).filter_map(|id| reg.get(id));
        let state = states.find(|s| short_name(s.name) == name && props.iter().all(|p| s.properties.contains(p)));
        classify(state.unwrap_or_else(|| panic!("no state {name} {props:?}")))
    }

    #[test]
    fn states_pick_geometry_texture_and_turn() {
        let chest = model("chest", &["cardinal_direction=west"]).unwrap();
        assert_eq!((chest.geometry, chest.texture.as_str(), chest.yaw), (CHEST, "textures/entity/chest/normal", 90.0));
        assert_eq!(model("waxed_exposed_copper_chest", &[]).unwrap().texture, "textures/entity/chest/copper_exposed");
        let sign = model("darkoak_standing_sign", &["ground_sign_direction=4"]).unwrap();
        assert_eq!((sign.geometry, sign.texture.as_str(), sign.yaw), (SIGN, "textures/entity/sign_darkoak", 90.0));
        let wall = model("cherry_wall_sign", &["facing_direction=2"]).unwrap();
        assert_eq!((wall.geometry, wall.texture.as_str(), wall.yaw), (WALL_SIGN, "textures/entity/cherry_sign", 180.0));
        assert_eq!(model("wall_sign", &[]).unwrap().texture, "textures/entity/sign");
        assert_eq!(model("zombie_head", &["facing_direction=5"]).unwrap().geometry, MOB_HEAD[1]);
        assert_eq!(model("player_head", &["facing_direction=1"]).unwrap().kind, Kind::FloorHead);
        let hanging = model("oak_hanging_sign", &["hanging=1", "attached_bit=1", "ground_sign_direction=2"]).unwrap();
        assert_eq!((hanging.geometry, hanging.texture.as_str(), hanging.yaw), (HANGING_SIGNS[1], "textures/entity/oak_hanging_sign", 45.0));
        assert_eq!(model("dark_oak_hanging_sign", &["hanging=1", "attached_bit=0", "facing_direction=4"]).unwrap().geometry, HANGING_SIGNS[0]);
        let bracket = model("cherry_hanging_sign", &["hanging=0", "facing_direction=5"]).unwrap();
        assert_eq!((bracket.geometry, bracket.yaw), (HANGING_SIGNS[2], 270.0));
    }

    #[test]
    fn block_entity_data_colours_beds_and_joins_chests() {
        let head = model("bed", &["head_piece_bit=1", "direction=2"]).unwrap();
        let at = IVec3::new(4, 64, -3);
        assert_eq!(head.place(at, None).unwrap().texture, "textures/entity/bed/red");
        let blue = BlockData { color: Some(11), ..Default::default() };
        let placed = head.place(at, Some(&blue)).unwrap();
        assert_eq!((placed.texture.as_str(), placed.yaw, placed.position), ("textures/entity/bed/blue", 180.0, DVec3::new(4.5, 64.0, -2.5)));
        assert_eq!(model("bed", &["head_piece_bit=0"]).unwrap().place(at, None), None);

        let chest = model("trapped_chest", &[]).unwrap();
        let lead = BlockData { pair: Some(([5, -3], true)), ..Default::default() };
        let double = chest.place(at, Some(&lead)).unwrap();
        assert_eq!((double.geometry, double.texture.as_str()), (DOUBLE_CHEST, "textures/entity/chest/trapped_double"));
        assert_eq!(double.position, DVec3::new(5.0, 64.0, -2.5));
        assert_eq!(chest.place(at, Some(&BlockData { pair: Some(([5, -3], false)), ..Default::default() })), None);
        assert_eq!(double_texture("textures/entity/chest/normal"), "textures/entity/chest/double_normal");
    }

    #[test]
    fn banners_turn_by_state_and_carry_their_block_entity() {
        let standing = model("standing_banner", &["ground_sign_direction=6"]).unwrap();
        assert_eq!((standing.geometry, standing.yaw), (BANNERS[0], 135.0));
        let wall = model("wall_banner", &["facing_direction=4"]).unwrap();
        assert_eq!((wall.geometry, wall.yaw, wall.texture.as_str()), (BANNERS[1], 90.0, BANNER_TEXTURE));
        let red = Arc::new(Banner { base: 14, layers: vec![("border", 15)], ominous: false });
        let data = BlockData { banner: Some(red.clone()), ..Default::default() };
        assert_eq!(wall.place(IVec3::ZERO, Some(&data)).unwrap().banner, Some(red));
        assert_eq!(wall.place(IVec3::ZERO, None).unwrap().banner, Some(Arc::default()));
    }
}
