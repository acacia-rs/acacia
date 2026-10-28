//! Blocks whose final state depends on neighbours, order, support or growth: rails, redstone dust, water and lava
//! sources, plants on their supports, farmland. Measured on BDS 1.26.52.3 with `place_probe SET=redstone`
//! (2026-10-28); see docs/builder/redstone.md.

use acacia_physics::BlockPos;
use acacia_world::BlockRegistry;

use super::rails;
use crate::build::structure::Block;
use crate::interact::Face;
use crate::pathfind::process::{Act, BlockView, Look};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    Rail,
    Wire,
    Liquid,
    /// On a support below: saplings, flowers, crops, cactus, sugar cane.
    Plant,
    /// In a water source on a support below: kelp, seagrass.
    Aquatic,
    Vine,
    Farmland,
}

const FLOWERS: &[&str] = &[
    "dandelion", "poppy", "blue_orchid", "allium", "azure_bluet", "red_tulip", "orange_tulip", "white_tulip", "pink_tulip", "oxeye_daisy", "cornflower", "lily_of_the_valley",
    "wither_rose", "torchflower", "short_grass", "fern", "deadbush", "bamboo_sapling",
];
const CROPS: &[(&str, &str)] = &[("wheat", "wheat_seeds"), ("carrots", "carrot"), ("potatoes", "potato"), ("beetroot", "beetroot_seeds")];
const HOES: &[&str] = &["netherite_hoe", "diamond_hoe", "iron_hoe", "golden_hoe", "stone_hoe", "wooden_hoe"];
/// Vine bits by attached side: south 1, west 2, north 4, east 8 (probe: a vine on a block's west face is 8).
pub(super) const VINE_BITS: [(Face, u8); 4] = [(Face::North, 1), (Face::East, 2), (Face::South, 4), (Face::West, 8)];

pub(super) fn short(name: &str) -> &str {
    name.trim_start_matches("minecraft:")
}

pub(super) fn kind(name: &str) -> Option<Kind> {
    let n = short(name);
    Some(match n {
        _ if rails::is_rail(name) => Kind::Rail,
        "redstone_wire" => Kind::Wire,
        "water" | "lava" | "flowing_water" | "flowing_lava" => Kind::Liquid,
        "kelp" | "seagrass" => Kind::Aquatic,
        "vine" => Kind::Vine,
        "farmland" => Kind::Farmland,
        "cactus" | "reeds" => Kind::Plant,
        _ if n.ends_with("_sapling") || FLOWERS.contains(&n) || CROPS.iter().any(|(c, _)| *c == n) => Kind::Plant,
        _ => return None,
    })
}

/// Grown bamboo comes from a sapling (an item placed on the ground is `bamboo_sapling`); the stalk on top was
/// not reachable for the probe's click. Brown and red mushrooms: BDS refuses them in daylight.
pub fn denied(name: &str) -> bool {
    matches!(short(name), "bamboo" | "brown_mushroom" | "red_mushroom")
}

/// A grass_block target under a solid target matches dirt: BDS turns covered grass to dirt as soon as the block
/// above is placed (live 2026-10-28).
pub fn covered_grass(target: &Block, above: Option<&Block>, w: &BlockView) -> bool {
    let r = BlockRegistry::vanilla();
    let solid = |b: &Block| r.find(&b.name, &b.props).and_then(|id| r.get(id)).is_some_and(|s| s.is_solid());
    short(&target.name) == "grass_block" && short(w.name) == "dirt" && above.is_some_and(solid)
}

/// States the server derives or that grow: never compared.
pub fn derived(key: &str) -> bool {
    matches!(key, "redstone_signal" | "rail_data_bit" | "moisturized_amount" | "growth" | "age" | "age_bit" | "kelp_age")
}

pub fn controlled(name: &str, key: &str) -> bool {
    match kind(name) {
        Some(Kind::Rail) => key == "rail_direction",
        Some(Kind::Vine) => key == "vine_direction_bits",
        Some(Kind::Liquid) => key == "liquid_depth",
        Some(Kind::Aquatic) => key == "sea_grass_type",
        _ => false,
    }
}

/// The target can be placed at all: one vine side (a click attaches one), short seagrass, liquid sources (flows
/// are not placed: they are never compared).
pub fn placeable(target: &Block) -> bool {
    match kind(&target.name) {
        Some(Kind::Vine) => target.prop("vine_direction_bits").and_then(|b| b.parse::<u8>().ok()).is_some_and(|b| b.count_ones() == 1),
        Some(Kind::Aquatic) => target.prop("sea_grass_type").is_none_or(|t| t == "default"),
        _ => true,
    }
}

/// The item that places the target, where it differs from the block's name.
pub fn item(target: &Block) -> Option<String> {
    let n = short(&target.name);
    let item = match n {
        "redstone_wire" => "redstone",
        "reeds" => "sugar_cane",
        "bamboo_sapling" => "bamboo",
        "water" | "flowing_water" => "water_bucket",
        "lava" | "flowing_lava" => "lava_bucket",
        "farmland" => "dirt",
        _ => CROPS.iter().find(|(c, _)| *c == n).map(|(_, i)| *i)?,
    };
    Some(format!("minecraft:{item}"))
}

/// An item the target also needs besides [`item`]: the water for kelp and seagrass. Farmland's hoe is a tool
/// the job's kit brings, not stock.
pub fn extra_item(target: &Block) -> Option<String> {
    (kind(&target.name)? == Kind::Aquatic).then(|| "minecraft:water_bucket".into())
}

pub(super) fn liquid_of(name: &str) -> Option<&'static str> {
    let n = short(name);
    n.ends_with("water").then_some("water").or_else(|| n.ends_with("lava").then_some("lava"))
}

pub(super) fn source(w: &BlockView) -> bool {
    w.liquid && w.prop("liquid_depth").is_none_or(|d| d == "0")
}

/// `Some` where the comparison differs from the plain one: a flowing target is never compared, a source wants a
/// source of the same liquid (`water` and `flowing_water` alike).
pub fn matches(target: &Block, w: &BlockView) -> Option<bool> {
    if kind(&target.name) != Some(Kind::Liquid) {
        return None;
    }
    if target.prop("liquid_depth").is_some_and(|d| d != "0") {
        return Some(true);
    }
    Some(liquid_of(w.name) == liquid_of(&target.name) && source(w))
}

/// The cell holds what the next step works on: dirt or grass to till into farmland.
pub fn completes(target: &Block, w: &BlockView) -> bool {
    kind(&target.name) == Some(Kind::Farmland) && matches!(short(w.name), "dirt" | "grass_block")
}

/// Height of a top face clicked although the block is not a full cube.
pub fn top(name: &str) -> Option<f32> {
    match short(name) {
        "farmland" => Some(0.9375),
        "cactus" | "reeds" | "kelp" => Some(1.0),
        _ => None,
    }
}

/// `Some` where the click is decided here: the Up face for rails, plants and liquids, the vine's side. A pour
/// against a side face that grazed the neighbour's top edge landed on that top (live 2026-10-28).
pub fn gives(target: &Block, face: Face) -> Option<bool> {
    Some(match kind(&target.name)? {
        Kind::Farmland => return None,
        Kind::Vine => {
            let bits = target.prop("vine_direction_bits").and_then(|b| b.parse::<u8>().ok());
            VINE_BITS.iter().any(|(f, b)| *f == face && Some(*b) == bits)
        }
        _ => face == Face::Up,
    })
}

/// The item for the cell's next step when it is not [`item`]: a hoe on dirt that becomes farmland, a water
/// bucket where kelp or seagrass goes and no water is yet.
pub fn item_now(target: &Block, world: Option<BlockView>, look: &dyn Look) -> Option<String> {
    let w = world?;
    match kind(&target.name)? {
        Kind::Farmland if completes(target, &w) => {
            let hoes: Vec<String> = HOES.iter().map(|h| format!("minecraft:{h}")).collect();
            Some(hoes.iter().find(|h| look.count(h) > 0).unwrap_or(&hoes[hoes.len() - 1]).clone())
        }
        Kind::Aquatic if !(source(&w) && liquid_of(w.name) == Some("water")) => Some("minecraft:water_bucket".into()),
        _ => None,
    }
}

/// The act for a click into `pos` with `item`: a hoe tills `pos` (the interact tier's use), a bucket pours (done
/// once the cell shows a source), else a placement.
pub fn act(item: String, against: BlockPos, face: Face, click: [f32; 3], pos: BlockPos) -> Act {
    let n = short(&item);
    match () {
        _ if n.ends_with("_hoe") => Act::Use { pos, item: Some(item), face: Face::Up },
        _ if n.ends_with("_bucket") => Act::UseOn { expect: n.trim_end_matches("_bucket").into(), item, against, face, cell: pos },
        _ => Act::Place { item, against, face, click },
    }
}

/// A `rail_direction` value turned with the structure (`transform`).
pub fn turned_rail(v: &str, turns: u8, mirror: bool) -> &str {
    const SHAPES: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
    v.parse::<u8>().ok().filter(|s| *s <= 9).map_or(v, |s| SHAPES[usize::from(rails::turned(s, turns % 4, mirror))])
}
