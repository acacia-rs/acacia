//! Client entity definitions (`entity/*.entity.json`) and render controllers
//! (`render_controllers/*.json`): the resources a kind has, and the expressions choosing among
//! them from the entity's state. Resource keys are lowercased, like the expressions naming them.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

use super::molang::{Arrays, Program};
use crate::assets::json;

#[derive(Default)]
pub struct Definition {
    /// `(min_engine_version, format_version)`: the newest definition of a kind wins.
    version: (Vec<u32>, Vec<u32>),
    /// Short name (`default`, `baby`) to geometry identifier.
    pub geometry: HashMap<String, String>,
    /// Short name to image path without extension.
    pub textures: HashMap<String, String>,
    /// Short name to material name.
    pub materials: HashMap<String, String>,
    /// `initialize` then `pre_animation`: they set the variables controllers read.
    pub scripts: Vec<Program>,
    pub scale: Option<Program>,
    /// Controller identifiers with the condition they draw under.
    pub controllers: Vec<(String, Option<Program>)>,
}

pub struct Controller {
    pub arrays: Arrays,
    pub geometry: Program,
    pub textures: Vec<Program>,
    pub material: Option<Program>,
    /// Bone name patterns with their visibility, applied in order.
    pub part_visibility: Vec<(String, Program)>,
}

fn program(v: &Value) -> Option<Program> {
    match v {
        Value::String(s) => Program::parse(s),
        Value::Bool(b) => Some(Program::constant(f32::from(u8::from(*b)))),
        Value::Number(n) => Some(Program::constant(n.as_f64()? as f32)),
        _ => None,
    }
}

fn files(dir: &Path) -> impl Iterator<Item = Value> {
    let entries = std::fs::read_dir(dir).into_iter().flatten().flatten();
    entries.filter_map(|e| json::read(&e.path()).ok())
}

/// The single-key objects of a list such as `[{"*": "query.is_saddled"}]`.
fn pairs(v: Option<&Value>) -> impl Iterator<Item = (&String, &Value)> {
    v.and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_object).flatten()
}

pub fn definitions(root: &Path) -> HashMap<String, Definition> {
    let mut out: HashMap<String, Definition> = HashMap::new();
    for file in files(&root.join("entity")) {
        let Some(d) = file.pointer("/minecraft:client_entity/description") else { continue };
        let Some(kind) = d.get("identifier").and_then(Value::as_str) else { continue };
        let version = |v: Option<&Value>| -> Vec<u32> {
            v.and_then(Value::as_str).map(|s| s.split('.').filter_map(|p| p.parse().ok()).collect()).unwrap_or_default()
        };
        let map = |key: &str| -> HashMap<String, String> {
            let entries = d.get(key).and_then(Value::as_object).into_iter().flatten();
            entries.filter_map(|(k, v)| Some((k.to_lowercase(), v.as_str()?.to_owned()))).collect()
        };
        let script = |key: &str| d.pointer(&format!("/scripts/{key}")).and_then(Value::as_array).into_iter().flatten().filter_map(program);
        let controllers = d.get("render_controllers").and_then(Value::as_array).into_iter().flatten().filter_map(|c| match c {
            Value::String(id) => Some((id.clone(), None)),
            Value::Object(o) => o.iter().next().map(|(id, condition)| (id.clone(), program(condition))),
            _ => None,
        });
        let definition = Definition {
            version: (version(d.get("min_engine_version")), version(file.get("format_version"))),
            geometry: map("geometry"),
            textures: map("textures"),
            materials: map("materials"),
            scripts: script("initialize").chain(script("pre_animation")).collect(),
            scale: d.pointer("/scripts/scale").and_then(program),
            controllers: controllers.collect(),
        };
        if out.get(kind).is_none_or(|old| old.version < definition.version) {
            out.insert(kind.to_owned(), definition);
        }
    }
    out
}

pub fn controllers(root: &Path) -> HashMap<String, Controller> {
    let mut out = HashMap::new();
    for file in files(&root.join("render_controllers")) {
        let all = file.get("render_controllers").and_then(Value::as_object).into_iter().flatten();
        for (id, c) in all {
            let Some(geometry) = c.get("geometry").and_then(program) else { continue };
            // `arrays` groups its lists by resource type; names are unique across the groups.
            let lists = c.get("arrays").and_then(Value::as_object).into_iter().flatten().filter_map(|(_, group)| group.as_object()).flatten();
            let items = |list: &Value| list.as_array().into_iter().flatten().filter_map(|i| program(i)?.0.pop()).collect();
            out.insert(id.clone(), Controller {
                arrays: lists.map(|(name, list)| (name.to_lowercase(), items(list))).collect(),
                geometry,
                textures: c.get("textures").and_then(Value::as_array).into_iter().flatten().filter_map(program).collect(),
                material: pairs(c.get("materials")).next().and_then(|(_, m)| program(m)),
                part_visibility: pairs(c.get("part_visibility")).filter_map(|(bone, v)| Some((bone.to_lowercase(), program(v)?))).collect(),
            });
        }
    }
    out
}

/// Bone name patterns: `*` stands for any run of characters.
pub fn matches(pattern: &str, name: &str) -> bool {
    let Some((head, rest)) = pattern.split_once('*') else { return pattern == name };
    let Some(mut name) = name.strip_prefix(head) else { return false };
    let mut parts = rest.split('*').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            return name.ends_with(part);
        }
        let Some(at) = name.find(part) else { return false };
        name = &name[at + part.len()..];
    }
    true
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn bone_patterns_take_wildcards_anywhere() {
        assert!(matches("*", "head") && matches("left*", "leftarm") && matches("*arm", "leftarm") && matches("l*t*m", "leftarm"));
        assert!(matches("head", "head") && !matches("head", "head2") && !matches("left*", "rightarm") && !matches("*leg", "leftarm"));
    }
}
