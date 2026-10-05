//! Animations (`animations/*.json`) and animation controllers (`animation_controllers/*.json`):
//! per bone, Molang channels added to the rest pose. Names are lowercased, like bone names.
//! Played by [`super::pose`].

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use serde_json::Value;

use super::controller::{Playing, files, playing, program, program_or_zero};
use super::molang::Program;

pub struct Animation {
    /// `anim_time_update`: what `query.anim_time` reads; seconds alive when absent.
    pub time: Option<Program>,
    pub length: Option<f32>,
    pub looped: bool,
    pub bones: Vec<(Arc<str>, Tracks)>,
}

#[derive(Default)]
pub struct Tracks {
    /// Degrees, in the geometry's convention.
    pub rotation: Option<Track>,
    /// 1/16 block.
    pub position: Option<Track>,
    pub scale: Option<Track>,
}

pub enum Track {
    Fixed([Program; 3]),
    /// Sorted by time.
    Keys(Vec<Key>),
}

pub struct Key {
    pub time: f32,
    /// The value arriving at this key, and the one leaving it.
    pub pre: [Program; 3],
    pub post: [Program; 3],
}

/// A state machine choosing animations; see [`super::pose`] for how its state is found.
pub struct AnimationController {
    pub initial: String,
    pub states: HashMap<String, State>,
}

#[derive(Default)]
pub struct State {
    pub animations: Vec<Playing>,
    /// Target state and the condition that enters it.
    pub transitions: Vec<(String, Program)>,
}

#[derive(Default)]
pub struct Library {
    pub animations: HashMap<String, Animation>,
    pub controllers: HashMap<String, AnimationController>,
}

fn triple(v: &Value) -> [Program; 3] {
    match v.as_array() {
        Some(list) => std::array::from_fn(|i| list.get(i).map_or_else(|| Program::constant(0.0), program_or_zero)),
        None => std::array::from_fn(|_| program_or_zero(v)),
    }
}

fn track(v: &Value) -> Track {
    let Some(keys) = v.as_object() else { return Track::Fixed(triple(v)) };
    let key = |(time, v): (&String, &Value)| {
        let side = |name: &str, other: &str| triple(v.get(name).or_else(|| v.get(other)).unwrap_or(v));
        Some(Key { time: time.parse().ok()?, pre: side("pre", "post"), post: side("post", "pre") })
    };
    let mut keys: Vec<Key> = keys.iter().filter_map(key).collect();
    keys.sort_by(|a, b| a.time.total_cmp(&b.time));
    Track::Keys(keys)
}

fn animation(v: &Value) -> Animation {
    let bones = v.get("bones").and_then(Value::as_object).into_iter().flatten();
    let tracks = |b: &Value| Tracks { rotation: b.get("rotation").map(track), position: b.get("position").map(track), scale: b.get("scale").map(track) };
    Animation {
        time: v.get("anim_time_update").and_then(program),
        length: v.get("animation_length").and_then(Value::as_f64).map(|l| l as f32),
        looped: v.get("loop").and_then(Value::as_bool).unwrap_or(false),
        bones: bones.map(|(name, b)| (name.to_lowercase().into(), tracks(b))).collect(),
    }
}

fn state(v: &Value) -> State {
    let transitions = v.get("transitions").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_object).flatten();
    State {
        animations: playing(v.get("animations")),
        transitions: transitions.filter_map(|(to, condition)| Some((to.to_lowercase(), program(condition)?))).collect(),
    }
}

fn controller(v: &Value) -> AnimationController {
    let states = v.get("states").and_then(Value::as_object).into_iter().flatten();
    AnimationController {
        initial: v.get("initial_state").and_then(Value::as_str).unwrap_or("default").to_lowercase(),
        states: states.map(|(name, s)| (name.to_lowercase(), state(s))).collect(),
    }
}

pub fn load(root: &Path) -> Library {
    let mut out = Library::default();
    let entries = |file: &Value, key: &str| file.get(key).and_then(Value::as_object).cloned().into_iter().flatten();
    for file in files(&root.join("animations")) {
        out.animations.extend(entries(&file, "animations").map(|(id, a)| (id, animation(&a))));
    }
    for file in files(&root.join("animation_controllers")) {
        out.controllers.extend(entries(&file, "animation_controllers").map(|(id, c)| (id, controller(&c))));
    }
    out
}
