//! Parity probe for tools/parity/harnesses/clouds: the oracle's cloud maps (`textures/*.png`) and
//! frames (`frames_cases.jsonl`) through crate::clouds, written as the oracle's records.
//! usage: clouds_probe <oracle dir> <out dir>

use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use acacia_render::clouds::{self, CELL_SIZE, CloudMap};
use glam::DVec3;
use serde_json::{Value, json};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (oracle, out) = (Path::new(&args[1]), Path::new(&args[2]));
    let mut maps = HashMap::new();
    let mut textures = BufWriter::new(std::fs::File::create(out.join("textures.jsonl")).unwrap());
    for entry in std::fs::read_dir(oracle.join("textures")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_stem().unwrap().to_str().unwrap().to_owned();
        let map = CloudMap::new(&image::open(&path).unwrap().into_rgba8());
        let cells: Vec<u8> = (0..map.height).flat_map(|z| (0..map.width).map(move |x| (x, z))).map(|(x, z)| map.cell(x, z)).collect();
        writeln!(textures, "{}", json!({ "case": format!("texture-{name}"), "cells": cells })).unwrap();
        maps.insert(name, map);
    }
    let cases = BufReader::new(std::fs::File::open(oracle.join("frames_cases.jsonl")).unwrap());
    let mut frames = BufWriter::new(std::fs::File::create(out.join("frames.jsonl")).unwrap());
    for line in cases.lines() {
        let case: Value = serde_json::from_str(&line.unwrap()).unwrap();
        writeln!(frames, "{}", frame(&case, &maps[case["texture"].as_str().unwrap()])).unwrap();
    }
}

fn bits(v: f32) -> String {
    format!("{:x}", v.to_bits())
}

fn frame(case: &Value, map: &CloudMap) -> Value {
    let c = &case["camera"];
    let camera = DVec3::new(c[0].as_f64().unwrap(), c[1].as_f64().unwrap(), c[2].as_f64().unwrap());
    let bottom = case["bottom"].as_f64().unwrap() as f32;
    let frame = clouds::frame(map, bottom, camera, case["game_time"].as_i64().unwrap(), case["partial"].as_f64().unwrap() as f32);
    let radius = clouds::radius_cells(case["range"].as_i64().unwrap() as i32);
    let faces: Vec<String> = clouds::build_mesh(map, &frame, case["fancy"].as_bool().unwrap(), radius)
        .iter()
        .map(|f| format!("{} {} {} {} {}", f.x, f.z, f.dir as u8, u8::from(f.inside), u8::from(f.top_color)))
        .collect();
    let empty = faces.is_empty();
    let mut record = json!({ "case": case["case"], "faces": faces });
    if !empty {
        let color = clouds::color(case["color"].as_i64().unwrap() as u32);
        record["color"] = json!(color.map(bits));
        record["offset"] = json!(frame.offset.to_array().map(bits));
        record["cell_size"] = json!(CELL_SIZE.to_array().map(bits));
    }
    record
}
