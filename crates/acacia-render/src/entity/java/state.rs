//! What `setupAnim` reads: the model's render state, its fields under their Java names
//! (`walkAnimationPos`, `isBaby`, `armPose`...), so a harness case and the viewer's mapping from
//! Bedrock entity state fill the same table (README "Java entity render state").

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    Num(f32),
    Flag(bool),
    /// An enum constant or an item identifier (`BOW_AND_ARROW`, `minecraft:bow`).
    Text(String),
    /// A list of numbers in double (the dragon's flight history, flattened).
    Nums(Vec<f64>),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RenderState {
    fields: HashMap<String, Field>,
}

impl RenderState {
    pub fn set(&mut self, name: &str, value: Field) -> &mut Self {
        self.fields.insert(name.to_owned(), value);
        self
    }

    pub fn with(mut self, name: &str, value: Field) -> Self {
        self.set(name, value);
        self
    }

    /// A number field; 0 when unset, a flag reads 0 or 1.
    pub fn num(&self, name: &str) -> f32 {
        match self.fields.get(name) {
            Some(Field::Num(v)) => *v,
            Some(Field::Flag(v)) => f32::from(u8::from(*v)),
            _ => 0.0,
        }
    }

    pub fn flag(&self, name: &str) -> bool {
        match self.fields.get(name) {
            Some(Field::Flag(v)) => *v,
            Some(Field::Num(v)) => *v != 0.0,
            _ => false,
        }
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        match self.fields.get(name) {
            Some(Field::Text(v)) => Some(v),
            _ => None,
        }
    }

    pub fn nums(&self, name: &str) -> &[f64] {
        match self.fields.get(name) {
            Some(Field::Nums(v)) => v,
            _ => &[],
        }
    }

    pub fn has(&self, name: &str) -> bool {
        self.fields.contains_key(name)
    }

    /// From a harness case: every key not starting with `@` (those steer the harness).
    pub fn from_json(case: &serde_json::Value) -> RenderState {
        let mut state = RenderState::default();
        for (name, value) in case.as_object().into_iter().flatten().filter(|(k, _)| !k.starts_with('@')) {
            let field = match value {
                serde_json::Value::Bool(b) => Field::Flag(*b),
                serde_json::Value::Number(n) => Field::Num(n.as_f64().unwrap_or(0.0) as f32),
                serde_json::Value::String(s) => Field::Text(s.clone()),
                serde_json::Value::Array(a) => Field::Nums(a.iter().filter_map(serde_json::Value::as_f64).collect()),
                _ => continue,
            };
            state.set(name.trim_start_matches('?'), field);
        }
        state
    }

    // LivingEntityRenderState, by the names setupAnim uses.
    pub fn walk_pos(&self) -> f32 {
        self.num("walkAnimationPos")
    }
    pub fn walk_speed(&self) -> f32 {
        self.num("walkAnimationSpeed")
    }
    pub fn age(&self) -> f32 {
        self.num("ageInTicks")
    }
    /// Head yaw against the body, degrees.
    pub fn y_rot(&self) -> f32 {
        self.num("yRot")
    }
    /// Head pitch, degrees.
    pub fn x_rot(&self) -> f32 {
        self.num("xRot")
    }
    pub fn swing(&self) -> f32 {
        self.num("swingAnimation")
    }
    pub fn baby(&self) -> bool {
        self.flag("isBaby")
    }
    /// `ageScale`: 1 unless set (a baby's is 0.5).
    pub fn age_scale(&self) -> f32 {
        if self.has("ageScale") { self.num("ageScale") } else { 1.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_harness_case() {
        let case = serde_json::json!({"walkAnimationPos": 3.5, "isBaby": true, "armPose": "CROSSED", "@rootYaw": 90.0, "?eatTick": 4});
        let state = RenderState::from_json(&case);
        assert_eq!((state.walk_pos(), state.baby(), state.text("armPose"), state.num("eatTick")), (3.5, true, Some("CROSSED"), 4.0));
        assert!(!state.has("@rootYaw"));
        assert_eq!(state.age_scale(), 1.0);
    }
}
