//! Client entity definitions (`entity/*.entity.json`) and render controllers
//! (`render_controllers/*.json`): the resources a kind has, and the expressions choosing among
//! them from the entity's state. Resource keys are lowercased, like the expressions naming them.

use std::collections::HashMap;
use std::path::Path;

use acacia_molang::{Arrays, Compiler};
use serde_json::Value;

use super::molang::{Loading, Program, compile};
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
    /// `initialize` then `pre_animation`, each one program: they set the variables controllers read.
    pub scripts: Vec<Program>,
    pub scale: Option<Program>,
    /// Controller identifiers with the condition they draw under.
    pub controllers: Vec<(String, Option<Program>)>,
    /// Short name to animation or animation controller identifier.
    pub animations: HashMap<String, String>,
    /// What plays, in order (`scripts.animate`, or the old `animation_controllers` list).
    pub animate: Vec<Playing>,
}

/// Its `arrays` are compiled into the programs that index them.
pub struct Controller {
    pub geometry: Program,
    pub textures: Vec<Program>,
    pub material: Option<Program>,
    /// Bone name patterns with their visibility, applied in order.
    pub part_visibility: Vec<(String, Program)>,
}

fn program_with(loading: &Loading, v: &Value, arrays: Option<&Arrays>) -> Option<Program> {
    match v {
        Value::String(s) => compile(loading, s, arrays),
        Value::Bool(b) => Some(Compiler::constant(f32::from(u8::from(*b)))),
        Value::Number(n) => Some(Compiler::constant(n.as_f64()? as f32)),
        _ => None,
    }
}

pub(super) fn program(loading: &Loading, v: &Value) -> Option<Program> {
    program_with(loading, v, None)
}

/// Unsupported Molang reads as 0, which leaves a bone alone or an animation off.
pub(super) fn program_or_zero(loading: &Loading, v: &Value) -> Program {
    program(loading, v).unwrap_or_else(|| Compiler::constant(0.0))
}

/// The short name of an animation with its blend weight (1 when absent).
pub type Playing = (String, Option<Program>);

/// `["setup", {"walk": "query.modified_move_speed"}]`
pub(super) fn playing(loading: &Loading, v: Option<&Value>) -> Vec<Playing> {
    let entry = |e: &Value| match e {
        Value::String(name) => Some((name.to_lowercase(), None)),
        Value::Object(o) => o.iter().next().map(|(name, weight)| (name.to_lowercase(), Some(program_or_zero(loading, weight)))),
        _ => None,
    };
    v.and_then(Value::as_array).into_iter().flatten().filter_map(entry).collect()
}

pub(super) fn files(dir: &Path) -> impl Iterator<Item = Value> {
    let entries = std::fs::read_dir(dir).into_iter().flatten().flatten();
    entries.filter_map(|e| json::read(&e.path()).ok())
}

/// The single-key objects of a list such as `[{"*": "query.is_saddled"}]`.
fn pairs(v: Option<&Value>) -> impl Iterator<Item = (&String, &Value)> {
    v.and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_object).flatten()
}

pub fn definitions(loading: &Loading, root: &Path) -> HashMap<String, Definition> {
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
        // The game runs a script's lines as one program: a block may open on one and close on another.
        let script = |key: &str| -> Vec<Program> {
            let lines = d.pointer(&format!("/scripts/{key}")).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str);
            let lines: Vec<&str> = lines.collect();
            match compile(loading, &lines.join(" "), None) {
                Some(whole) => vec![whole],
                // One line that does not compile should not cost the others.
                None => lines.iter().filter_map(|line| compile(loading, line, None)).collect(),
            }
        };
        let controllers = d.get("render_controllers").and_then(Value::as_array).into_iter().flatten().filter_map(|c| match c {
            Value::String(id) => Some((id.clone(), None)),
            Value::Object(o) => o.iter().next().map(|(id, condition)| (id.clone(), program(loading, condition))),
            _ => None,
        });
        let mut animations = map("animations");
        let mut animate = playing(loading, d.pointer("/scripts/animate"));
        if animate.is_empty() {
            // The old layout lists controllers by identifier and plays them all.
            for (name, id) in pairs(d.get("animation_controllers")) {
                animations.insert(name.to_lowercase(), id.as_str().unwrap_or_default().to_owned());
                animate.push((name.to_lowercase(), None));
            }
        }
        let definition = Definition {
            animations,
            animate,
            version: (version(d.get("min_engine_version")), version(file.get("format_version"))),
            geometry: map("geometry"),
            textures: map("textures"),
            materials: map("materials"),
            scripts: script("initialize").into_iter().chain(script("pre_animation")).collect(),
            scale: d.pointer("/scripts/scale").and_then(|scale| program(loading, scale)),
            controllers: controllers.collect(),
        };
        if out.get(kind).is_none_or(|old| old.version < definition.version) {
            out.insert(kind.to_owned(), definition);
        }
    }
    out
}

pub fn controllers(loading: &Loading, root: &Path) -> HashMap<String, Controller> {
    let mut out = HashMap::new();
    for file in files(&root.join("render_controllers")) {
        let all = file.get("render_controllers").and_then(Value::as_object).into_iter().flatten();
        for (id, c) in all {
            // `arrays` groups its lists by resource type; names are unique across the groups.
            let lists = c.get("arrays").and_then(Value::as_object).into_iter().flatten().filter_map(|(_, group)| group.as_object()).flatten();
            let items = |list: &Value| list.as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect();
            let named = |name: &String| name.to_lowercase().trim_start_matches("array.").to_owned();
            let arrays: Arrays = lists.map(|(name, list)| (named(name), items(list))).collect();
            let indexing = |v: &Value| program_with(loading, v, Some(&arrays));
            let Some(geometry) = c.get("geometry").and_then(indexing) else { continue };
            out.insert(id.clone(), Controller {
                geometry,
                textures: c.get("textures").and_then(Value::as_array).into_iter().flatten().filter_map(indexing).collect(),
                material: pairs(c.get("materials")).next().and_then(|(_, m)| indexing(m)),
                part_visibility: pairs(c.get("part_visibility")).filter_map(|(bone, v)| Some((bone.to_lowercase(), indexing(v)?))).collect(),
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
