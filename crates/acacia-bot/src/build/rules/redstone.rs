//! Blocks whose final state depends on neighbours, order, support or growth: rails, redstone dust, water and lava
//! sources, plants on their supports, farmland. Measured on BDS 1.26.52.3 with `place_probe SET=redstone`
//! (2026-10-28); see docs/builder/redstone.md.

use acacia_physics::BlockPos;

use super::rails::{self, World};
use crate::build::builder::Cell;
use crate::build::structure::Block;
use crate::build::survey::Need;
use crate::interact::Face;
use crate::pathfind::process::{Act, BlockView, Look};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
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
    "wither_rose", "torchflower", "short_grass", "fern", "deadbush", "brown_mushroom", "red_mushroom", "bamboo_sapling",
];
const CROPS: &[(&str, &str)] = &[("wheat", "wheat_seeds"), ("carrots", "carrot"), ("potatoes", "potato"), ("beetroot", "beetroot_seeds")];
const HOES: &[&str] = &["netherite_hoe", "diamond_hoe", "iron_hoe", "golden_hoe", "stone_hoe", "wooden_hoe"];
/// Vine bits by attached side: south 1, west 2, north 4, east 8 (probe: a vine on a block's west face is 8).
const VINE_BITS: [(Face, u8); 4] = [(Face::North, 1), (Face::East, 2), (Face::South, 4), (Face::West, 8)];

fn short(name: &str) -> &str {
    name.trim_start_matches("minecraft:")
}

fn kind(name: &str) -> Option<Kind> {
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
/// not reachable for the probe's click.
pub fn denied(name: &str) -> bool {
    short(name) == "bamboo"
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

/// An item the target also needs besides [`item`]: the water for kelp and seagrass, a hoe for farmland (any
/// hoe held does; a station is asked for a wooden one).
pub fn extra_item(target: &Block) -> Option<String> {
    match kind(&target.name)? {
        Kind::Aquatic => Some("minecraft:water_bucket".into()),
        Kind::Farmland => Some("minecraft:wooden_hoe".into()),
        _ => None,
    }
}

fn liquid_of(name: &str) -> Option<&'static str> {
    let n = short(name);
    n.ends_with("water").then_some("water").or_else(|| n.ends_with("lava").then_some("lava"))
}

fn source(w: &BlockView) -> bool {
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

/// `Some` where the click is decided here: the Up face for rails and plants, the vine's side, any face for a
/// liquid.
pub fn gives(target: &Block, face: Face) -> Option<bool> {
    Some(match kind(&target.name)? {
        Kind::Liquid => true,
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

/// The act for a click into `pos` with `item`: a use for hoes and buckets (done once the cell shows the block),
/// else a placement.
pub fn act(item: String, against: BlockPos, face: Face, click: [f32; 3], pos: BlockPos) -> Act {
    let n = short(&item);
    match () {
        _ if n.ends_with("_hoe") => Act::UseOn { item, against: pos, face: Face::Up, cell: pos, expect: "farmland".into() },
        _ if n.ends_with("_bucket") => Act::UseOn { expect: n.trim_end_matches("_bucket").into(), item, against, face, cell: pos },
        _ => Act::Place { item, against, face, click },
    }
}

/// A `rail_direction` value turned with the structure (`transform`).
pub fn turned_rail<'a>(v: &'a str, turns: u8, mirror: bool) -> &'a str {
    const SHAPES: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
    v.parse::<u8>().ok().filter(|s| *s <= 9).map_or(v, |s| SHAPES[usize::from(rails::turned(s, turns % 4, mirror))])
}

fn below(p: BlockPos) -> BlockPos {
    [p[0], p[1] - 1, p[2]]
}

/// The cell shows its target (outside the footprint: something solid).
fn done(p: BlockPos, look: &dyn Look, targets: &dyn Fn(BlockPos) -> Option<Block>) -> bool {
    match targets(p) {
        Some(t) => look.block(p).is_some_and(|w| super::matches(&t, &w)),
        None => look.solid(p) == Some(true),
    }
}

fn sides(p: BlockPos) -> [BlockPos; 4] {
    [[p[0], p[1], p[2] - 1], [p[0], p[1], p[2] + 1], [p[0] - 1, p[1], p[2]], [p[0] + 1, p[1], p[2]]]
}

/// The cells the target needs first are built: its support (a vine's side), water next to sugar cane's, and for a
/// rail an order that keeps every rail on the way to its shape ([`rail_ready`]).
pub fn ready(target: &Block, pos: BlockPos, look: &dyn Look, targets: &dyn Fn(BlockPos) -> Option<Block>) -> bool {
    let Some(k) = kind(&target.name) else { return true };
    let wet = |p: BlockPos| targets(p).is_some_and(|t| kind(&t.name) == Some(Kind::Liquid));
    match k {
        Kind::Liquid | Kind::Farmland => true,
        Kind::Vine => VINE_BITS.iter().find(|(_, b)| target.prop("vine_direction_bits") == Some(b.to_string().as_str())).is_some_and(|(f, _)| {
            let o = f.offset();
            done([pos[0] - o[0], pos[1] - o[1], pos[2] - o[2]], look, targets)
        }),
        Kind::Plant if short(&target.name) == "reeds" => done(below(pos), look, targets) && sides(below(pos)).into_iter().filter(|p| wet(*p)).all(|p| done(p, look, targets)),
        Kind::Rail => done(below(pos), look, targets) && rail_ready(target, pos, look, targets),
        _ => done(below(pos), look, targets),
    }
}

/// Liquids and what lives in them go last: water flows over cells still to build and washes dust, rails and
/// plants away. Drops them from `ready` while anything dry is ready.
pub fn dry_first(ready: &mut Vec<(usize, String)>, target: impl Fn(usize) -> Block) {
    let wet = |i: usize| matches!(kind(&target(i).name), Some(Kind::Liquid | Kind::Aquatic));
    if ready.iter().any(|(i, _)| !wet(*i)) {
        ready.retain(|(i, _)| !wet(*i));
    }
}

fn rail_of(look: &dyn Look, p: BlockPos) -> Option<rails::Rail> {
    let v = look.block(p).filter(|v| rails::is_rail(v.name))?;
    Some((rails::straight(v.name), v.prop("rail_direction")?.parse().ok()?))
}

fn target_shape(targets: &dyn Fn(BlockPos) -> Option<Block>, p: BlockPos) -> Option<u8> {
    targets(p).filter(|t| rails::is_rail(&t.name))?.prop("rail_direction")?.parse().ok()
}

/// Every rail the placement shapes joins only rails its target leads to.
fn keeps_shapes(w: &mut World, pos: BlockPos, straight: bool, targets: &dyn Fn(BlockPos) -> Option<Block>) -> bool {
    w.place(pos, straight).into_iter().all(|(p, _)| {
        let Some(t) = target_shape(targets, p) else { return true };
        let to = rails::ends(p, t);
        w.joined(p).iter().all(|j| to.iter().any(|e| e[0] == j[0] && e[2] == j[2]))
    })
}

/// Rail cells around `pos` (same height, one up, one down).
fn around(pos: BlockPos) -> impl Iterator<Item = BlockPos> {
    sides(pos).into_iter().flat_map(|s| [-1, 0, 1].map(|dy| [s[0], s[1] + dy, s[2]]))
}

/// Placing the rail now keeps every rail on the way to its shape, and so does placing any open rail beside it
/// next (one step of lookahead: a junction's middle waits for both ends of its own line).
fn rail_ready(target: &Block, pos: BlockPos, look: &dyn Look, targets: &dyn Fn(BlockPos) -> Option<Block>) -> bool {
    let base = |p: BlockPos| rail_of(look, p);
    let mut w = World::new(&base);
    if !keeps_shapes(&mut w, pos, rails::straight(&target.name), targets) {
        return false;
    }
    around(pos).all(|q| match targets(q).filter(|t| rails::is_rail(&t.name)) {
        Some(t) if w.get(q).is_none() => keeps_shapes(&mut w.clone(), q, rails::straight(&t.name), targets),
        _ => true,
    })
}

/// The liquids the structure has sources of.
pub fn poured<'a>(blocks: impl Iterator<Item = &'a Block>) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = blocks.filter(|b| kind(&b.name) == Some(Kind::Liquid) && b.prop("liquid_depth").is_none_or(|d| d == "0")).filter_map(|b| liquid_of(&b.name)).collect();
    out.dedup();
    out
}

/// Liquid flowing from the structure's own sources where air belongs: left alone, and not a mismatch.
pub fn spilled(poured: &[&str], target: &Block, w: &BlockView) -> bool {
    target.is_air() && w.liquid && !source(w) && liquid_of(w.name).is_some_and(|l| poured.contains(&l))
}

/// Corrects the survey for this family: a rail on its way to its shape (joined only where its target leads, a
/// rail at one of its target's ends not at its own shape yet) is not wrong yet; liquid flowing from the job's own sources into cells
/// where air belongs is left alone (draining it would fight the sources).
pub fn settle(cells: &[Cell], look: &dyn Look, targets: &dyn Fn(BlockPos) -> Option<Block>, needs: &mut [Need]) {
    let base = |p: BlockPos| rail_of(look, p);
    let w = World::new(&base);
    let poured = poured(cells.iter().map(|c| &c.target));
    for (c, n) in cells.iter().zip(needs.iter_mut()) {
        let Some(seen) = look.block(c.pos) else { continue };
        match *n {
            Need::Drain if spilled(&poured, &c.target, &seen) => *n = Need::Done,
            Need::Break if seen.name == c.target.name && rails::is_rail(seen.name) => {
                let Some(t) = target_shape(targets, c.pos) else { continue };
                let to = rails::ends(c.pos, t);
                let joined = w.joined(c.pos);
                let on_way = joined.iter().all(|j| to.iter().any(|e| e[0] == j[0] && e[2] == j[2]));
                // Not yet at its own shape either: a neighbour just placed shows the client's prediction, and
                // the server's turn of this rail comes with its echo (live: broken a tick before it turned).
                let to_come = to.iter().any(|e| [*e, [e[0], e[1] + 1, e[2]], below(*e)].iter().any(|q| target_shape(targets, *q).is_some() && !done(*q, look, targets)));
                if on_way && to_come {
                    *n = Need::Done;
                }
            }
            _ => {}
        }
    }
}
