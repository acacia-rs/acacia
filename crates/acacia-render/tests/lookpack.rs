//! A look pack baked from the vanilla pack, saved and loaded back. Needs assets/vanilla
//! (tools/fetch-vanilla-pack.sh); the test skips when it is missing.

use std::path::PathBuf;

use acacia_render::assets::Pack;
use acacia_render::assets::flipbook::Atlas;
use acacia_render::biome::{BiomeColors, BiomeDef};
use acacia_render::blocks::BlockTable;
use acacia_render::entity::EntityModels;
use acacia_render::{Look, LookPack};
use acacia_world::BlockRegistry;

#[test]
fn a_baked_pack_draws_what_the_bedrock_pack_does() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/vanilla");
    if !source.is_dir() {
        eprintln!("skipped: {} missing (run tools/fetch-vanilla-pack.sh)", source.display());
        return;
    }
    let pack = Pack::load(&source).unwrap();
    let registry = BlockRegistry::vanilla();
    let (direct, atlas, _) = BlockTable::build(registry, &pack);
    let dir = std::env::temp_dir().join(format!("acacia-lookpack-{}-baked", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    LookPack::bake_bedrock(&pack, Look::BEDROCK).0.save(&dir).unwrap();
    let loaded = LookPack::load(&dir).unwrap();

    let table = loaded.block_table(registry);
    for id in 0..registry.len() as u32 {
        assert_eq!(table.get(id), direct.get(id), "{:?}", registry.get(id));
    }
    let texels = |a: &Atlas| a.layers.iter().map(|t| t.rgba.to_vec()).collect::<Vec<_>>();
    assert_eq!(texels(&loaded.atlas), texels(&atlas));
    assert_eq!(loaded.atlas.animations.len(), atlas.animations.len());
    for (a, b) in loaded.atlas.animations.iter().zip(&atlas.animations) {
        assert_eq!((a.layer, a.at(7).rgba), (b.layer, b.at(7).rgba));
    }

    // The loose files came along: entities and biome colours load from the pack alone.
    assert_eq!(loaded.files(), dir);
    let (ours, theirs) = (EntityModels::load(loaded.files()), EntityModels::load(&source));
    assert!(!ours.models().is_empty());
    assert_eq!((ours.models().len(), ours.textures().len()), (theirs.models().len(), theirs.textures().len()));
    let swamp = [BiomeDef { id: 6, name: "swampland".into(), temperature: 0.8, downfall: 0.9 }];
    assert_eq!(BiomeColors::build(&swamp, loaded.files()).get(6), BiomeColors::build(&swamp, &source).get(6));
    std::fs::remove_dir_all(&dir).unwrap();
}
