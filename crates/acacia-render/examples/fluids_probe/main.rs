//! CPU probe for tools/parity's `fluids` harness (docs/parity.md). Builds each case's section from
//! the oracle's Java block states, mapped to Bedrock by the table the Java look bakes with, takes
//! the block table from a baked Java look, runs `mesh::liquid::java_faces` on every liquid cell and
//! writes the faces as the oracle records Java's. Then checks every state's fluid facts ([`facts`]).

mod facts;

use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Write};

use acacia_render::biome::{BiomeColors, BiomeDef};
use acacia_render::blocks::{BlockTable, Fluid, Layer, LightFacts};
use acacia_render::lookpack::{LookPack, state_key};
use acacia_render::mesh::liquid::{LiquidFace, Sprite, java_faces, light_coords};
use acacia_render::mesh::smooth::LightWorld;
use acacia_render::mesh::volume::{Volume, cell};
use acacia_world::BlockRegistry;
use serde_json::{Value, json};

const USAGE: &str = "usage: fluids_probe <cases.jsonl> <blockstate_mappings.json> <look dir> <out.jsonl> <facts.jsonl>";
/// The oracle's section: world y 64..80.
const MIN_Y: i32 = 64;
const BIOME: u16 = 1;

/// Bedrock state keys by Java state key, both with properties sorted by name.
fn java_to_bedrock(path: &str) -> HashMap<String, String> {
    let table: HashMap<String, String> = serde_json::from_slice(&std::fs::read(path).expect("mapping")).expect("mapping json");
    let mut out: HashMap<String, String> = HashMap::new();
    for (bedrock, java) in table {
        let bedrock = sorted(&bedrock);
        let slot = out.entry(sorted(&java)).or_insert_with(|| bedrock.clone());
        if bedrock < *slot {
            *slot = bedrock;
        }
    }
    out
}

fn sorted(state: &str) -> String {
    let Some((name, properties)) = state.split_once('[') else { return state.to_owned() };
    let mut pairs: Vec<&str> = properties.trim_end_matches(']').split(',').collect();
    pairs.sort_unstable_by_key(|p| p.split_once('=').map_or(*p, |(key, _)| key));
    format!("{name}[{}]", pairs.join(","))
}

struct World {
    ids: HashMap<String, u32>,
    mapping: HashMap<String, String>,
}

impl World {
    /// A Java state as the Bedrock world would hold it; waterlogging is the liquid layer's business.
    /// States the table never produces (it folds some properties away) take the state of the same
    /// block sharing the most property values.
    fn bedrock(&self, java: &str) -> Option<u32> {
        self.exact(java).or_else(|| self.nearest(java))
    }

    /// The Bedrock state the table maps to `java` (waterlogging aside), if any.
    fn exact(&self, java: &str) -> Option<u32> {
        let dry = java.replace("waterlogged=true", "waterlogged=false");
        self.mapping.get(java).or_else(|| self.mapping.get(&dry)).and_then(|b| self.ids.get(b)).copied()
    }

    fn nearest(&self, java: &str) -> Option<u32> {
        let dry = java.replace("waterlogged=true", "waterlogged=false");
        let props = |s: &str| -> Vec<String> { s.split_once('[').map_or(vec![], |(_, p)| p.trim_end_matches(']').split(',').map(str::to_owned).collect()) };
        let name = |s: &str| s.split('[').next().unwrap_or(s).to_owned();
        let wanted = props(&dry);
        let candidates = self.mapping.iter().filter(|(j, _)| name(j) == name(&dry));
        let best = candidates.max_by_key(|(j, b)| (props(j).iter().filter(|p| wanted.contains(p)).count(), std::cmp::Reverse((*b).clone())));
        best.and_then(|(_, b)| self.ids.get(b)).copied()
    }
}

fn sprite_name(fluid: Fluid, sprite: Sprite) -> String {
    let fluid = if fluid == Fluid::Lava { "lava" } else { "water" };
    let sprite = match sprite {
        Sprite::Still => "still",
        Sprite::Flow => "flow",
        Sprite::Overlay => "overlay",
    };
    format!("minecraft:block/{fluid}_{sprite}")
}

fn record(world: &World, table: &BlockTable, case: &Value, look: &str) -> Value {
    let air = world.ids["minecraft:air"];
    let mut v = Volume::filled(air);
    v.biomes.fill(u32::from(BIOME));
    v.liquid.fill(air);
    let mut unmapped = Vec::new();
    for b in case["blocks"].as_array().expect("blocks") {
        let [x, y, z] = [0, 1, 2].map(|i| b[i].as_i64().expect("coordinate") as i32);
        let p = [x, y - MIN_Y, z];
        let mut place = |key: &str, layer: &mut [u32]| match world.bedrock(key) {
            Some(id) => layer[cell(p[0], p[1], p[2])] = id,
            None => unmapped.push(key.to_owned()),
        };
        place(b[3].as_str().expect("state"), &mut v.blocks[..]);
        if let Some(fluid) = b[4].as_str() {
            place(fluid, &mut v.liquid[..]);
        }
    }
    let name = match case["biome"].as_str().expect("biome") {
        "swamp" => "swampland",
        "nether_wastes" => "hell",
        other => other,
    };
    let def = BiomeDef { id: BIOME, name: name.to_owned(), temperature: 0.8, downfall: 0.4 };
    let biomes = BiomeColors::build(&[def], std::path::Path::new(look));
    let nether = case["nether"].as_bool().unwrap_or(false);
    let mut quads = Vec::new();
    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                for face in java_faces(&v, table, &biomes, [x, y, z]) {
                    quads.push(quad(table, &v, [x, y, z], &face, nether));
                }
            }
        }
    }
    json!({"case": case["case"], "quads": quads, "unmapped": unmapped})
}

/// The oracle's light over the section's blocks, as the mesher's lighter sees a section's.
struct OracleLight<'a> {
    table: &'a BlockTable,
    v: &'a Volume,
}

impl LightWorld for OracleLight<'_> {
    fn facts(&self, [x, y, z]: [i32; 3]) -> LightFacts {
        self.table.get(self.v.block(x, y, z)).light
    }

    /// `Scene.block` and `Scene.sky` at the world position.
    fn light(&self, [x, y, z]: [i32; 3]) -> (u8, u8) {
        let y = y + MIN_Y;
        (((x * 3 + y * 5 + z * 11 + 3) & 15) as u8, ((x * 7 + y * 3 + z * 5) & 15) as u8)
    }
}

fn quad(table: &BlockTable, v: &Volume, p: [i32; 3], face: &LiquidFace, nether: bool) -> Value {
    let me = table.get(v.block(p[0], p[1], p[2]));
    let me = if me.fluid == Fluid::None { table.get(v.liquid(p[0], p[1], p[2])) } else { me };
    let packed = light_coords(&OracleLight { table, v }, face);
    let [r, g, b] = face.color(nether);
    let rgb = i64::from(r) << 16 | i64::from(g) << 8 | i64::from(b);
    let sprite = sprite_name(me.fluid, face.sprite);
    let vertices: Vec<Value> =
        (0..4).map(|i| json!([face.corners[i][0], face.corners[i][1], face.corners[i][2], face.uv[i][0], face.uv[i][1], rgb, 255, packed, sprite])).collect();
    let layer = if me.layer == Layer::Translucent { "translucent" } else { "solid" };
    json!({"block": [p[0], p[1] + MIN_Y, p[2]], "layer": layer, "vertices": vertices})
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [cases, mapping, look, out, facts] = &args[..] else { panic!("{USAGE}") };
    let registry = BlockRegistry::vanilla();
    let ids = (0..registry.len() as u32).filter_map(|id| Some((state_key(registry.get(id)?), id))).collect();
    let world = World { ids, mapping: java_to_bedrock(mapping) };
    let table = LookPack::load(std::path::Path::new(look)).expect("look pack").block_table(registry);
    let mut writer = BufWriter::new(std::fs::File::create(out).expect("out"));
    let mut count = 0;
    for line in BufReader::new(std::fs::File::open(cases).expect("cases")).lines() {
        let line = line.expect("read");
        if line.trim().is_empty() {
            continue;
        }
        let case: Value = serde_json::from_str(&line).expect("case json");
        writeln!(writer, "{}", record(&world, &table, &case, look)).expect("write");
        count += 1;
    }
    eprintln!("fluids_probe: {count} cases meshed");
    facts::check(&world, &table, facts);
}
