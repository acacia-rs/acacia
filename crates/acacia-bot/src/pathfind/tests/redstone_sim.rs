//! The simulations' server for the redstone tier (`place_probe SET=redstone`, 2026-10-28): what plants, dust and
//! rails need under them, how rails join, hoes and buckets. Written from the probe's table, apart from
//! `build/rules/redstone.rs` so the two can disagree. Liquids do not flow here.

use std::collections::HashMap;

use acacia_physics::BlockPos;

use super::build_sim::view;
use super::grid::Grid;
use crate::interact::Face;
use crate::ActionError;

const SOIL: &[&str] = &["grass_block", "dirt", "podzol", "coarse_dirt", "farmland"];

fn name_at(g: &Grid, p: BlockPos) -> String {
    view(g, p).0.trim_start_matches("minecraft:").to_string()
}

fn solid(g: &Grid, p: BlockPos) -> bool {
    let n = name_at(g, p);
    n != "air" && !n.contains("water") && !n.contains("lava")
}

fn off(p: BlockPos, d: [i32; 3]) -> BlockPos {
    [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
}

const SIDES: [[i32; 3]; 4] = [[0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];

/// The block an item of this tier places into `pos` by a click on `face`; `None`: not this tier's item;
/// `Some(None)`: refused.
pub fn place(g: &Grid, item: &str, face: Face, pos: BlockPos) -> Option<Option<(String, String)>> {
    let n = item.trim_start_matches("minecraft:");
    let below = name_at(g, off(pos, [0, -1, 0]));
    let wet = |p: BlockPos| name_at(g, p).contains("water");
    let up = face == Face::Up;
    let ok_if = |ok: bool, name: &str, props: &str| Some(ok.then(|| (name.to_string(), props.to_string())));
    match n {
        "redstone" => ok_if(face != Face::Down && solid(g, off(pos, [0, -1, 0])), "redstone_wire", "redstone_signal=0"),
        "rail" | "golden_rail" | "detector_rail" | "activator_rail" => ok_if(up && solid(g, off(pos, [0, -1, 0])), n, "rail_direction=0"),
        "brown_mushroom" | "red_mushroom" => Some(None),
        "bamboo" => ok_if(up && SOIL.contains(&below.as_str()), "bamboo_sapling", "age_bit=0"),
        "sugar_cane" => ok_if(up && (below == "reeds" || (["sand", "dirt", "grass_block"].contains(&below.as_str()) && SIDES.iter().any(|d| wet(off(off(pos, [0, -1, 0]), *d))))), "reeds", "age=0"),
        "cactus" => ok_if(up && ["sand", "red_sand", "cactus"].contains(&below.as_str()) && !SIDES.iter().any(|d| solid(g, off(pos, *d))), "cactus", "age=0"),
        "wheat_seeds" | "carrot" | "potato" | "beetroot_seeds" => {
            let crop = match n {
                "wheat_seeds" => "wheat",
                "carrot" => "carrots",
                "potato" => "potatoes",
                _ => "beetroot",
            };
            ok_if(up && below == "farmland", crop, "growth=0")
        }
        "kelp" => ok_if(up && solid(g, off(pos, [0, -1, 0])) && (below != "kelp" || wet(pos)), "kelp", "kelp_age=0"),
        "seagrass" => ok_if(up && wet(pos) && solid(g, off(pos, [0, -1, 0])), "seagrass", "sea_grass_type=default"),
        "vine" => {
            let bits = match face {
                Face::North => 1,
                Face::East => 2,
                Face::South => 4,
                Face::West => 8,
                _ => return Some(None),
            };
            ok_if(true, "vine", &format!("vine_direction_bits={bits}"))
        }
        _ if n.ends_with("_sapling") || ["poppy", "dandelion", "cornflower", "short_grass"].contains(&n) => ok_if(up && SOIL.contains(&below.as_str()), n, ""),
        _ => None,
    }
}

/// A hoe on dirt or grass with air above makes farmland; a full bucket fills the cell beside the face and comes
/// back empty.
pub fn use_on(g: &Grid, inv: &mut HashMap<String, u32>, item: &str, against: BlockPos, face: Face) -> Result<(), ActionError> {
    let n = item.trim_start_matches("minecraft:");
    let refused = || Err(ActionError::Rejected(format!("{item} on {against:?}")));
    let have = inv.get_mut(item).filter(|c| **c > 0).ok_or_else(|| ActionError::NotPossible(format!("no {item}")))?;
    if n.ends_with("_hoe") {
        if !["dirt", "grass_block"].contains(&name_at(g, against).as_str()) || name_at(g, off(against, [0, 1, 0])) != "air" {
            return refused();
        }
        g.set_state(against, "farmland", "moisturized_amount=0");
        return Ok(());
    }
    let liquid = n.strip_suffix("_bucket").filter(|l| ["water", "lava"].contains(l)).ok_or_else(|| ActionError::NotPossible(format!("{item} is no use here")))?;
    let cell = face.adjacent(against);
    if !solid(g, against) || solid(g, cell) {
        return refused();
    }
    *have -= 1;
    *inv.entry("minecraft:bucket".into()).or_default() += 1;
    g.set_state(cell, liquid, "liquid_depth=0");
    Ok(())
}

fn rail(g: &Grid, p: BlockPos) -> Option<(bool, u8)> {
    let (name, props) = view(g, p);
    let n = name.trim_start_matches("minecraft:");
    if !n.ends_with("rail") {
        return None;
    }
    let shape = props.split(',').find_map(|kv| kv.strip_prefix("rail_direction="))?.parse().ok()?;
    Some((n != "rail", shape))
}

/// Directions (north, south, west, east) of a shape's ends, and the end one up.
fn dirs(shape: u8) -> ([usize; 2], Option<usize>) {
    match shape {
        0 => ([0, 1], None),
        1 => ([2, 3], None),
        2 => ([2, 3], Some(3)),
        3 => ([2, 3], Some(2)),
        4 => ([0, 1], Some(0)),
        5 => ([0, 1], Some(1)),
        6 => ([1, 3], None),
        7 => ([1, 2], None),
        8 => ([0, 2], None),
        _ => ([0, 3], None),
    }
}

/// The rail cell an end of the rail at `p` meets: beside it, one up or one down.
fn rail_at(g: &Grid, p: BlockPos, d: usize) -> Option<BlockPos> {
    let q = off(p, SIDES[d]);
    [q, off(q, [0, 1, 0]), off(q, [0, -1, 0])].into_iter().find(|c| rail(g, *c).is_some())
}

fn leads_back(g: &Grid, from: BlockPos, to: BlockPos) -> bool {
    rail(g, from).is_some_and(|(_, s)| dirs(s).0.iter().any(|d| off(from, SIDES[*d])[0] == to[0] && off(from, SIDES[*d])[2] == to[2]))
}

/// Directions in which the rail at `p` is linked both ways.
fn linked(g: &Grid, p: BlockPos) -> Vec<usize> {
    let Some((_, s)) = rail(g, p) else { return Vec::new() };
    dirs(s).0.into_iter().filter(|d| rail_at(g, p, *d).is_some_and(|q| leads_back(g, q, p))).collect()
}

/// East-west once joined along x (also along both, probe `golden-curve-last`), else north-south; a plain rail
/// takes the last curve of NW, NE, SW, SE that fits (`rail-three-new*`).
fn shape_for(g: &Grid, p: BlockPos, straight: bool, [n, s, w, e]: [bool; 4]) -> u8 {
    let mut shape = if w || e { 1 } else { 0 };
    if !straight {
        [(n && w, 8), (n && e, 9), (s && w, 7), (s && e, 6)].into_iter().filter(|c| c.0).for_each(|c| shape = c.1);
    }
    let up = |d: usize| rail(g, off(off(p, SIDES[d]), [0, 1, 0])).is_some();
    match shape {
        0 if up(0) => 4,
        0 if up(1) => 5,
        1 if up(3) => 2,
        1 if up(2) => 3,
        s => s,
    }
}

fn set_shape(g: &Grid, p: BlockPos, shape: u8) {
    let (name, props) = view(g, p);
    let kept: Vec<&str> = props.split(',').filter(|kv| !kv.is_empty() && !kv.starts_with("rail_direction=")).collect();
    let props = format!("{}rail_direction={shape}", kept.iter().map(|k| format!("{k},")).collect::<String>());
    g.set_state(p, name.trim_start_matches("minecraft:"), &props);
}

/// After a rail is placed at `pos`: its shape from the rails that would join it, then those at its ends turn.
pub fn join(g: &Grid, pos: BlockPos) {
    let Some((straight, _)) = rail(g, pos) else { return };
    let free = |q: BlockPos| {
        let l = linked(g, q);
        l.len() < 2 || l.iter().any(|d| off(q, SIDES[*d])[0] == pos[0] && off(q, SIDES[*d])[2] == pos[2])
    };
    let sides: [bool; 4] = std::array::from_fn(|d| rail_at(g, pos, d).is_some_and(free));
    set_shape(g, pos, shape_for(g, pos, straight, sides));
    let (ends, _) = dirs(rail(g, pos).expect("placed").1);
    for d in ends {
        let Some(q) = rail_at(g, pos, d).filter(|q| free(*q)) else { continue };
        let mut to: Vec<usize> = linked(g, q);
        to.push(d ^ 1);
        let mut sides = [false; 4];
        to.iter().for_each(|x| sides[*x] = true);
        let st = rail(g, q).expect("rail").0;
        set_shape(g, q, shape_for(g, q, st, sides));
    }
}
