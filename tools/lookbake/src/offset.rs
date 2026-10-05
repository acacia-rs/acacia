//! Which Java blocks stand off the grid, and how far: `BlockBehaviour.OffsetType` per block.

use acacia_render::blocks::placed::Offset;

/// Shifted sideways only.
pub const SIDEWAYS: &[&str] = &[
    "mangrove_propagule", "tall_seagrass", "dandelion", "golden_dandelion", "torchflower", "poppy", "blue_orchid", "allium",
    "azure_bluet", "red_tulip", "orange_tulip", "white_tulip", "pink_tulip", "oxeye_daisy", "cornflower", "wither_rose",
    "lily_of_the_valley", "sunflower", "lilac", "rose_bush", "peony", "tall_grass", "large_fern", "pitcher_plant", "bamboo_sapling",
    "bamboo", "warped_roots", "nether_sprouts", "crimson_roots", "hanging_roots", "open_eyeblossom", "closed_eyeblossom",
];
/// Sideways, and into the ground.
pub const SUNK: &[&str] = &["short_grass", "fern", "short_dry_grass", "tall_dry_grass", "small_dripleaf"];
/// Sideways by less.
pub const NARROW: &[&str] = &["pointed_dripstone", "sulfur_spike"];

/// Of a Java block, by its name without namespace.
pub fn of(name: &str) -> Option<Offset> {
    let vertical = match name {
        "small_dripleaf" => 0.1,
        _ if SUNK.contains(&name) => 0.2,
        _ => 0.0,
    };
    match () {
        _ if NARROW.contains(&name) => Some(Offset { horizontal: 0.125, vertical }),
        _ if SIDEWAYS.contains(&name) || SUNK.contains(&name) => Some(Offset { horizontal: 0.25, vertical }),
        _ => None,
    }
}

#[test]
fn plants_shift_and_grass_sinks() {
    assert_eq!(of("poppy"), Some(Offset { horizontal: 0.25, vertical: 0.0 }));
    assert_eq!(of("fern"), Some(Offset { horizontal: 0.25, vertical: 0.2 }));
    assert_eq!(of("small_dripleaf"), Some(Offset { horizontal: 0.25, vertical: 0.1 }));
    assert_eq!(of("pointed_dripstone"), Some(Offset { horizontal: 0.125, vertical: 0.0 }));
    assert_eq!(of("stone"), None);
}
