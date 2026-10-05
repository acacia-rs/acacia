//! Meshes for the blocks of [`crate::blocks::model`]. The game hard-codes the chest and sign
//! models (`block_models.json`, after Java's `ChestModel` and `SignModel`); beds and heads come
//! from the pack, moved to where the block draws them.

use std::collections::HashMap;
use std::path::Path;

use glam::{Mat4, Vec3, Vec4};

use super::bake::{self, Mesh};
use super::geometry::{self, Geometry};
use crate::blocks::model::{BED, CHEST, DOUBLE_CHEST, DRAGON_HEAD, MOB_HEAD, PLAYER_HEAD, SIGN, WALL_SIGN};

const BUILT_IN: &str = include_str!("block_models.json");
/// Signs are drawn at two thirds of their model.
const SIGN_SCALE: f32 = 2.0 / 3.0;
const BED_TEXTURE: [f32; 2] = [64.0, 64.0];

/// Every block model that could be built, by the id blocks name it with. `pack` holds the
/// pack's geometries.
pub(super) fn meshes(pack: &HashMap<String, Geometry>) -> Vec<(&'static str, Mesh)> {
    let built_in = geometry::parse(BUILT_IN);
    let sign = Mat4::from_scale(Vec3::splat(SIGN_SCALE));
    // The pack's bed stands upright with its head at the top: lay it on its legs, head forward.
    let bed = Mat4::from_cols(Vec4::new(-1.0, 0.0, 0.0, 0.0), Vec4::new(0.0, 0.0, -1.0, 0.0), Vec4::new(0.0, -1.0, 0.0, 0.0), Vec4::new(0.5, 9.0 / 16.0, 1.5, 1.0));
    // Heads sit on a humanoid's neck, 1.5 blocks up. On a wall they hang a quarter block higher
    // and against the block behind.
    let floor = Mat4::from_translation(Vec3::new(0.0, -1.5, 0.0));
    let wall = Mat4::from_translation(Vec3::new(0.0, -1.25, 0.25));
    let mut out = Vec::new();
    let mut add = |id, source: Option<&Geometry>, matrix| out.extend(source.map(|g| (id, bake::bake(g).fixed(matrix))));
    add(CHEST, built_in.get(CHEST), Mat4::IDENTITY);
    add(DOUBLE_CHEST, built_in.get(DOUBLE_CHEST), Mat4::IDENTITY);
    add(SIGN, built_in.get(SIGN), sign);
    add(WALL_SIGN, built_in.get(WALL_SIGN), sign);
    // The old layout states no texture size and beds are not 64×32.
    let pack_bed = pack.get("geometry.bed").map(|g| Geometry { texture_size: BED_TEXTURE, ..g.clone() });
    add(BED, pack_bed.as_ref(), bed);
    for (ids, source) in [(MOB_HEAD, "geometry.mob_head"), (PLAYER_HEAD, "geometry.player_head"), (DRAGON_HEAD, "geometry.dragon_head")] {
        add(ids[0], pack.get(source), floor);
        add(ids[1], pack.get(source), wall);
    }
    out
}

/// Chest and sign images, which no entity definition names: paths without extension.
pub(super) fn textures(root: &Path) -> Vec<String> {
    let stems = |dir: &str| {
        let files = std::fs::read_dir(root.join(dir)).into_iter().flatten().flatten();
        let dir = dir.to_owned();
        files.filter_map(move |f| Some(format!("{dir}/{}", f.path().file_name()?.to_str()?.strip_suffix(".png")?)))
    };
    let signs = stems("textures/entity").filter(|path| path.rsplit('/').next().is_some_and(|name| name.contains("sign") && !name.contains("hanging")));
    stems("textures/entity/chest").chain(signs).collect()
}
