//! `poses.jsonl` of entity_poses: our `setupAnim` on the case's render state.

use std::collections::BTreeMap;

use acacia_render::entity::java::{Family, Field, Part, Poses, RenderState, anim};
use serde_json::Value;

use crate::compare::{Case, Report};
use crate::drawn::compare_posed;

pub fn posed(layers: &BTreeMap<String, Part>, record: &Value, report: &mut Report) {
    let mut case = report.case(record["case"].as_str().unwrap_or_default());
    if let Some((model, poses, _)) = setup(layers, record, &mut case) {
        compare_posed(&mut case, record, model, &poses);
    }
}

/// The case's model posed by our `setupAnim`; `None` (with the divergence noted) when the oracle
/// failed, the look lacks the layer or its `setupAnim` is not ported.
pub fn setup<'a>(layers: &'a BTreeMap<String, Part>, record: &Value, case: &mut Case) -> Option<(&'a Part, Poses, RenderState)> {
    if let Some(error) = record["error"].as_str() {
        case.diverge("oracle", "a pose", error);
        return None;
    }
    let layer = record["layer"].as_str().unwrap_or_default();
    let Some(model) = layers.get(layer) else {
        case.diverge("layer", "present", "absent in the look");
        return None;
    };
    let Some(family) = Family::of_layer(layer) else {
        case.diverge("setupAnim", "ported", "not ported");
        return None;
    };
    let mut state = RenderState::from_json(&record["state"]);
    if let Some(hand) = record["swingHand"].as_str() {
        state.set("currentSwing", Field::Text(hand.to_owned()));
    }
    let mut poses = Poses::new(model);
    anim::setup(family, &state, &mut poses);
    Some((model, poses, state))
}
