//! `poses.jsonl` of entity_poses: our `setupAnim` on the case's render state.

use std::collections::BTreeMap;

use acacia_render::entity::java::{Family, Field, Part, Poses, RenderState, anim};
use serde_json::Value;

use crate::compare::Report;
use crate::drawn::compare_posed;

pub fn posed(layers: &BTreeMap<String, Part>, record: &Value, report: &mut Report) {
    let mut case = report.case(record["case"].as_str().unwrap_or_default());
    if let Some(error) = record["error"].as_str() {
        return case.diverge("oracle", "a pose", error);
    }
    let layer = record["layer"].as_str().unwrap_or_default();
    let Some(model) = layers.get(layer) else { return case.diverge("layer", "present", "absent in the look") };
    let Some(family) = Family::of_layer(layer) else { return case.diverge("setupAnim", "ported", "not ported") };
    let mut state = RenderState::from_json(&record["state"]);
    if let Some(hand) = record["swingHand"].as_str() {
        state.set("currentSwing", Field::Text(hand.to_owned()));
    }
    let mut poses = Poses::new(model);
    anim::setup(family, &state, &mut poses);
    compare_posed(&mut case, record, model, &poses);
}
