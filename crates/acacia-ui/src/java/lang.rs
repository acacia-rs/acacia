//! The game's `en_us` translations as `Language.loadDefault` reads them.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

/// `lang/en_us.json` under `root` (the `assets/minecraft` layout) with `lang/deprecated.json`
/// applied (`DeprecatedTranslationsInfo.applyToMap`: removed keys dropped, renamed keys moved);
/// empty when the files are missing.
pub fn load(root: &Path) -> HashMap<String, String> {
    let read = |name: &str| std::fs::read_to_string(root.join("lang").join(name)).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok());
    let mut map: HashMap<String, String> = read("en_us.json").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    if let Some(deprecated) = read("deprecated.json") {
        apply_deprecated(&mut map, &deprecated);
    }
    map
}

fn apply_deprecated(map: &mut HashMap<String, String>, deprecated: &Value) {
    for key in deprecated["removed"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        map.remove(key);
    }
    for (from, to) in deprecated["renamed"].as_object().into_iter().flatten() {
        let to = to.as_str().unwrap_or_default().to_owned();
        match map.remove(from) {
            Some(value) => map.insert(to, value),
            None => map.remove(&to),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deprecated_keys_are_dropped_and_moved() {
        let mut map: HashMap<String, String> = [("old", "Old"), ("gone", "x"), ("kept", "y")].map(|(k, v)| (k.to_owned(), v.to_owned())).into();
        apply_deprecated(&mut map, &serde_json::json!({"removed": ["gone"], "renamed": {"old": "new"}}));
        let mut keys: Vec<_> = map.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        keys.sort();
        assert_eq!(keys, [("kept", "y"), ("new", "Old")]);
    }
}
