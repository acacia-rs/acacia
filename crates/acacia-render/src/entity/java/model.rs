//! Java's baked model layers (`LayerDefinitions.createRoots()` then `bakeRoot()`): a tree of
//! parts, each with its initial pose and its cubes' textured quads, read from the file the
//! entity-model exporter writes (`tools/parity/entity_models`; README "Java entity models").

use std::collections::BTreeMap;

use serde_json::Value;

/// `ModelPart`'s pose fields: offset in 1/16 block, angles in radians, scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PartPose {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub x_rot: f32,
    pub y_rot: f32,
    pub z_rot: f32,
    pub x_scale: f32,
    pub y_scale: f32,
    pub z_scale: f32,
}

impl Default for PartPose {
    fn default() -> Self {
        PartPose { x: 0.0, y: 0.0, z: 0.0, x_rot: 0.0, y_rot: 0.0, z_rot: 0.0, x_scale: 1.0, y_scale: 1.0, z_scale: 1.0 }
    }
}

impl PartPose {
    pub fn from_array([x, y, z, x_rot, y_rot, z_rot, x_scale, y_scale, z_scale]: [f32; 9]) -> Self {
        PartPose { x, y, z, x_rot, y_rot, z_rot, x_scale, y_scale, z_scale }
    }

    pub fn to_array(self) -> [f32; 9] {
        [self.x, self.y, self.z, self.x_rot, self.y_rot, self.z_rot, self.x_scale, self.y_scale, self.z_scale]
    }

    /// `ModelPart.translateAndRotate`'s matrix, in blocks.
    pub fn matrix(&self) -> glam::Mat4 {
        use glam::{Mat4, Vec3};
        Mat4::from_translation(Vec3::new(self.x, self.y, self.z) / 16.0)
            * Mat4::from_quat(super::joml::rotation_zyx_quat(self.z_rot, self.y_rot, self.x_rot))
            * Mat4::from_scale(Vec3::new(self.x_scale, self.y_scale, self.z_scale))
    }
}

/// One face of a cube: four corners `[x, y, z, u, v]` (pixels; u and v over the texture's size).
#[derive(Debug, Clone, PartialEq)]
pub struct Polygon {
    pub vertices: [[f32; 5]; 4],
    pub normal: [f32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub name: String,
    pub pose: PartPose,
    pub polygons: Vec<Polygon>,
    pub children: Vec<Part>,
}

impl Part {
    pub fn parse(name: &str, value: &Value) -> Part {
        let floats = |v: &Value| -> Vec<f32> { v.as_array().into_iter().flatten().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect() };
        let mut pose = PartPose::default().to_array();
        for (slot, f) in pose.iter_mut().zip(floats(&value["pose"])) {
            *slot = f;
        }
        let normals: Vec<Vec<f32>> = value["normals"].as_array().into_iter().flatten().map(floats).collect();
        let polygons = value["polygons"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .filter_map(|(index, polygon)| {
                let corners: Vec<Vec<f32>> = polygon.as_array()?.iter().map(floats).collect();
                let corner = |i: usize| -> Option<[f32; 5]> { corners.get(i)?.get(..5)?.try_into().ok() };
                let normal = normals.get(index).and_then(|n| n.get(..3)?.try_into().ok()).unwrap_or_default();
                Some(Polygon { vertices: [corner(0)?, corner(1)?, corner(2)?, corner(3)?], normal })
            })
            .collect();
        let children = value["children"].as_object().into_iter().flatten().map(|(name, child)| Part::parse(name, child)).collect();
        Part { name: name.to_owned(), pose: PartPose::from_array(pose), polygons, children }
    }

    /// Depth first, the root first; each part with its path (`body/head`, the root's is empty).
    pub fn walk(&self) -> Vec<(String, &Part)> {
        fn visit<'a>(part: &'a Part, path: String, out: &mut Vec<(String, &'a Part)>) {
            for child in &part.children {
                let child_path = if path.is_empty() { child.name.clone() } else { format!("{path}/{}", child.name) };
                out.push((child_path.clone(), child));
                visit(child, child_path, out);
            }
        }
        let mut out = vec![(String::new(), self)];
        visit(self, String::new(), &mut out);
        out
    }
}

/// Every layer of the export by its name (`minecraft:cow#main`).
pub fn parse_layers(text: &str) -> serde_json::Result<BTreeMap<String, Part>> {
    let layers: BTreeMap<String, Value> = serde_json::from_str(text)?;
    Ok(layers.into_iter().map(|(name, value)| (name, Part::parse("root", &value))).collect())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// A 2-part layer in the exporter's format: a body with a head 4 pixels up from it.
    pub const SAMPLE: &str = r#"{"minecraft:test#main": {"pose": [0,0,0,0,0,0,1,1,1], "polygons": [], "normals": [], "children": {
        "body": {"pose": [0, 12, 0, 0, 0, 0, 1, 1, 1],
                 "polygons": [[[-4,0,-2,0.25,0.5],[4,0,-2,0.5,0.5],[4,12,-2,0.5,1],[-4,12,-2,0.25,1]]],
                 "normals": [[0,0,-1]],
                 "children": {"head": {"pose": [0,-4,0,0.5,0,0,1,1,1], "polygons": [], "normals": [], "children": {}}}}}}}"#;

    #[test]
    fn parses_the_exporters_tree() {
        let layers = parse_layers(SAMPLE).unwrap();
        let root = &layers["minecraft:test#main"];
        let paths: Vec<_> = root.walk().into_iter().map(|(path, _)| path).collect();
        assert_eq!(paths, ["", "body", "body/head"]);
        let body = &root.children[0];
        assert_eq!((body.pose.y, body.polygons[0].normal, body.polygons[0].vertices[2]), (12.0, [0.0, 0.0, -1.0], [4.0, 12.0, -2.0, 0.5, 1.0]));
        assert_eq!(body.children[0].pose.x_rot, 0.5);
    }
}
