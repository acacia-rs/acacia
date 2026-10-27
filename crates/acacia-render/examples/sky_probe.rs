//! Parity probe for `tools/parity/harnesses/sky`: the oracle's cases through acacia's environment
//! attributes and sky geometry. usage: sky_probe <cases.jsonl> <environment.json> <out.jsonl>

use std::io::{BufRead, BufReader, BufWriter, Write};

use acacia_render::sky::geometry::{self, END_SKY_SHADE, MOON_SIZE, SUN_SIZE};
use acacia_render::sky::timeline::MOON_PHASES;
use acacia_render::sky::{Attr, Attributes, Environment, blend};
use glam::Mat4;
use serde_json::{Map, Value, json};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, cases, environment, out] = &args[..] else { panic!("usage: sky_probe <cases.jsonl> <environment.json> <out.jsonl>") };
    let doc: Value = serde_json::from_slice(&std::fs::read(environment).expect("environment.json")).expect("environment.json is JSON");
    let env = Environment::parse(&doc);
    let mut out = BufWriter::new(std::fs::File::create(out).expect("out file"));
    for line in BufReader::new(std::fs::File::open(cases).expect("cases")).lines() {
        let case: Value = serde_json::from_str(&line.expect("a line")).expect("a JSON case");
        let mut record = match case["kind"].as_str() {
            Some("attributes" | "gauss") => attributes(&env, &doc, &case),
            Some("geometry") => geometry(case["case"].as_str().unwrap_or_default()),
            Some("frame") => frame(&case),
            kind => panic!("unknown case kind {kind:?}"),
        };
        record.insert("case".into(), case["case"].clone());
        writeln!(out, "{}", Value::Object(record)).expect("write");
    }
}

/// The name acacia knows a Java biome by: its first Bedrock name, or one no layer has.
fn bedrock(doc: &Value, java: &str) -> String {
    doc["biomes"][java]["bedrock"][0].as_str().map_or_else(|| format!("java:{java}"), str::to_owned)
}

fn attributes(env: &Environment, doc: &Value, case: &Value) -> Map<String, Value> {
    let weights: Vec<(String, f64)> = match &case["position"] {
        Value::Array(p) => {
            let palette: Vec<&str> = case["palette"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
            let at = |x: i32, y: i32, z: i32| (x + 2 * z + 3 * y).rem_euclid(palette.len() as i32) as usize;
            let position = [0, 1, 2].map(|i| p[i].as_f64().unwrap());
            blend::gaussian_weights(position, at).into_iter().map(|(i, w)| (bedrock(doc, palette[i]), w)).collect()
        }
        _ => case["biomes"].as_array().unwrap().iter().map(|b| (bedrock(doc, b[0].as_str().unwrap()), b[1].as_f64().unwrap())).collect(),
    };
    let weights: Vec<(&str, f64)> = weights.iter().map(|(n, w)| (n.as_str(), *w)).collect();
    let dimension = env.dimension(case["dimension"].as_str().unwrap()).expect("a dimension in environment.json");
    let number = |key: &str| case[key].as_f64().unwrap() as f32;
    let a = env.sample(dimension, case["ticks"].as_i64().unwrap(), &weights, number("rain"), number("thunder"), case["flash"].as_bool().unwrap());
    Attr::ALL.iter().map(|&attr| (attr.name().to_owned(), value(&a, attr))).collect()
}

fn value(a: &Attributes, attr: Attr) -> Value {
    let v = a.get(attr);
    match (attr, attr.arity()) {
        (Attr::MoonPhase, _) => json!(MOON_PHASES[usize::from(a.moon_phase())]),
        (_, 1) => json!(v[0]),
        (_, n) => json!(v[..n]),
    }
}

fn flat(points: impl IntoIterator<Item = [f32; 3]>) -> Value {
    json!(points.into_iter().flatten().collect::<Vec<_>>())
}

fn quad_record(quads: &[geometry::Quad]) -> Map<String, Value> {
    let corners = || quads.iter().flatten();
    let mut m = Map::new();
    m.insert("positions".into(), flat(corners().map(|c| c.0)));
    m.insert("uvs".into(), json!(corners().flat_map(|c| c.1).collect::<Vec<_>>()));
    m
}

fn geometry(case: &str) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("kind".into(), json!("geometry"));
    match case.trim_start_matches("geometry/") {
        "sky_disc" => drop(m.insert("positions".into(), flat(geometry::sky_disc(geometry::SKY_DISC_Y)))),
        "dark_disc" => drop(m.insert("positions".into(), flat(geometry::sky_disc(geometry::DARK_DISC_Y)))),
        "stars" => drop(m.insert("positions".into(), flat(geometry::stars().into_iter().flatten()))),
        "sunrise_fan" => {
            let fan = geometry::sunrise_fan_model();
            m.insert("positions".into(), flat(fan));
            let colors: Vec<u8> = (0..fan.len()).flat_map(|i| [255, 255, 255, if i == 0 { 255 } else { 0 }]).collect();
            m.insert("colors".into(), json!(colors));
        }
        "sun" => m.extend(quad_record(&[geometry::celestial_quad()])),
        "moon" => m.extend(quad_record(&[geometry::moon_quad(); 8])),
        "end_sky" => {
            let faces = geometry::end_sky();
            m.extend(quad_record(&faces));
            let shade = (END_SKY_SHADE * 255.0).round() as u8;
            m.insert("colors".into(), json!([shade, shade, shade, 255].repeat(24)));
        }
        other => panic!("unknown geometry {other}"),
    }
    m
}

fn frame(case: &Value) -> Map<String, Value> {
    let angle = case["angle"].as_f64().unwrap_or(0.0) as f32;
    let m: Mat4 = match case["body"].as_str().unwrap() {
        "sun" => geometry::body_frame(angle, SUN_SIZE),
        "moon" => geometry::body_frame(angle, MOON_SIZE),
        "stars" => geometry::celestial_frame(angle),
        "sunrise" => geometry::sunrise_frame(angle, case["alpha"].as_f64().unwrap() as f32),
        "dark_disc" => geometry::dark_disc_frame(),
        other => panic!("unknown frame {other}"),
    };
    let mut out = Map::new();
    out.insert("matrix".into(), json!(m.to_cols_array()));
    out
}
