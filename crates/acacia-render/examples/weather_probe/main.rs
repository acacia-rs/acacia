//! Parity probe for tools/parity/harnesses/weather: reads the oracle's `*_cases.jsonl` and writes
//! acacia's answers as the same `*.jsonl` records. usage: weather_probe <oracle dir> <out dir>

mod scene;

use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use acacia_render::lightning;
use acacia_render::weather::climate::{Climate, Precipitation, SEA_LEVEL};
use acacia_render::weather::columns::{self, Column, WeatherVertex};
use acacia_render::weather::splash::{ParticleStatus, RainTicker, RainSound};
use glam::DVec3;
use scene::Scene;
use serde_json::{Value, json};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (oracle, out) = (Path::new(&args[1]), Path::new(&args[2]));
    let biomes: HashMap<String, Value> = serde_json::from_slice(&std::fs::read(oracle.join("biomes.json")).unwrap()).unwrap();
    let climates: HashMap<String, Climate> = biomes
        .into_iter()
        .map(|(name, b)| {
            let climate = Climate { temperature: b["temperature"].as_f64().unwrap() as f32, frozen: b["frozen"].as_bool().unwrap(), has_precipitation: b["has_precipitation"].as_bool().unwrap() };
            (name, climate)
        })
        .collect();
    each(oracle, out, "columns", |case| columns(case, &climates));
    each(oracle, out, "climate", |case| climate(case, &climates));
    each(oracle, out, "splashes", |case| splashes(case, &climates));
    each(oracle, out, "bolts", bolts);
}

fn each(oracle: &Path, out: &Path, kind: &str, answer: impl Fn(&Value) -> Value) {
    let cases = BufReader::new(std::fs::File::open(oracle.join(format!("{kind}_cases.jsonl"))).unwrap());
    let mut records = BufWriter::new(std::fs::File::create(out.join(format!("{kind}.jsonl"))).unwrap());
    for line in cases.lines() {
        let case: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let mut record = answer(&case);
        record["case"] = case["case"].clone();
        writeln!(records, "{record}").unwrap();
    }
}

fn f32_bits(v: f32) -> String {
    if v.is_nan() { "nan".into() } else { format!("{:x}", v.to_bits()) }
}

fn f64_bits(v: f64) -> String {
    if v.is_nan() { "nan".into() } else { format!("{:x}", v.to_bits()) }
}

/// `LightCoordsUtil.pack`.
fn coords([block, sky]: [u8; 2]) -> u32 {
    u32::from(block) << 4 | u32::from(sky) << 20
}

fn camera(case: &Value) -> DVec3 {
    let c = &case["camera"];
    DVec3::new(c[0].as_f64().unwrap(), c[1].as_f64().unwrap(), c[2].as_f64().unwrap())
}

fn columns(case: &Value, climates: &HashMap<String, Climate>) -> Value {
    let scene = Scene::new(case, climates);
    let camera = camera(case);
    let rain = case["rain"].as_f64().unwrap() as f32;
    let found = columns::extract(&scene, rain, case["game_time"].as_i64().unwrap(), case["partial"].as_f64().unwrap() as f32, camera, case["radius"].as_i64().unwrap() as i32);
    let line = |c: &Column| format!("{} {} {} {} {} {} {}", c.x, c.z, c.bottom, c.top, f32_bits(c.u_offset), f32_bits(c.v_offset), coords(c.light));
    let vertex = |v: &WeatherVertex| {
        let [x, y, z] = v.pos.map(f32_bits);
        let [u, w] = v.uv.map(f32_bits);
        format!("{x} {y} {z} {u} {w} {:08x} {}", u32::from(v.alpha) << 24 | 0xFF_FFFF, coords(v.light))
    };
    let vertices: Vec<String> = columns::quads(&found, camera).iter().flatten().map(vertex).collect();
    json!({ "rain": found.rain.iter().map(line).collect::<Vec<_>>(), "snow": found.snow.iter().map(line).collect::<Vec<_>>(), "vertices": vertices })
}

fn climate(case: &Value, climates: &HashMap<String, Climate>) -> Value {
    let climate = climates[case["biome"].as_str().unwrap()];
    let p = &case["pos"];
    let pos = [0, 1, 2].map(|i| p[i].as_i64().unwrap() as i32);
    let precipitation = match climate.precipitation_at(pos, SEA_LEVEL) {
        Precipitation::None => "NONE",
        Precipitation::Rain => "RAIN",
        Precipitation::Snow => "SNOW",
    };
    json!({ "temperature": f32_bits(climate.temperature_at(pos, SEA_LEVEL)), "precipitation": precipitation })
}

fn splashes(case: &Value, climates: &HashMap<String, Climate>) -> Value {
    let scene = Scene::new(case, climates);
    let status = match case["particles"].as_str().unwrap() {
        "all" => ParticleStatus::All,
        "decreased" => ParticleStatus::Decreased,
        _ => ParticleStatus::Minimal,
    };
    let (start, rain, radius) = (case["game_time"].as_i64().unwrap(), case["rain"].as_f64().unwrap() as f32, case["radius"].as_i64().unwrap() as i32);
    let mut ticker = RainTicker::default();
    let ticks: Vec<Value> = (0..case["ticks"].as_i64().unwrap())
        .map(|t| {
            let (splashes, sound) = ticker.tick(&scene, rain, start + t, camera(case), radius, status);
            let splashes: Vec<String> = splashes
                .iter()
                .map(|s| format!("{} {} {} {}", if s.smoke { "smoke" } else { "rain" }, f64_bits(s.pos.x), f64_bits(s.pos.y), f64_bits(s.pos.z)))
                .collect();
            let sound = sound.map(|RainSound { above, at: [x, y, z] }| format!("{} {x} {y} {z}", if above { "above" } else { "rain" }));
            json!({ "splashes": splashes, "sound": sound })
        })
        .collect();
    json!({ "ticks": ticks })
}

fn bolts(case: &Value) -> Value {
    let seed: i64 = case["seed"].as_str().unwrap().parse().unwrap();
    let [r, g, b, a] = lightning::COLOR;
    let color = format!("{:08x}", u32::from_be_bytes([a, r, g, b]));
    let vertices: Vec<String> =
        lightning::bolt(seed).iter().flatten().map(|v| format!("{} {} {} {color}", f32_bits(v.x), f32_bits(v.y), f32_bits(v.z))).collect();
    json!({ "vertices": vertices })
}
