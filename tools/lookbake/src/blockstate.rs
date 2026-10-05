//! Java blockstate files (`blockstates/<block>.json`): the models a block state draws and how each is turned.

use serde_json::Value;

use crate::mapping::JavaState;

/// One model placed in the block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRef {
    /// Without namespace: `block/oak_fence_side`.
    pub model: String,
    /// Quarter-turn rotations in degrees.
    pub x: u16,
    pub y: u16,
    /// Textures stay aligned to the world when the model is turned.
    pub uvlock: bool,
}

/// The models `state` draws: one for `variants` files, any number for `multipart`.
pub fn models(blockstate: &Value, state: &JavaState) -> Vec<ModelRef> {
    if let Some(variants) = blockstate.get("variants").and_then(Value::as_object) {
        let chosen = variants.iter().find(|(key, _)| key.split(',').filter(|p| !p.is_empty()).all(|p| holds(state, p)));
        return chosen.and_then(|(_, v)| model_ref(v)).into_iter().collect();
    }
    let parts = blockstate.get("multipart").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    parts.iter().filter(|part| part.get("when").is_none_or(|w| when(state, w))).filter_map(|part| model_ref(part.get("apply")?)).collect()
}

/// `key=value` of a variant name.
fn holds(state: &JavaState, pair: &str) -> bool {
    pair.split_once('=').is_some_and(|(k, v)| state.property(k) == Some(v))
}

/// A multipart condition: every key matches one of its `a|b` values, or `OR`/`AND` over conditions.
fn when(state: &JavaState, condition: &Value) -> bool {
    let Some(object) = condition.as_object() else { return false };
    object.iter().all(|(key, value)| match (key.as_str(), value) {
        ("OR", Value::Array(any)) => any.iter().any(|c| when(state, c)),
        ("AND", Value::Array(all)) => all.iter().all(|c| when(state, c)),
        // Values are strings, but booleans and numbers appear unquoted in some packs.
        (key, value) => {
            let wanted = value.as_str().map_or_else(|| value.to_string(), str::to_owned);
            state.property(key).is_some_and(|have| wanted.split('|').any(|w| w == have))
        }
    })
}

// TODO: a list is weighted random variants; the first is drawn everywhere.
fn model_ref(variant: &Value) -> Option<ModelRef> {
    let variant = variant.as_array().map_or(Some(variant), |list| list.first())?;
    let model = variant.get("model")?.as_str()?;
    let turn = |axis: &str| variant.get(axis).and_then(Value::as_u64).unwrap_or(0) as u16;
    Some(ModelRef {
        model: model.strip_prefix("minecraft:").unwrap_or(model).to_owned(),
        x: turn("x"),
        y: turn("y"),
        uvlock: variant.get("uvlock").and_then(Value::as_bool).unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn state(name: &str, properties: &[(&str, &str)]) -> JavaState {
        JavaState { name: name.into(), properties: properties.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect() }
    }

    fn names(models: &[ModelRef]) -> Vec<(&str, u16)> {
        models.iter().map(|m| (m.model.as_str(), m.y)).collect()
    }

    #[test]
    fn variants_match_on_the_properties_they_name() {
        let file = json!({"variants": {
            "half=bottom,facing=east": {"model": "minecraft:block/stairs"},
            "half=top,facing=east": {"model": "minecraft:block/stairs", "x": 180, "uvlock": true},
            "half=top,facing=south": [{"model": "minecraft:block/a", "y": 90}, {"model": "minecraft:block/b"}],
        }});
        let top = models(&file, &state("stairs", &[("facing", "east"), ("half", "top"), ("waterlogged", "false")]));
        assert_eq!(top, [ModelRef { model: "block/stairs".into(), x: 180, y: 0, uvlock: true }]);
        assert_eq!(names(&models(&file, &state("stairs", &[("facing", "south"), ("half", "top")]))), [("block/a", 90)]);
        assert!(models(&file, &state("stairs", &[("facing", "west"), ("half", "top")])).is_empty());
        let any = json!({"variants": {"": {"model": "block/stone"}}});
        assert_eq!(names(&models(&any, &state("stone", &[]))), [("block/stone", 0)]);
    }

    #[test]
    fn multipart_applies_every_part_whose_condition_holds() {
        let file = json!({"multipart": [
            {"apply": {"model": "block/post"}},
            {"when": {"north": "true"}, "apply": {"model": "block/side"}},
            {"when": {"east": "true"}, "apply": {"model": "block/side", "y": 90}},
            {"when": {"OR": [{"south": "low|tall"}, {"west": "tall", "up": "true"}]}, "apply": {"model": "block/cap"}},
            {"when": {"AND": [{"north": "true"}, {"east": "true"}]}, "apply": {"model": "block/corner"}},
        ]});
        let fence = |p: &[(&str, &str)]| models(&file, &state("fence", p));
        assert_eq!(names(&fence(&[("north", "false"), ("east", "true"), ("south", "none")])), [("block/post", 0), ("block/side", 90)]);
        assert_eq!(names(&fence(&[("north", "true"), ("east", "true"), ("south", "tall")])).len(), 5);
        assert_eq!(names(&fence(&[("west", "tall"), ("up", "false"), ("south", "none")])), [("block/post", 0)]);
    }
}
