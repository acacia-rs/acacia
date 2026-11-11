//! Probe of the particles parity harness (tools/parity/harnesses/particles): runs each case through
//! `particles::java::probe` with the seeds Java's particles got and writes one record per case, as
//! the oracle writes its own.
//!
//! cargo run -p acacia-render --example java_particles -- <cases.jsonl> <seeds.jsonl> <out.jsonl>

mod record;
mod scene;

use std::io::Write;

use acacia_render::particles::java::probe::{self, Seeds};
use acacia_render::particles::java::{Options, Type};
use glam::DVec3;
use serde_json::Value;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [cases, seeds, out] = args.as_slice() else {
        eprintln!("usage: java_particles <cases.jsonl> <seeds.jsonl> <out.jsonl>");
        std::process::exit(2);
    };
    let seeds: std::collections::HashMap<String, Vec<Seeds>> = lines(seeds).map(|s| (s["case"].as_str().unwrap().to_owned(), record::seeds(&s))).collect();
    let mut out = std::io::BufWriter::new(std::fs::File::create(out).expect("out file"));
    for case in lines(cases) {
        let name = case["case"].as_str().expect("case name");
        let given = seeds.get(name).map_or(&[][..], Vec::as_slice);
        let blocks = scene::Scene::new(&case["blocks"]);
        let (seed, ticks) = (case["seed"].as_i64().unwrap(), case["ticks"].as_u64().unwrap() as usize);
        let run = if case["kind"] == "block" {
            let foliage = case["foliage"].as_u64().expect("foliage") as u32;
            probe::block(&blocks, vec3(&case["cell"]).as_ivec3(), seed, ticks, foliage, given)
        } else {
            let p = &case["particle"];
            probe::particle(&blocks, &options(p), vec3(&p["pos"]), vec3(&p["vel"]), seed, ticks, given)
        };
        writeln!(out, "{}", record::case(name, &run)).expect("write record");
    }
}

fn lines(path: &str) -> impl Iterator<Item = Value> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    text.lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).expect("json line")).collect::<Vec<_>>().into_iter()
}

pub fn vec3(v: &Value) -> DVec3 {
    let c = |i: usize| v[i].as_f64().expect("number");
    DVec3::new(c(0), c(1), c(2))
}

/// The case's `ParticleOptions`: colours as Java's ints, scaled as Java's floats.
fn options(p: &Value) -> Options {
    let name = p["type"].as_str().expect("type");
    let ty = Type::from_name(name).unwrap_or_else(|| panic!("no particle type {name}"));
    let rgb = |key: &str| p[key].as_u64().map(|c| c as u32);
    let scale = p["scale"].as_f64().map_or(1.0, |s| s as f32);
    match (rgb("color"), rgb("to")) {
        (Some(from), Some(to)) => {
            let to = Options::colored(ty, to).color;
            Options { scale, to_color: [to[0], to[1], to[2]], ..Options::colored(ty, from) }
        }
        (Some(color), None) => Options { scale, ..Options::colored(ty, color) },
        _ => Options::simple(ty),
    }
}
