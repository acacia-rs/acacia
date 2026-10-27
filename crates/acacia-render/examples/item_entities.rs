//! Probe of the parity harness item_models' dropped and framed items: `drop::java_frames`,
//! `framed::java_frame` and `JavaItems::bounds`, and the images of the mesh
//! `JavaItems::drawn_context` picks. Prints `CASES\t<n>` and `DIVERGENCE\t...`.
//!
//! cargo run -p acacia-render --example item_entities -- <look dir> <item_entities.jsonl>

#[path = "java_entities/compare.rs"]
mod compare;

use std::path::Path;

use acacia_render::item::drop::{self, Drop};
use acacia_render::item::framed;
use acacia_render::item::java::context;
use acacia_render::item::JavaItems;
use glam::{DVec3, IVec3, Mat4};
use serde_json::Value;

fn direction(name: &str) -> IVec3 {
    match name {
        "down" => IVec3::NEG_Y,
        "up" => IVec3::Y,
        "north" => IVec3::NEG_Z,
        "south" => IVec3::Z,
        "west" => IVec3::NEG_X,
        _ => IVec3::X,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [look, records] = args.as_slice() else {
        eprintln!("usage: item_entities <look dir> <item_entities.jsonl>");
        std::process::exit(2);
    };
    let items = JavaItems::load(Path::new(look)).expect("the look's java/item_models.json");
    let mut out = compare::Report::default();
    for line in std::fs::read_to_string(records).expect("oracle records").lines().filter(|l| !l.trim().is_empty()) {
        let r: Value = serde_json::from_str(line).expect("oracle record");
        let name = r["case"].as_str().unwrap_or_default();
        let mut case = out.case(name);
        if let Some(error) = r.get("error") {
            case.diverge("oracle", "a record", error);
            continue;
        }
        let mut parts = name.split('|');
        let (kind, item) = (parts.next().unwrap_or_default(), parts.next().unwrap_or_default());
        let display = if kind == "drop" { context::GROUND } else { context::FIXED };
        let Some([lo, hi]) = items.bounds(item, display) else {
            // Java's box of a model without extents is empty at the origin.
            if compare::floats(&r["box"]).iter().any(|&v| v != 0.0) {
                case.diverge("bounds", "a model", "none");
            }
            continue;
        };
        case.numbers("box", &compare::floats(&r["box"]), &[lo.x, lo.y, lo.z, hi.x, hi.y, hi.z], 1e-5);
        let Some(expected) = r["frames"].as_array() else { continue };
        let num = |k: &str| r[k].as_f64().unwrap_or(0.0) as f32;
        let placed: Vec<Mat4> = if kind == "drop" {
            let at = Drop { feet: DVec3::ZERO, count: 0, seed: r["seed"].as_i64().unwrap_or(0) as i32, age_ticks: num("age"), bob_offset: num("bob_offset") };
            let count = [0u16, 1, 2, 17, 33, 64][r["count"].as_u64().unwrap_or(1) as usize];
            drop::java_frames([lo.y, hi.z - lo.z], &Drop { count, ..at })
        } else {
            let rotation = r["rotation"].as_i64().unwrap_or(0) as i32;
            vec![framed::java_frame(direction(r["direction"].as_str().unwrap_or_default()), rotation, r["invisible"].as_bool().unwrap_or(false))]
        };
        let layers = r["layers"].as_u64().unwrap_or(1) as usize;
        let got: Vec<f32> = placed.iter().flat_map(|m| std::iter::repeat_n(m.to_cols_array(), layers)).flatten().collect();
        let want: Vec<f32> = expected.iter().flat_map(compare::floats).collect();
        case.numbers("frames", &want, &got, 1e-5);
        let mut images: Vec<&str> = Vec::new();
        for layer in items.layers(item, items.drawn_context(item, display)).unwrap_or_default() {
            for quad in items.quads(layer) {
                if !images.contains(&quad.image.as_str()) {
                    images.push(&quad.image);
                }
            }
        }
        let want: Vec<&str> = r["images"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        if want != images {
            case.diverge("images", format!("{want:?}"), format!("{images:?}"));
        }
    }
    out.print();
}
