//! Parity probe for harness `lightmap` (docs/parity.md): the lightmap state and texture of each
//! case. Usage: `lightmap_probe <cases.jsonl> <out.jsonl>`.

use std::io::{BufRead, BufReader, BufWriter, Write};

use acacia_render::light::environment::{LightConditions, java_state};
use acacia_render::sky::{Realm, Weather};
use serde_json::{Value, json};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cases = BufReader::new(std::fs::File::open(&args[1]).expect("cases"));
    let mut out = BufWriter::new(std::fs::File::create(&args[2]).expect("out"));
    for line in cases.lines().map(Result::unwrap).filter(|l| !l.trim().is_empty()) {
        let c: Value = serde_json::from_str(&line).expect("case");
        let f = |k: &str| c[k].as_f64().unwrap_or_else(|| panic!("{k}")) as f32;
        let realm = match c["dimension"].as_str().unwrap() {
            "overworld" => Realm::Overworld,
            "the_nether" => Realm::Nether,
            _ => Realm::End,
        };
        let weather = Weather { rain: f("rain"), thunder: f("thunder"), flash: if c["flash"].as_bool().unwrap() { 1.0 } else { 0.0 } };
        let conditions = LightConditions {
            gamma: f("gamma"),
            night_vision: f("night_vision"),
            darkness: f("darkness") * f("darkness_scale"),
            darkness_scale: f("darkness_scale"),
            age: f("age"),
            boss_darkening: f("boss"),
            end_flash: (f("end_flash"), c["boss_fog"].as_bool().unwrap()),
            flicker: f("flicker"),
        };
        let s = java_state(realm, c["ticks"].as_i64().unwrap() as f32, weather, &conditions);
        let table: Vec<u8> = s.table().iter().flat_map(|t| [t[0], t[1], t[2]]).collect();
        let record = json!({
            "case": c["case"],
            "sky_factor": s.sky_factor, "sky_color": s.sky_color,
            "block_factor": s.block_factor, "block_tint": s.block_tint,
            "ambient": s.ambient,
            "night_vision": s.night_vision, "night_vision_color": s.night_vision_color,
            "darkness": s.darkness, "brightness": s.brightness, "boss": s.boss_darkening,
            "table": table,
        });
        writeln!(out, "{record}").unwrap();
    }
}
