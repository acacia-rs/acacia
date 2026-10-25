//! The Java look's translations (`lang/en_us.json`, copied from the jar by lookbake), which the
//! screens' `Lang` reads; a key without one shows as itself, as `Language.getOrDefault` does.

use std::collections::HashMap;
use std::path::Path;

#[derive(Default)]
pub struct JavaLang(HashMap<String, String>);

impl JavaLang {
    /// With `lang/deprecated.json` applied as the game loads it (removed keys dropped, renamed moved).
    pub fn load(root: &Path) -> JavaLang {
        let read = |name: &str| std::fs::read(root.join(name)).ok().and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
        let table: Option<HashMap<String, String>> = read("lang/en_us.json").and_then(|v| serde_json::from_value(v).ok());
        if table.is_none() {
            tracing::debug!(target: "ui", root = %root.display(), "no Java translations: screens show keys");
        }
        let mut map = table.unwrap_or_default();
        if let Some(deprecated) = read("lang/deprecated.json") {
            for key in deprecated["removed"].as_array().into_iter().flatten().filter_map(|k| k.as_str()) {
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
        JavaLang(map)
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }

    pub fn has(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    #[cfg(test)]
    pub fn of(pairs: &[(&str, &str)]) -> JavaLang {
        JavaLang(pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect())
    }
}
