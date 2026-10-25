//! Probe of the parity harness capes_elytra (tools/parity/harnesses/capes_elytra): replays each
//! of the oracle's tick sequences through `avatar::Cloak` + `avatar::cape` and `avatar::Wings`.
//! Prints `CASES\t<n>` and `DIVERGENCE\t<case>\t<field>\t<expected>\t<got>`.
//!
//! cargo run -p acacia-render --example capes_elytra -- <capes_elytra.jsonl>

#[path = "java_entities/compare.rs"]
mod compare;

use acacia_render::entity::java::avatar::{Cloak, Wings, cape};
use glam::DVec3;
use serde_json::Value;

fn vec(v: &Value) -> DVec3 {
    let n = |i: usize| v[i].as_f64().unwrap_or(0.0);
    DVec3::new(n(0), n(1), n(2))
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: capes_elytra <capes_elytra.jsonl>");
    let mut out = compare::Report::default();
    for line in std::fs::read_to_string(path).expect("oracle records").lines().filter(|l| !l.trim().is_empty()) {
        let r: Value = serde_json::from_str(line).expect("oracle record");
        let name = r["case"].as_str().unwrap_or_default();
        let mut case = out.case(name);
        let ticks = r["ticks"].as_array().cloned().unwrap_or_default();
        let num = |t: &Value, k: &str| t[k].as_f64().unwrap_or(0.0) as f32;
        if name.starts_with("cape|") {
            let mut cloak = Cloak::default();
            for (i, t) in ticks.iter().enumerate() {
                let (at, old, partial) = (vec(&t["at"]), vec(&t["old"]), num(t, "partial"));
                cloak.tick(at, num(t, "bob"));
                cloak.walked(num(t, "walked"));
                let position = old + (at - old) * f64::from(partial);
                let got = cape(&cloak, position, num(t, "yaw"), partial, num(t, "fall_flying_scale"));
                case.numbers(&format!("tick {i}"), &compare::floats(&t["cape"]), &got, 1e-4);
            }
        } else {
            let mut wings = Wings::default();
            for (i, t) in ticks.iter().enumerate() {
                let gliding = t["gliding"].as_bool().unwrap_or(false).then(|| vec(&t["movement"]));
                wings.tick(gliding, t["crouching"].as_bool().unwrap_or(false));
                case.numbers(&format!("tick {i}"), &compare::floats(&t["rot"]), &wings.at(num(t, "partial")), 1e-6);
            }
        }
    }
    out.print();
}
