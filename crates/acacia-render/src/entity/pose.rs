//! Plays a kind's animations into a [`Pose`], and turns a pose into bone matrices. See README
//! "Entities" for what is approximated.

use std::array::from_fn;
use std::cell::Cell;
use std::sync::Arc;

use glam::{Mat4, Vec3};

use super::animation::{Animation, AnimationController, Key, State, Track};
use super::bake::{self, Joint, MAX_BONES, Mesh};
use super::controller::Definition;
use super::molang::{Program, Scope, Value};
use super::{EntityModels, ModelId};

/// What animations add to one bone's rest pose.
#[derive(Debug, Clone, PartialEq)]
pub struct BonePose {
    /// Lowercase.
    pub bone: Arc<str>,
    /// Degrees, added to the rest rotation.
    pub rotation: [f32; 3],
    /// 1/16 block.
    pub position: [f32; 3],
    pub scale: [f32; 3],
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pose(pub Vec<BonePose>);

impl Pose {
    pub fn get(&self, bone: &str) -> Option<&BonePose> {
        self.0.iter().find(|b| &*b.bone == bone)
    }

    fn index(&mut self, bone: &Arc<str>) -> usize {
        self.0.iter().position(|b| b.bone == *bone).unwrap_or_else(|| {
            self.0.push(BonePose { bone: bone.clone(), rotation: [0.0; 3], position: [0.0; 3], scale: [1.0; 3] });
            self.0.len() - 1
        })
    }
}

impl Mesh {
    /// Per bone, the matrix that moves its rest-pose vertices into `pose`.
    pub fn skin(&self, pose: &Pose) -> Vec<Mat4> {
        let local = |(joint, name): (&Joint, &String)| {
            let posed = pose.get(name);
            let turn = from_fn(|i| joint.rotation[i] + posed.map_or(0.0, |p| p.rotation[i]));
            let (moved, scale) = posed.map_or((Vec3::ZERO, Vec3::ONE), |p| (Vec3::from(p.position) / 16.0, Vec3::from(p.scale)));
            Mat4::from_translation(joint.pivot + moved) * bake::rotation(turn) * Mat4::from_scale(scale) * Mat4::from_translation(-joint.pivot)
        };
        let locals: Vec<Mat4> = self.joints.iter().zip(&self.bones).map(local).collect();
        let world = |index: usize| {
            let (mut matrix, mut parent) = (locals[index], self.joints[index].parent);
            for _ in 0..bake::MAX_DEPTH {
                let Some(p) = parent else { break };
                (matrix, parent) = (locals[p] * matrix, self.joints[p].parent);
            }
            matrix * self.joints[index].unbind
        };
        (0..self.joints.len().min(MAX_BONES)).map(world).collect()
    }

    /// Where an item held in the right hand sits, posed: the `rightitem` bone (Bedrock's hand
    /// locator) moved to its pivot, in model space. `None` for models without one.
    pub fn right_hand(&self, pose: &Pose) -> Option<Mat4> {
        let index = self.bones.iter().position(|b| b == "rightitem")?;
        let posed = *self.skin(pose).get(index)?;
        Some(posed * Mat4::from_translation(self.joints[index].pivot))
    }
}

/// Controllers inside controllers this deep still play.
const MAX_NESTING: usize = 4;
/// Transitions followed from a controller's initial state.
const MAX_HOPS: usize = 4;
/// Variables the game sets itself (or carries between frames, which nothing here does), as an
/// entity seen by another player at rest has them. Unset, reading one ends the whole script
/// (acacia-molang README, "Rules BDS follows"): the player's `pre_animation` stopped at
/// `attack_time` before setting `tcos0`, and its controllers never left first person.
pub(super) const BUILT_IN: [(&str, f32); 7] = [
    ("gliding_speed_value", 1.0),
    ("is_first_person", 0.0),
    ("is_paperdoll", 0.0),
    ("map_face_icon", 0.0),
    ("attack_time", 0.0),
    ("player_x_rotation", 0.0),
    ("hand_bob", 0.0),
];

impl Track {
    /// The channel at `time`; `this` is its value so far, and `lerp` receives how far `time` is
    /// between two keys.
    fn at(&self, time: f32, this: [f32; 3], scope: &mut Scope, lerp: &Cell<f32>) -> [f32; 3] {
        let mut eval = |programs: &[Program; 3]| {
            from_fn(|i| {
                scope.this = this[i];
                scope.num(&programs[i])
            })
        };
        let keys = match self {
            Track::Fixed(programs) => return eval(programs),
            Track::Keys(keys) => keys,
        };
        let next = keys.partition_point(|k: &Key| k.time <= time);
        match (next.checked_sub(1).map(|i| &keys[i]), keys.get(next)) {
            (Some(a), Some(b)) => {
                let f = (time - a.time) / (b.time - a.time);
                lerp.set(f);
                let (from, to): ([f32; 3], [f32; 3]) = (eval(&a.post), eval(&b.pre));
                from_fn(|i| from[i] + (to[i] - from[i]) * f)
            }
            (Some(a), None) => eval(&a.post),
            (None, Some(b)) => eval(&b.pre),
            (None, None) => [0.0; 3],
        }
    }
}

struct Play<'a, 's> {
    models: &'a EntityModels,
    definition: &'a Definition,
    /// The skeleton `this` reads rest rotations from.
    rest: Option<&'a Mesh>,
    scope: Scope<'s>,
    time: &'s Cell<f32>,
    lerp: &'s Cell<f32>,
    pose: Pose,
}

impl<'a> Play<'a, '_> {
    fn play(&mut self, name: &str, weight: f32, depth: usize) {
        let (models, definition) = (self.models, self.definition);
        let Some(id) = definition.animations.get(name).filter(|_| weight != 0.0 && depth <= MAX_NESTING) else { return };
        if let Some(animation) = models.animations.animations.get(id) {
            self.apply(animation, weight);
        } else if let Some(state) = models.animations.controllers.get(id).and_then(|c| self.state(c)) {
            for (name, condition) in &state.animations {
                let share = condition.as_ref().map_or(1.0, |c| self.scope.num(c));
                self.play(name, weight * share, depth + 1);
            }
        }
    }

    /// No state is kept between frames: the controller is walked from its initial state each
    /// time, which lands in the same place while conditions hold steady.
    fn state(&mut self, controller: &'a AnimationController) -> Option<&'a State> {
        let mut state = controller.states.get(&controller.initial)?;
        for _ in 0..MAX_HOPS {
            let taken = state.transitions.iter().find(|(_, condition)| self.scope.truthy(condition));
            match taken.and_then(|(to, _)| controller.states.get(to)) {
                Some(next) => state = next,
                None => break,
            }
        }
        Some(state)
    }

    fn apply(&mut self, animation: &Animation, weight: f32) {
        let life = (self.scope.query)("life_time").num();
        // So the default update, `anim_time + delta_time`, lands on the entity's age.
        self.time.set(life - (self.scope.query)("delta_time").num());
        let mut time = animation.time.as_ref().map_or(life, |p| self.scope.num(p));
        if let Some(length) = animation.length.filter(|l| *l > 0.0) {
            time = if animation.looped { time.rem_euclid(length) } else { time.min(length) };
        }
        self.time.set(time);
        for (bone, tracks) in &animation.bones {
            let rest = self.rest.and_then(|m| Some(m.joints[m.bones.iter().position(|b| **b == **bone)?].rotation)).unwrap_or_default();
            let index = self.pose.index(bone);
            if let Some(track) = &tracks.rotation {
                let so_far = self.pose.0[index].rotation;
                let value = track.at(time, from_fn(|i| rest[i] + so_far[i]), &mut self.scope, self.lerp);
                self.pose.0[index].rotation = from_fn(|i| so_far[i] + value[i] * weight);
            }
            if let Some(track) = &tracks.position {
                let so_far = self.pose.0[index].position;
                let value = track.at(time, so_far, &mut self.scope, self.lerp);
                self.pose.0[index].position = from_fn(|i| so_far[i] + value[i] * weight);
            }
            if let Some(track) = &tracks.scale {
                let so_far = self.pose.0[index].scale;
                let value = track.at(time, so_far, &mut self.scope, self.lerp);
                self.pose.0[index].scale = from_fn(|i| so_far[i] * (1.0 + (value[i] - 1.0) * weight));
            }
        }
    }
}

impl EntityModels {
    /// The pose of an entity of `kind` now. `model` is the skeleton its animations read rest
    /// rotations from; `query` answers as for [`EntityModels::appearance`], plus the moving
    /// state: `life_time`, `modified_distance_moved`, `modified_move_speed`, `target_x_rotation`
    /// and `target_y_rotation` (the head's pitch and its yaw against the body, in degrees).
    pub fn pose(&self, kind: &str, model: ModelId, query: &dyn Fn(&str) -> Value) -> Pose {
        let (time, lerp) = (Cell::new(0.0), Cell::new(0.0));
        let ask = |name: &str| match name {
            "anim_time" => Value::Num(time.get()),
            "key_frame_lerp_time" => Value::Num(lerp.get()),
            _ => query(name),
        };
        let mut pose = Pose::default();
        if let Some(definition) = self.kinds.get(kind) {
            let scope = self.scope(&ask);
            let rest = self.models.get(model as usize).map(|m| &m.mesh);
            let mut play = Play { models: self, definition, rest, scope, time: &time, lerp: &lerp, pose };
            for script in &definition.scripts {
                play.scope.run(script);
            }
            for (name, weight) in &definition.animate {
                let weight = weight.as_ref().map_or(1.0, |w| play.scope.num(w));
                play.play(name, weight, 0);
            }
            pose = play.pose;
        }
        if pose.get("head").is_none() {
            // Kinds without a look animation still turn their head.
            let rotation = [query("target_x_rotation").num(), query("target_y_rotation").num(), 0.0];
            pose.0.push(BonePose { bone: "head".into(), rotation, position: [0.0; 3], scale: [1.0; 3] });
        }
        pose
    }
}
