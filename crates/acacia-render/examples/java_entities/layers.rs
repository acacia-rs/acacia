//! `layers.jsonl` of entity_layers: where each arm hands its item (`Mesh::hand`, then
//! `item::hand::fist`), under the entity pass's placement at yaw 0; and each armour slot's layer
//! posed by the wearer's `setupAnim` run on it (as `Library::pose` poses an armour model); each
//! second layer (wool, clothes, aura) with the body's poses, as one instance draws them.

use std::collections::BTreeMap;

use acacia_render::entity::java::{Family, Part, Poses, anim, mesh};
use acacia_render::item::hand::{Grip, fist};
use glam::{Mat4, Vec3};
use serde_json::Value;

use crate::compare::{Report, floats};
use crate::drawn::compare_posed;
use crate::poses::setup;

pub fn layers(layers: &BTreeMap<String, Part>, record: &Value, report: &mut Report) {
    let name = record["case"].as_str().unwrap_or_default();
    let mut case = report.case(name);
    let Some((model, poses, state)) = setup(layers, record, &mut case) else { return };
    let Some(family) = record["layer"].as_str().and_then(Family::of_layer) else { return };
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
    for (slot, worn) in record["armor"].as_object().into_iter().flatten() {
        let mut case = report.case(&format!("{name} {slot}"));
        let Some(armor) = worn["layer"].as_str().and_then(|layer| layers.get(layer)) else {
            case.diverge("layer", &worn["layer"], "absent in the look");
            continue;
        };
        let mut posed = Poses::new(armor);
        anim::setup(family, &state, &mut posed);
        compare_posed(&mut case, worn, armor, &posed);
    }
    for (suffix, overlay) in record["overlays"].as_object().into_iter().flatten() {
        let mut case = report.case(&format!("{name} {suffix}"));
        let Some(part) = overlay["layer"].as_str().and_then(|layer| layers.get(layer)) else {
            case.diverge("layer", &overlay["layer"], "absent in the look");
            continue;
        };
        compare_posed(&mut case, overlay, part, &poses.retarget(part));
    }
}
