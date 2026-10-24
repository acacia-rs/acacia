//! Java state properties Bedrock keeps in block entities: bed and banner colours, potted plants, floor
//! skull rotation, shulker box facing, chest pairing, lectern books. BlueMap reads no block-entity NBT, so
//! these are the only parts of a block entity that change a render.

use acacia_nbt::Value;
use acacia_world::BlockRegistry;

use crate::column::JavaColumn;
use crate::mapping::{self, JavaState, StateMap};
use crate::section::VOLUME;

/// Java `DyeColor` order (Bedrock beds use it; Bedrock banners count it backwards).
const DYES: [&str; 16] = [
    "white",
    "orange",
    "magenta",
    "light_blue",
    "yellow",
    "lime",
    "pink",
    "gray",
    "light_gray",
    "cyan",
    "purple",
    "blue",
    "brown",
    "green",
    "red",
    "black",
];
const FACINGS: [&str; 6] = ["down", "up", "north", "south", "west", "east"];

fn int(v: &Value, key: &str) -> Option<i64> {
    match v.get(key)? {
        Value::Byte(b) => Some(*b as i64),
        Value::Short(s) => Some(*s as i64),
        Value::Int(i) => Some(*i as i64),
        Value::Long(l) => Some(*l),
        Value::Float(f) => Some(*f as i64),
        _ => None,
    }
}

fn float(v: &Value, key: &str) -> Option<f32> {
    match v.get(key)? {
        Value::Float(f) => Some(*f),
        Value::Double(d) => Some(*d as f32),
        _ => int(v, key).map(|i| i as f32),
    }
}

/// `oak_bed` style name swap keeping every property (beds, banners differ only by colour in Java).
fn recolour(state: JavaState, suffix: &str, dye: usize) -> Option<JavaState> {
    let j = mapping::java();
    let block = j.find_block(&format!("minecraft:{}_{suffix}", DYES.get(dye)?))?;
    let props: Vec<_> = j.properties(state).collect();
    j.state(block.name, &props)
}

fn potted(registry: &BlockRegistry, map: &StateMap, be: &Value) -> Option<JavaState> {
    let plant = be.get("PlantBlock")?;
    let Some(Value::String(name)) = plant.get("name") else {
        return None;
    };
    let mut props: Vec<String> = match plant.get("states") {
        Some(Value::Compound(entries)) => entries
            .iter()
            .map(|(k, v)| match v {
                Value::String(s) => format!("{k}={s}"),
                Value::Byte(b) => format!("{k}={b}"),
                Value::Int(i) => format!("{k}={i}"),
                _ => format!("{k}="),
            })
            .collect(),
        _ => Vec::new(),
    };
    props.sort();
    let id = registry
        .find(name, &props.join(","))
        .or_else(|| registry.states_of(name).next().map(|(id, _)| id))?;
    let j = mapping::java();
    let plant = j.block(map.java(id)).name.strip_prefix("minecraft:")?;
    let pot = match plant {
        "azalea" | "flowering_azalea" => format!("minecraft:potted_{plant}_bush"),
        p => format!("minecraft:potted_{p}"),
    };
    j.find_block(&pot).map(|b| b.default_state())
}

fn rotate_cw(f: &str) -> &'static str {
    match f {
        "north" => "east",
        "east" => "south",
        "south" => "west",
        _ => "north",
    }
}

fn offset(f: &str) -> [i32; 2] {
    match f {
        "north" => [0, -1],
        "south" => [0, 1],
        "west" => [-1, 0],
        _ => [1, 0],
    }
}

/// The state a block entity implies for the block at its position, if it changes anything.
fn implied(
    registry: &BlockRegistry,
    map: &StateMap,
    state: JavaState,
    pos: [i32; 3],
    be: &Value,
) -> Option<JavaState> {
    let j = mapping::java();
    let name = j.block(state).name;
    let Some(Value::String(id)) = be.get("id") else {
        return None;
    };
    match &**id {
        "Bed" if name.ends_with("_bed") => recolour(state, "bed", int(be, "color")? as usize),
        "Banner" if name.ends_with("_wall_banner") => recolour(
            state,
            "wall_banner",
            15 - int(be, "Base").unwrap_or(0) as usize,
        ),
        "Banner" if name.ends_with("_banner") => {
            recolour(state, "banner", 15 - int(be, "Base").unwrap_or(0) as usize)
        }
        "FlowerPot" if name.starts_with("minecraft:potted_") || name == "minecraft:flower_pot" => {
            potted(registry, map, be)
        }
        "Skull" if j.block(state).has_property("rotation") => {
            let r = ((float(be, "Rotation")? / 22.5).round() as i32).rem_euclid(16);
            j.with(state, "rotation", &r.to_string())
        }
        "ShulkerBox" => j.with(state, "facing", FACINGS.get(int(be, "facing")? as usize)?),
        "Lectern" => j.with(
            state,
            "has_book",
            if int(be, "hasBook").unwrap_or(0) != 0 {
                "true"
            } else {
                "false"
            },
        ),
        "Chest" => {
            let (px, pz) = (int(be, "pairx")? as i32, int(be, "pairz")? as i32);
            let facing = j.property(state, "facing")?;
            let d = [px - pos[0], pz - pos[2]];
            // Java's left half has its partner clockwise of its facing.
            let ty = if d == offset(rotate_cw(facing)) {
                "left"
            } else if d == offset(rotate_cw(rotate_cw(rotate_cw(facing)))) {
                "right"
            } else {
                return None;
            };
            j.with(state, "type", ty)
        }
        _ => None,
    }
}

/// Applies what `block_entities` (world positions and Bedrock NBT) imply to a converted column.
pub fn apply<'a>(
    col: &mut JavaColumn,
    block_entities: impl IntoIterator<Item = ([i32; 3], &'a Value)>,
    registry: &BlockRegistry,
    map: &StateMap,
    buf: &mut [JavaState; VOLUME],
) {
    let (bx, bz, min_y) = (col.x * 16, col.z * 16, col.min_y());
    let mut edits = Vec::new();
    for (pos, be) in block_entities {
        let (lx, lz, ly) = (pos[0] - bx, pos[2] - bz, pos[1] - min_y);
        if !(0..16).contains(&lx) || !(0..16).contains(&lz) || !(0..col.height()).contains(&ly) {
            continue;
        }
        let state = col.block(lx as usize, pos[1], lz as usize);
        if let Some(new) = implied(registry, map, state, pos, be).filter(|&n| n != state) {
            edits.push(([lx as usize, ly as usize, lz as usize], new));
        }
    }
    if !edits.is_empty() {
        col.apply_edits(&mut edits, buf);
    }
}
