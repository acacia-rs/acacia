//! Parity probe for harness `smooth_light` (docs/parity.md): lights the oracle's quads with the
//! facts and light of its scenes. Usage: `smooth_light_probe <scenes.jsonl> <out.jsonl>`.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Write};

use acacia_render::mesh::smooth::{Dir, LightFacts, LightWorld, Lighter, QuadIn, facing};
use serde_json::{Value, json};

struct Scene {
    facts: HashMap<[i32; 3], LightFacts>,
    light: HashMap<[i32; 3], (u8, u8)>,
    /// Block and sky level of cells not listed.
    default: (u8, u8),
}

impl LightWorld for Scene {
    fn facts(&self, p: [i32; 3]) -> LightFacts {
        self.facts.get(&p).copied().unwrap_or_default()
    }

    fn light(&self, p: [i32; 3]) -> (u8, u8) {
        self.light.get(&p).copied().unwrap_or(self.default)
    }
}

fn ints<const N: usize>(v: &Value) -> [i32; N] {
    std::array::from_fn(|i| v[i].as_i64().unwrap() as i32)
}

fn scene(row: &Value) -> Scene {
    let facts = row["cells"].as_array().unwrap().iter().map(|c| {
        let flag = |k: &str| c[k].as_bool().unwrap();
        let level = |k: &str| c[k].as_u64().unwrap() as u8;
        let facts = LightFacts {
            emission: level("emission"),
            dampening: level("dampening"),
            solid_render: flag("solid_render"),
            full_collision: flag("full_collision"),
            emissive: flag("emissive"),
            dark: flag("dark"),
            ambient_occlusion: false,
            shape: 0,
        };
        (ints::<3>(&c["pos"]), facts)
    });
    let [sky, block] = ints::<2>(&row["light"]);
    let light = row["lights"].as_array().unwrap().iter().map(|l| {
        let [x, y, z, sky, block] = ints::<5>(l);
        ([x, y, z], (block as u8, sky as u8))
    });
    Scene { facts: facts.collect(), light: light.collect(), default: (block as u8, sky as u8) }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input = BufReader::new(std::fs::File::open(&args[1]).expect("scenes"));
    let mut out = BufWriter::new(std::fs::File::create(&args[2]).expect("out"));
    for line in input.lines().map(Result::unwrap).filter(|l| !l.trim().is_empty()) {
        let row: Value = serde_json::from_str(&line).expect("scene");
        let lighter = Lighter { world: scene(&row), nether: row["nether"].as_bool().unwrap() };
        let quads: Vec<Value> = row["quads"].as_array().unwrap().iter().map(|q| {
            let positions: [[f32; 3]; 4] = std::array::from_fn(|i| std::array::from_fn(|k| q["positions"][i][k].as_f64().unwrap() as f32));
            let quad = QuadIn {
                pos: ints::<3>(&q["pos"]),
                positions,
                shade: q["shade"].as_u64().unwrap() as Dir,
                cull: q["cull"].as_u64().map(|d| d as Dir),
                ambient_occlusion: q["ao"].as_bool().unwrap(),
            };
            let lit = lighter.light(&quad);
            json!({ "direction": facing(&positions), "coords": lit.coords, "grey": lit.grey })
        }).collect();
        writeln!(out, "{}", json!({ "case": row["case"], "quads": quads })).unwrap();
    }
}
