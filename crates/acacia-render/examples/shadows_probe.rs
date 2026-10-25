//! Probe of the parity harness shadows (tools/parity/harnesses/shadows): `shadows::radius` per
//! entity type and age, and `shadows::pieces` / `corners` over the oracle's scenes, fed the
//! oracle's per-cell facts. Prints `CASES\t<n>` and `DIVERGENCE\t<case>\t<field>\t<expected>\t<got>`.
//!
//! cargo run -p acacia-render --example shadows_probe -- <shadows.jsonl>

#[path = "java_entities/compare.rs"]
mod compare;

use std::collections::HashMap;

use acacia_render::shadows::{self, Ground, Shadow};
use glam::DVec3;
use serde_json::Value;

/// The oracle's cells: brightness and whether the block below is a floor, by `x,y,z`.
struct Cells(HashMap<String, (u8, bool)>);

impl Ground for Cells {
    fn floor(&self, x: i32, y: i32, z: i32) -> bool {
        self.0.get(&format!("{x},{},{z}", y + 1)).is_some_and(|c| c.1)
    }

    fn brightness(&self, x: i32, y: i32, z: i32) -> u8 {
        self.0.get(&format!("{x},{y},{z}")).map_or(0, |c| c.0)
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: shadows_probe <shadows.jsonl>");
    let mut out = compare::Report::default();
    for line in std::fs::read_to_string(path).expect("oracle records").lines().filter(|l| !l.trim().is_empty()) {
        let r: Value = serde_json::from_str(line).expect("oracle record");
        let name = r["case"].as_str().unwrap_or_default();
        let mut case = out.case(name);
        if let Some(error) = r.get("error") {
            case.diverge("oracle", "a record", error);
            continue;
        }
        let num = |k: &str| r[k].as_f64().unwrap_or(0.0);
        if let Some(rest) = name.strip_prefix("radius|") {
            let (kind, age) = rest.rsplit_once('|').unwrap_or((rest, "adult"));
            let expected = [num("radius") as f32, num("strength") as f32];
            let got = shadows::radius(kind, age == "baby", 2.0, 2.0).unwrap_or((0.0, 1.0));
            // A kind without a shadow needs no strength.
            if expected[0] == 0.0 && got.0 == 0.0 {
                continue;
            }
            case.numbers("radius, strength", &expected, &[got.0, got.1], 1e-6);
            continue;
        }
        let cells = r["cells"].as_object().into_iter().flatten().map(|(k, v)| (k.clone(), (v["brightness"].as_u64().unwrap_or(0) as u8, v["floor"].as_bool().unwrap_or(false))));
        let ground = Cells(cells.collect());
        let shadow = Shadow { feet: DVec3::new(num("x"), num("y"), num("z")), radius: num("radius") as f32, strength: num("strength") as f32 };
        let pieces = shadows::pieces(&shadow, num("distance_sq"), &ground, num("ambient") as f32);
        let expected = r["pieces"].as_array().cloned().unwrap_or_default();
        if expected.len() != pieces.len() {
            case.diverge("pieces", expected.len(), pieces.len());
            continue;
        }
        for (i, (want, got)) in expected.iter().zip(&pieces).enumerate() {
            let mut want_numbers = compare::floats(&want["relative"]);
            want_numbers.push(want["alpha"].as_f64().unwrap_or(0.0) as f32);
            case.numbers(&format!("piece {i}"), &want_numbers, &[got.relative[0], got.relative[1], got.relative[2], got.alpha], 1e-6);
            if compare::floats(&want["bounds"]) != [0.0, 0.0, 0.0, 1.0, 1.0, 1.0] {
                case.diverge(&format!("piece {i} bounds"), &want["bounds"], "a full block");
            }
        }
        let vertices: Vec<f32> = pieces
            .iter()
            .flat_map(|p| shadows::corners(p, shadow.radius).into_iter().flat_map(move |[x, y, z, u, v]| [x, y, z, u, v, (p.alpha * 255.0).floor() / 255.0]))
            .collect();
        let want: Vec<f32> = r["vertices"].as_array().into_iter().flatten().flat_map(compare::floats).collect();
        case.numbers("vertices", &want, &vertices, 1e-5);
    }
    out.print();
}
