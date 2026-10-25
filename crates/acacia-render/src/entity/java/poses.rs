//! The poses `setupAnim` writes, one per part of a model, by part name (after Horizon's
//! `mobs/anim.rs` `Poses`, MIT). Starts from every part's initial pose (`resetPose`).

use std::collections::HashMap;

use super::model::{Part, PartPose};

#[derive(Debug, Clone, PartialEq)]
pub struct Poses {
    /// In [`Part::walk`] order.
    poses: Vec<PartPose>,
    /// `ModelPart.visible`: a hidden part and its children do not draw.
    visible: Vec<bool>,
    /// `ModelPart.skipDraw`: the part's own cubes do not draw, its children do.
    skip_draw: Vec<bool>,
    by_name: HashMap<String, usize>,
    /// What writes to a part the model lacks land on, as Java's would on a part it never looks up.
    scratch: PartPose,
}

impl Poses {
    pub fn new(model: &Part) -> Poses {
        let parts = model.walk();
        let mut by_name = HashMap::new();
        for (index, (_, part)) in parts.iter().enumerate() {
            by_name.entry(part.name.clone()).or_insert(index);
        }
        Poses {
            poses: parts.iter().map(|(_, p)| p.pose).collect(),
            visible: vec![true; parts.len()],
            skip_draw: vec![false; parts.len()],
            by_name,
            scratch: PartPose::default(),
        }
    }

    pub fn has(&self, name: &str) -> bool {
        self.by_name.contains_key(name)
    }

    pub fn get(&mut self, name: &str) -> &mut PartPose {
        match self.by_name.get(name) {
            Some(&index) => &mut self.poses[index],
            None => &mut self.scratch,
        }
    }

    /// A copy of `name`'s pose, or the default for a part the model lacks.
    pub fn read(&self, name: &str) -> PartPose {
        self.by_name.get(name).map_or_else(PartPose::default, |&i| self.poses[i])
    }

    pub fn x_rot(&mut self, name: &str, v: f32) {
        self.get(name).x_rot = v;
    }

    pub fn y_rot(&mut self, name: &str, v: f32) {
        self.get(name).y_rot = v;
    }

    pub fn z_rot(&mut self, name: &str, v: f32) {
        self.get(name).z_rot = v;
    }

    pub fn set_visible(&mut self, name: &str, v: bool) {
        if let Some(&index) = self.by_name.get(name) {
            self.visible[index] = v;
        }
    }

    pub fn set_skip_draw(&mut self, name: &str, v: bool) {
        if let Some(&index) = self.by_name.get(name) {
            self.skip_draw[index] = v;
        }
    }

    /// By index in [`Part::walk`] order.
    pub fn at(&self, index: usize) -> (PartPose, bool, bool) {
        (self.poses[index], self.visible[index], self.skip_draw[index])
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::{parse_layers, tests::SAMPLE};
    use super::*;

    #[test]
    fn writes_land_on_the_named_part_and_unknown_parts_are_ignored() {
        let model = parse_layers(SAMPLE).unwrap().remove("minecraft:test#main").unwrap();
        let mut poses = Poses::new(&model);
        assert_eq!(poses.read("head").x_rot, 0.5);
        poses.x_rot("head", 1.0);
        poses.x_rot("tail", 3.0);
        poses.set_visible("body", false);
        assert_eq!((poses.at(2).0.x_rot, poses.at(1).1, poses.has("tail")), (1.0, false, false));
    }
}
