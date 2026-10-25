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
    for (i, (want, &region)) in expected.iter().zip(&mesh.quad_regions).enumerate() {
        let region = &mesh.regions[region];
        let corners = [0, 1, 2, 5].map(|k| &vertices[i * 6 + k]);
        let pos: Vec<f32> = corners.iter().flat_map(|v| frame.transform_point3(Vec3::from(v.position)).to_array()).collect();
        let uv: Vec<f32> = corners.iter().flat_map(|v| region.sprite_uv(&mesh.skin, v.uv)).collect();
        if want["image"].as_str() != Some(region.image.as_str()) {
            case.diverge(&format!("quad{i}.image"), &want["image"], &region.image);
        }
        if want["argb"].as_i64() != Some(i64::from(region.argb as i32)) {
            case.diverge(&format!("quad{i}.argb"), &want["argb"], region.argb as i32);
        }
        case.numbers(&format!("quad{i}.pos"), &floats(&want["pos"]), &pos, 1e-4);
        case.numbers(&format!("quad{i}.uv"), &floats(&want["uv"]), &uv, 1e-4);
    }
}
