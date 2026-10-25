//! Probe of the parity harness item_models (tools/parity/harnesses/item_models): builds the look's
//! Java items as acacia draws them (item::JavaItems: the skin per item and context, placed by its
//! display) and compares each quad with the oracle's. Prints `CASES\t<n>` and
//! `DIVERGENCE\t<case>\t<field>\t<expected>\t<got>`.
//!
//! cargo run -p acacia-render --example java_items -- <look dir> <items.jsonl>

#[path = "java_entities/compare.rs"]
mod compare;

use std::path::Path;

use acacia_render::item::JavaItems;
use glam::Vec3;
use serde_json::Value;

use compare::{Case, floats};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [look, records] = args.as_slice() else {
        eprintln!("usage: java_items <look dir> <items.jsonl>");
        std::process::exit(2);
    };
    let items = JavaItems::load(Path::new(look)).expect("the look's java/item_models.json");
    let mut out = compare::Report::default();
    for line in std::fs::read_to_string(records).expect("oracle records").lines().filter(|l| !l.trim().is_empty()) {
        let record: Value = serde_json::from_str(line).expect("oracle record");
        let name = record["case"].as_str().expect("case");
        let mut case = out.case(name);
        if let Some(error) = record.get("error") {
            case.diverge("oracle", "drawn", error);
            continue;
        }
        let (item, context) = name.split_once('|').expect("item|context");
        compare(&items, item, context, &record, &mut case);
    }
    out.print();
}

fn compare(items: &JavaItems, item: &str, context: &str, record: &Value, case: &mut Case) {
    if let Some(skipped) = record["skipped"].as_object().filter(|s| !s.is_empty()) {
        case.diverge("skipped", "{}", Value::Object(skipped.clone()));
    }
    let expected = record["quads"].as_array().map_or(&[][..], Vec::as_slice);
    let (Some(mesh), Some(display)) = (items.mesh(item, context, None), items.display(item, context)) else {
        if !expected.is_empty() {
            case.diverge("quads.len", expected.len(), 0);
        }
        return;
    };
    let vertices = &mesh.skin.mesh.as_ref().expect("an item skin has its mesh").vertices;
    if vertices.len() / 6 != expected.len() {
        return case.diverge("quads.len", expected.len(), vertices.len() / 6);
    }
    let frame = display.mesh_frame();
    let mut got: Vec<Quad> = mesh.quad_regions.iter().enumerate().map(|(i, &region)| {
        let region = &mesh.regions[region];
        let corners = [0, 1, 2, 5].map(|k| &vertices[i * 6 + k]);
        Quad {
            image: region.image.clone(),
            argb: i64::from(region.argb as i32),
            pos: corners.iter().flat_map(|v| frame.transform_point3(Vec3::from(v.position)).to_array()).collect(),
            uv: corners.iter().flat_map(|v| region.sprite_uv(&mesh.skin, v.uv)).collect(),
        }
    }).collect();
    let mut want: Vec<Quad> = expected.iter().map(|q| Quad {
        image: q["image"].as_str().unwrap_or_default().to_owned(),
        argb: q["argb"].as_i64().unwrap_or_default(),
        pos: floats(&q["pos"]),
        uv: floats(&q["uv"]),
    }).collect();
    // ItemModelGenerator's side faces come out of a hash set: their order differs between runs.
    got.sort_by_cached_key(Quad::key);
    want.sort_by_cached_key(Quad::key);
    for (i, (want, got)) in want.iter().zip(&got).enumerate() {
        if want.image != got.image {
            case.diverge(&format!("quad{i}.image"), &want.image, &got.image);
        }
        if want.argb != got.argb {
            case.diverge(&format!("quad{i}.argb"), want.argb, got.argb);
        }
        case.numbers(&format!("quad{i}.pos"), &want.pos, &got.pos, 1e-4);
        case.numbers(&format!("quad{i}.uv"), &want.uv, &got.uv, 1e-4);
    }
}

struct Quad {
    image: String,
    argb: i64,
    pos: Vec<f32>,
    uv: Vec<f32>,
}

impl Quad {
    /// Coarse enough that float noise between the two sides does not reorder.
    fn key(&self) -> String {
        let rounded: Vec<String> = self.pos.iter().chain(&self.uv).map(|f| format!("{:.3}", f + 0.00001)).collect();
        format!("{} {}", self.image, rounded.join(","))
    }
}
