//! Where rain falls: BDS `LevelChunk::getTopRainBlockPos` (docs/research/riptide-rain.md).

use crate::{BlockAccess, BlockRegistry, ChunkView};

/// Blocks rain falls through besides air: sapling's oracle-fitted plant set (`freeze.rs::stops_rain`), crops
/// (cactus, melon, pumpkin, mangrove roots and leaves) excluded.
const RAIN_THROUGH: &[&str] = &[
    "short_grass", "fern", "tall_grass", "large_fern", "sunflower", "lilac", "rose_bush", "peony", "bush", "red_shrub",
    "firefly_bush", "short_dry_grass", "tall_dry_grass", "deadbush", "reeds", "brown_mushroom", "red_mushroom",
    "shelf_mushroom", "waterlily", "sweet_berry_bush", "poppy", "dandelion", "blue_orchid", "azure_bluet", "red_tulip",
    "orange_tulip", "white_tulip", "pink_tulip", "oxeye_daisy", "cornflower", "lily_of_the_valley", "allium", "vine",
    "moss_carpet", "leaf_litter", "pink_petals", "wildflowers", "mangrove_propagule", "cocoa", "azalea",
    "flowering_azalea", "spore_blossom", "hanging_roots", "pale_moss_carpet", "pale_hanging_moss", "closed_eyeblossom",
    "open_eyeblossom", "cave_vines", "cave_vines_head_with_berries", "cave_vines_body_with_berries", "big_dripleaf",
    "small_dripleaf_block", "seagrass", "bubble_column", "tube_coral", "brain_coral", "bubble_coral", "fire_coral",
    "horn_coral", "tube_coral_fan", "brain_coral_fan", "bubble_coral_fan", "fire_coral_fan", "horn_coral_fan",
    "tube_coral_wall_fan", "brain_coral_wall_fan", "bubble_coral_wall_fan", "fire_coral_wall_fan", "horn_coral_wall_fan",
    "sea_pickle", "kelp", "pointed_dripstone", "sulfur_spike", "sculk_sensor", "sculk_shrieker", "glow_lichen",
    "sculk_vein", "crimson_roots", "warped_roots", "nether_sprouts", "crimson_fungus", "warped_fungus", "weeping_vines",
    "twisting_vines",
];

fn obstructs(registry: &BlockRegistry, id: u32) -> bool {
    match registry.get(id) {
        Some(s) if s.is_air() => false,
        Some(s) => !RAIN_THROUGH.contains(&s.name.strip_prefix("minecraft:").unwrap_or(s.name)),
        None => true,
    }
}

/// Rain reaches cell `[x, y, z]`: no block in it or above it stops rain. Unloaded cells read as air, as BDS
/// treats a missing chunk. The weather, the biome and its temperature are the caller's.
pub fn rain_reaches(view: &ChunkView, registry: &BlockRegistry, [x, y, z]: [i32; 3]) -> bool {
    let dim = view.world().dimension();
    let top = dim.min_y + dim.height as i32;
    (y.max(dim.min_y)..top).all(|y| !obstructs(registry, view.block(x, y, z)) && !obstructs(registry, view.liquid(x, y, z)))
}
