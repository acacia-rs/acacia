//! Entity models from the vanilla pack. Needs assets/vanilla (tools/fetch-vanilla-pack.sh); tests
//! skip when it is missing.

use std::path::PathBuf;
use std::sync::Arc;

use acacia_render::assets::Pack;
use acacia_render::entity::{EntityModels, Layer, NO_TEXTURE, Value};
use glam::Vec3;

fn models() -> Option<EntityModels> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/vanilla");
    if !dir.join("render_controllers").is_dir() {
        eprintln!("skipped: {} has no render controllers (run tools/fetch-vanilla-pack.sh)", dir.display());
        return None;
    }
    Some(EntityModels::load(&Pack::load(&dir).unwrap()))
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
        assert_eq!(models.models()[layers[0].model as usize].mesh.head_pivot, Vec3::new(0.0, 1.5, 0.0));
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
