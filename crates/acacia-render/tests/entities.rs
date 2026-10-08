//! Entity models from the vanilla pack. Needs assets/vanilla (tools/fetch-vanilla-pack.sh); tests
//! skip when it is missing.

use std::path::PathBuf;
use std::sync::Arc;

use acacia_render::entity::{EntityModels, Layer, NO_TEXTURE, Pose, Value};
use glam::Vec3;

fn models() -> Option<EntityModels> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/vanilla");
    if !dir.join("render_controllers").is_dir() {
        eprintln!("skipped: {} has no render controllers (run tools/fetch-vanilla-pack.sh)", dir.display());
        return None;
    }
    Some(EntityModels::load(&dir))
}

/// The layers of `kind` in the state given as (query, value) pairs.
fn look(models: &EntityModels, kind: &str, state: &[(&str, f32)]) -> (Arc<[Layer]>, f32) {
    let query = |name: &str| Value::Num(state.iter().find(|(n, _)| *n == name).map_or(0.0, |(_, v)| *v));
    models.appearance(kind, &query).unwrap_or_else(|| panic!("{kind}: no appearance"))
}

fn bounds(models: &EntityModels, model: u32) -> (Vec3, Vec3) {
    let points = models.models()[model as usize].mesh.vertices.iter().map(|v| Vec3::from(v.position));
    points.fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)))
}

fn texture(models: &EntityModels, layer: &Layer, slot: usize) -> String {
    models.textures()[layer.textures[slot] as usize].file_stem().unwrap().to_string_lossy().into_owned()
}

/// (kind, height in blocks, longest horizontal side in blocks), from the vanilla hitboxes and looks.
const MOBS: [(&str, f32, f32); 8] = [
    ("minecraft:cow", 1.5, 1.6),
    ("minecraft:pig", 1.0, 1.4),
    ("minecraft:sheep", 1.4, 1.5),
    ("minecraft:chicken", 0.8, 0.7),
    ("minecraft:creeper", 1.7, 0.7),
    ("minecraft:zombie", 2.0, 1.0),
    ("minecraft:villager_v2", 2.0, 0.8),
    ("minecraft:wolf", 0.9, 1.5),
];

#[test]
fn common_mobs_stand_on_the_ground_at_their_size() {
    let Some(models) = models() else { return };
    let mut wrong = Vec::new();
    for (kind, height, length) in MOBS {
        let (layers, _) = look(&models, kind, &[]);
        let (lo, hi) = bounds(&models, layers[0].model);
        let size = hi - lo;
        let fits = (size.y - height).abs() < 0.35 && (size.x.max(size.z) - length).abs() < 0.4 && lo.y.abs() < 0.1;
        if !fits || layers[0].textures[0] == NO_TEXTURE {
            wrong.push(format!("{kind}: {lo:.2}..{hi:.2}, {:?}", layers[0]));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn players_have_the_three_skin_layouts() {
    let Some(models) = models() else { return };
    let skin = |width, height| acacia_render::entity::Skin { width, height, rgba: Vec::new(), mesh: None };
    let wide = models.player(Some((&skin(64, 64), false))).unwrap();
    let slim = models.player(Some((&skin(64, 64), true))).unwrap();
    let legacy = models.player(Some((&skin(64, 32), true))).unwrap();
    assert!(wide != slim && slim != legacy && wide != legacy);
    assert_eq!(models.player(None), Some(wide.clone()));
    assert_eq!(texture(&models, &wide[0], 0), "steve", "the default skin");
    for layers in [wide, slim, legacy] {
        let (lo, hi) = bounds(&models, layers[0].model);
        assert!((hi.y - 2.0).abs() < 0.1 && lo.y.abs() < 0.1 && (hi.x - lo.x - 1.0).abs() < 0.1, "{layers:?}: {lo}..{hi}");
        let mesh = &models.models()[layers[0].model as usize].mesh;
        let head = mesh.bones.iter().position(|b| b == "head").unwrap();
        assert_eq!(mesh.joints[head].pivot, Vec3::new(0.0, 1.5, 0.0));
    }
}

#[test]
fn render_controllers_follow_the_entity_state() {
    let Some(models) = models() else { return };

    // Skin, then biome clothes under the profession's, then the trade level badge.
    let (librarian, _) = look(&models, "minecraft:villager_v2", &[("variant", 5.0), ("mark_variant", 1.0), ("trade_tier", 2.0)]);
    assert_eq!(librarian.len(), 3, "{librarian:?}");
    assert_eq!(texture(&models, &librarian[0], 0), "villager");
    assert_eq!((texture(&models, &librarian[1], 0), texture(&models, &librarian[1], 1)), ("biome_desert".into(), "librarian".into()));
    assert_eq!(texture(&models, &librarian[2], 0), "level_gold");
    let (unskilled, _) = look(&models, "minecraft:villager_v2", &[]);
    assert_eq!(unskilled.len(), 2, "no badge without a profession: {unskilled:?}");

    let (woolly, adult_scale) = look(&models, "minecraft:sheep", &[("color", 14.0)]);
    let (sheared, _) = look(&models, "minecraft:sheep", &[("is_sheared", 1.0)]);
    let (lamb, lamb_scale) = look(&models, "minecraft:sheep", &[("is_baby", 1.0)]);
    assert!(woolly[0].model != sheared[0].model && lamb[0].model != woolly[0].model);
    let [r, g, b] = woolly[0].tint.expect("wool takes the dye");
    assert!(r > 0.3 && g < 0.1 && b < 0.1, "red wool: {r} {g} {b}");
    assert!(lamb_scale > adult_scale, "the script undoes the half scale the server sends for babies");

    let cat = |variant| texture(&models, &look(&models, "minecraft:cat", &[("variant", variant)]).0[0], 0);
    assert!(cat(0.0) != cat(1.0) && cat(1.0) != cat(3.0));

    // The slime's translucent shell is left out, the body stays.
    assert_eq!(look(&models, "minecraft:slime", &[]).0.len(), 1);
    // Kinds whose controllers are missing or all overlays still draw their default.
    for kind in ["minecraft:iron_golem", "minecraft:ender_dragon"] {
        assert_eq!(look(&models, kind, &[]).0.len(), 1, "{kind}");
    }
}

#[test]
fn block_models_fill_their_blocks() {
    use acacia_render::blocks::model::{BED, CHEST, DOUBLE_CHEST, MOB_HEAD, SIGN, WALL_SIGN};
    let Some(models) = models() else { return };
    // Model space before the z mirror: the front is at -z, the origin at the block's bottom centre.
    let extent = |geometry: &str, texture: &str| {
        let layers = models.block_layers(geometry, &format!("textures/entity/{texture}")).unwrap_or_else(|| panic!("{geometry} in {texture}"));
        let (lo, hi) = bounds(&models, layers[0].model);
        (lo.to_array().map(|v| (v * 16.0).round()), hi.to_array().map(|v| (v * 16.0).round()))
    };
    assert_eq!(extent(CHEST, "chest/normal"), ([-7.0, 0.0, -8.0], [7.0, 14.0, 7.0]));
    assert_eq!(extent(DOUBLE_CHEST, "chest/double_normal"), ([-15.0, 0.0, -8.0], [15.0, 14.0, 7.0]));
    // The bed lies on its legs, its head end at the front of the head block.
    assert_eq!(extent(BED, "bed/red"), ([-8.0, 0.0, -8.0], [8.0, 9.0, 24.0]));
    assert_eq!(extent(SIGN, "sign_spruce"), ([-8.0, 0.0, -1.0], [8.0, 17.0, 1.0]));
    assert_eq!(extent(WALL_SIGN, "cherry_sign"), ([-8.0, 4.0, 6.0], [8.0, 12.0, 8.0]));
    assert_eq!(extent(MOB_HEAD[0], "skulls/creeper"), ([-4.0, 0.0, -4.0], [4.0, 8.0, 4.0]));
    assert_eq!(extent(MOB_HEAD[1], "skulls/creeper"), ([-4.0, 4.0, 0.0], [4.0, 12.0, 8.0]));
    assert!(models.block_layers(CHEST, "textures/entity/chest/missing").is_none());
}

#[test]
fn animations_swing_legs_and_turn_heads() {
    let Some(models) = models() else { return };
    let pose = |kind: &str, state: &[(&str, f32)]| {
        let query = |name: &str| Value::Num(state.iter().find(|(n, _)| *n == name).map_or(0.0, |(_, v)| *v));
        let model = look(&models, kind, &[]).0[0].model;
        let pose = models.pose(kind, model, &query);
        (models.models()[model as usize].mesh.skin(&pose), pose)
    };
    let turn = |pose: &acacia_render::entity::Pose, bone: &str| pose.get(bone).unwrap_or_else(|| panic!("{bone} is not posed")).rotation;

    // Standing still, looking ahead: every bone stays in its rest pose.
    let (skin, _) = pose("minecraft:cow", &[]);
    assert!(skin.iter().all(|m| m.abs_diff_eq(glam::Mat4::IDENTITY, 1e-4)), "{skin:?}");

    // A full stride (cos = 1) at speed 0.5: opposite legs 40° apart from rest, head on its target.
    let walking = [("modified_move_speed", 0.5), ("target_x_rotation", 10.0), ("target_y_rotation", -30.0)];
    let (skin, cow) = pose("minecraft:cow", &walking);
    assert_eq!((turn(&cow, "leg0"), turn(&cow, "leg1")), ([40.0, 0.0, 0.0], [-40.0, 0.0, 0.0]));
    assert_eq!(turn(&cow, "head"), [10.0, -30.0, 0.0]);
    assert!(!skin.iter().all(|m| m.abs_diff_eq(glam::Mat4::IDENTITY, 1e-4)));

    // Players go through nested controllers and the script variable `tcos0`.
    let (_, player) = pose("minecraft:player", &walking);
    assert!((turn(&player, "rightarm")[0] + 0.5 * 57.3).abs() < 0.01, "{:?}", turn(&player, "rightarm"));
    assert_eq!(turn(&player, "head"), [10.0, -30.0, 0.0]);
}

#[test]
fn humanoids_hold_items_in_their_right_hand() {
    let Some(models) = models() else { return };
    for kind in ["minecraft:skeleton", "minecraft:zombie", "minecraft:vindicator"] {
        let (layers, _) = look(&models, kind, &[]);
        let mesh = &models.models()[layers[0].model as usize].mesh;
        let hand = mesh.right_hand(&Pose::default()).unwrap_or_else(|| panic!("{kind} has no rightitem bone"));
        let at = hand.transform_point3(Vec3::ZERO);
        // Model space has the entity's right at -x; a hanging hand is a little over half a block up.
        assert!(at.x < -0.2 && (0.4..1.2).contains(&at.y), "{kind}'s hand at {at}");
    }
}
