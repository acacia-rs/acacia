//! `layers.jsonl` of entity_layers: where each arm hands its item (`Mesh::hand`, then
//! `item::hand::fist`), under the entity pass's placement at yaw 0.

use std::collections::BTreeMap;

use acacia_render::entity::java::{Part, mesh};
use acacia_render::item::hand::{Grip, fist};
use glam::{Mat4, Vec3};
use serde_json::Value;

use crate::compare::{Report, floats};
use crate::poses::setup;

pub fn layers(layers: &BTreeMap<String, Part>, record: &Value, report: &mut Report) {
    let mut case = report.case(record["case"].as_str().unwrap_or_default());
    let Some((model, poses, state)) = setup(layers, record, &mut case) else { return };
    let body = Mat4::from_scale(Vec3::new(1.0, 1.0, -1.0));
    let expected = record["hands"].as_object().cloned().unwrap_or_default();
    for (arm, matrix) in &expected {
        let left = arm == "LEFT";
        let Some(hand) = mesh::hand(model, &poses, left) else {
            case.diverge(&format!("hand {arm}"), "an arm", "none");
            continue;
        };
        let got = body * fist(hand, Grip { left, baby: state.baby() });
        case.numbers(&format!("hand {arm}"), &floats(matrix), &got.to_cols_array(), 1e-5);
    }
}
