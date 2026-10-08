//! Item icons from the vanilla pack. Needs assets/vanilla with its items
//! (tools/fetch-vanilla-pack.sh); tests skip when it is missing.

use std::path::PathBuf;

use acacia_render::item::ItemIcons;

fn icons() -> Option<ItemIcons> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/vanilla");
    if !dir.join("textures/item_texture.json").is_file() {
        eprintln!("skipped: {} has no item textures (run tools/fetch-vanilla-pack.sh)", dir.display());
        return None;
    }
    Some(ItemIcons::load(&dir))
}

#[test]
fn renamed_items_find_their_icons() {
    let Some(icons) = icons() else { return };
    let cases = [
        ("minecraft:apple", 0, "textures/items/apple"),
        ("minecraft:cooked_beef", 0, "textures/items/beef_cooked"),
        ("minecraft:golden_sword", 0, "textures/items/gold_sword"),
        ("minecraft:wooden_pickaxe", 0, "textures/items/wood_pickaxe"),
        ("minecraft:cave_spider_spawn_egg", 0, "textures/items/spawn_egg_cave_spider"),
        ("minecraft:music_disc_cat", 0, "textures/items/record_cat"),
        ("minecraft:water_bucket", 0, "textures/items/bucket_water"),
        ("minecraft:light_gray_dye", 0, "textures/items/dye_powder_silver"),
        ("minecraft:dark_oak_boat", 0, "textures/items/boat_darkoak"),
        ("minecraft:bone_meal", 0, "textures/items/dye_powder_white"),
    ];
    for (name, aux, want) in cases {
        let got = icons.path(name, aux).unwrap_or_else(|| panic!("{name}: no icon"));
        assert!(got.ends_with(want.rsplit('/').next().unwrap()), "{name}: {got}");
    }
}

#[test]
fn blocks_without_icons_find_none() {
    let Some(icons) = icons() else { return };
    assert_eq!(icons.path("minecraft:stone", 0), None);
}
