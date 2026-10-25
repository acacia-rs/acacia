//! Parity probe for `tools/parity/harnesses/fog`: the oracle's cameras through acacia's
//! `setup_fog`. usage: fog_probe <cases.jsonl> <environment.json> <out.jsonl>

use std::io::{BufRead, BufReader, BufWriter, Write};

use acacia_render::sky::Environment;
use acacia_render::sky::fog::{FogInputs, FogType, PlayerFog, RainFog, night_vision_scale, setup_fog, water_vision};
use serde_json::{Value, json};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, cases, environment, out] = &args[..] else { panic!("usage: fog_probe <cases.jsonl> <environment.json> <out.jsonl>") };
    let doc: Value = serde_json::from_slice(&std::fs::read(environment).expect("environment.json")).expect("environment.json is JSON");
    let env = Environment::parse(&doc);
    let mut out = BufWriter::new(std::fs::File::create(out).expect("out file"));
    for line in BufReader::new(std::fs::File::open(cases).expect("cases")).lines() {
        let case: Value = serde_json::from_str(&line.expect("a line")).expect("a JSON case");
        writeln!(out, "{}", fog(&env, &doc, &case)).expect("write");
    }
}

fn fog(env: &Environment, doc: &Value, c: &Value) -> Value {
    let f = |key: &str| c[key].as_f64().unwrap_or_else(|| panic!("{key}")) as f32;
    let i = |key: &str| c[key].as_i64().unwrap_or_else(|| panic!("{key}")) as i32;
    let b = |key: &str| c[key].as_bool().unwrap_or_else(|| panic!("{key}"));
    let java = c["biome"].as_str().unwrap();
    let biome = doc["biomes"][java]["bedrock"][0].as_str().map_or_else(|| format!("java:{java}"), str::to_owned);
    let dimension = env.dimension(c["dimension"].as_str().unwrap()).expect("a dimension in environment.json");
    let attributes = env.sample(dimension, c["ticks"].as_i64().unwrap(), &[(biome.as_str(), 1.0)], f("rain"), f("thunder"), false);

    let fog_type = match c["fog_type"].as_str().unwrap() {
        "water" => FogType::Water,
        "lava" => FogType::Lava,
        "powder_snow" => FogType::PowderSnow,
        _ => FogType::Atmospheric,
    };
    let partial = f("partial");
    let darkness = c["darkness"].as_array().map(|blend| {
        let (factor, previous) = (blend[0].as_f64().unwrap() as f32, blend[1].as_f64().unwrap() as f32);
        previous + (factor - previous) * partial
    });
    let player = PlayerFog {
        blindness: c["blindness"].as_i64().map(|d| (d as i32, d == -1)),
        darkness,
        night_vision: c["night_vision"].as_i64().map(|d| night_vision_scale(d as i32, partial)),
        fire_resistance: b("fire_resistance"),
        spectator: b("spectator"),
        darken_world: f("darken_world"),
        boss_world_fog: b("boss_fog"),
    };
    let mut rain_fog = RainFog { multiplier: f("rain_fog_before") };
    rain_fog.update(f("rain"), i("sky_light") as u8, b("has_precipitation"), f("delta_ticks"));
    let forward = c["forward"].as_array().unwrap();
    let input = FogInputs {
        fog_type,
        render_distance_chunks: i("render_distance"),
        cloud_range_chunks: i("cloud_range"),
        camera_y: c["camera_y"].as_f64().unwrap(),
        min_y: i("min_y"),
        void_onset: if b("flat") { 1.0 } else { 32.0 },
        forward: std::array::from_fn(|k| forward[k].as_f64().unwrap() as f32),
        rain: f("rain"),
        thunder: f("thunder"),
        player,
        water_vision: water_vision(i("water_vision_time")),
        rain_fog: rain_fog.multiplier,
    };
    let fog = setup_fog(&attributes, &input);
    json!({
        "case": c["case"],
        "color": fog.color,
        "environmental_start": fog.environmental_start,
        "environmental_end": fog.environmental_end,
        "render_distance_start": fog.render_distance_start,
        "render_distance_end": fog.render_distance_end,
        "sky_end": fog.sky_end,
        "cloud_end": fog.cloud_end,
    })
}
