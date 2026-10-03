//! Entity models from the vanilla pack. Needs assets/vanilla (tools/fetch-vanilla-pack.sh); tests
//! skip when it is missing.

use std::path::PathBuf;

use acacia_render::assets::Pack;
use acacia_render::entity::EntityModels;
use glam::Vec3;

fn models() -> Option<EntityModels> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/vanilla");
    if !dir.join("entity").is_dir() {
        eprintln!("skipped: {} has no entities (run tools/fetch-vanilla-pack.sh)", dir.display());
        return None;
    }
    Some(EntityModels::load(&Pack::load(&dir).unwrap()))
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
        let Some((id, _)) = models.lookup(kind, false) else {
            wrong.push(format!("{kind}: no model"));
            continue;
        };
        let model = &models.models()[id as usize];
        let points = model.mesh.vertices.iter().map(|v| Vec3::from(v.position));
        let (lo, hi) = points.fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)));
        let size = hi - lo;
        let fits = (size.y - height).abs() < 0.35 && (size.x.max(size.z) - length).abs() < 0.4 && lo.y.abs() < 0.1;
        if !fits || model.textures.is_empty() {
            wrong.push(format!("{kind}: {} vertices, {lo:.2}..{hi:.2}, textures {:?}", model.mesh.vertices.len(), model.textures));
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
    assert_eq!(models.player(None), Some(wide));
    assert_eq!(models.models()[wide as usize].textures.len(), 1, "steve is the default");
    let (villager, _) = models.lookup("minecraft:villager_v2", false).unwrap();
    assert_eq!(models.models()[villager as usize].textures.len(), 3, "skin, biome clothes, profession");
    for id in [wide, slim, legacy] {
        let mesh = &models.models()[id as usize].mesh;
        let points = mesh.vertices.iter().map(|v| Vec3::from(v.position));
        let (lo, hi) = points.fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)));
        assert!((hi.y - 2.0).abs() < 0.1 && lo.y.abs() < 0.1 && (hi.x - lo.x - 1.0).abs() < 0.1, "{id}: {lo}..{hi}, {} vertices", mesh.vertices.len());
        assert_eq!(mesh.head_pivot, Vec3::new(0.0, 1.5, 0.0));
    }
}
