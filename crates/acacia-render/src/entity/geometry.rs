//! Bedrock entity geometry files, both layouts: `"geometry.x:geometry.parent": {..}` keys
//! (format 1.8/1.10, with inheritance) and the `minecraft:geometry` array (1.12+).

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

use crate::assets::json;

/// Raw model-space box in 1/16 block; axes and rotations: see README "Entities".
#[derive(Debug, Clone, PartialEq)]
pub struct Cube {
    pub origin: [f32; 3],
    pub size: [f32; 3],
    pub inflate: f32,
    pub mirror: bool,
    /// Rotation in degrees around a pivot, on top of the bone's.
    pub rotation: Option<([f32; 3], [f32; 3])>,
    pub uv: Uv,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Uv {
    /// Top-left of the unfolded box.
    Box([f32; 2]),
    /// `(uv, uv_size)` per face in [`FACES`] order.
    Faces([Option<([f32; 2], [f32; 2])>; 6]),
}

/// Per-face UV keys, in the face order of [`super::bake`].
pub const FACES: [&str; 6] = ["west", "east", "up", "down", "south", "north"];

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Bone {
    pub name: String,
    pub parent: Option<String>,
    pub pivot: [f32; 3],
    pub rotation: [f32; 3],
    /// Turns this bone's own cubes around its pivot; child bones don't follow (sheep legs stay
    /// upright under a body laid flat this way).
    pub bind_pose_rotation: [f32; 3],
    pub cubes: Vec<Cube>,
    /// Free-form polygons (persona skins).
    pub polys: Vec<Vec<Corner>>,
}

/// A polygon corner: position, normal, and uv in 0..1 from the top-left.
pub type Corner = ([f32; 3], [f32; 3], [f32; 2]);

#[derive(Debug, Clone, PartialEq)]
pub struct Geometry {
    pub texture_size: [f32; 2],
    pub bones: Vec<Bone>,
}

/// Geometries the game has in code and packs only name: the boat's hull (Java's `BoatModel`, no paddles;
/// a boat's position is 0.375 above its bottom, and its bow points 90° left of its yaw).
const HARD_CODED: &str = include_str!("hardcoded.geo.json");
/// The texture size of geometries in the old layout that state none.
const LEGACY_TEXTURE: [f32; 2] = [64.0, 32.0];

/// Every geometry under `models/`, by identifier, with inheritance resolved. `texture_sizes` has
/// the size of the texture a geometry is drawn with, for files that state none.
pub fn load_all(root: &Path, texture_sizes: &HashMap<&str, [f32; 2]>) -> HashMap<String, Geometry> {
    let mut raw: HashMap<String, (Option<String>, Value)> = HashMap::new();
    collect(serde_json::from_str(HARD_CODED).unwrap_or_default(), &mut raw);
    let mut files = vec![root.join("models/mobs.json")];
    files.extend(std::fs::read_dir(root.join("models/entity")).into_iter().flatten().flatten().map(|e| e.path()));
    for file in files {
        match json::read(&file) {
            Ok(v) => collect(v, &mut raw),
            Err(e) => tracing::warn!(%e, "entity geometry"),
        }
    }
    let ids: Vec<String> = raw.keys().cloned().collect();
    let size = |id: &str| texture_sizes.get(id).copied().unwrap_or(LEGACY_TEXTURE);
    ids.into_iter().filter_map(|id| Some((id.clone(), resolve(&id, &raw, size(&id))?))).collect()
}

/// A geometry a skin brings along: `resource_patch` names it under `geometry.<key>`,
/// `geometry_data` is a geometry file defining it.
pub fn from_skin(resource_patch: &str, geometry_data: &str, key: &str) -> Option<Geometry> {
    let patch: Value = serde_json::from_str(resource_patch).ok()?;
    let name = patch.get("geometry")?.get(key)?.as_str()?;
    let mut raw = HashMap::new();
    collect(serde_json::from_str(geometry_data).ok()?, &mut raw);
    resolve(name, &raw, LEGACY_TEXTURE)
}

/// Every geometry of one file's text; nothing from a file that does not parse.
pub fn parse(data: &str) -> HashMap<String, Geometry> {
    let mut raw = HashMap::new();
    collect(serde_json::from_str(data).unwrap_or_default(), &mut raw);
    raw.keys().filter_map(|id| Some((id.clone(), resolve(id, &raw, LEGACY_TEXTURE)?))).collect()
}

fn collect(file: Value, raw: &mut HashMap<String, (Option<String>, Value)>) {
    let Value::Object(mut top) = file else { return };
    let mut add = |key: &str, body: Value| {
        let (id, parent) = key.split_once(':').map_or((key, None), |(id, p)| (id, Some(p.to_owned())));
        raw.insert(id.to_owned(), (parent, body));
    };
    if let Some(Value::Array(list)) = top.remove("minecraft:geometry") {
        for g in list {
            if let Some(id) = g.pointer("/description/identifier").and_then(Value::as_str).map(str::to_owned) {
                add(&id, g);
            }
        }
        return;
    }
    for (key, body) in top {
        if key.starts_with("geometry.") {
            add(&key, body);
        }
    }
}

fn resolve(id: &str, raw: &HashMap<String, (Option<String>, Value)>, unstated_size: [f32; 2]) -> Option<Geometry> {
    let (texture_size, bones) = inherit(id, raw, 0)?;
    // Only the `minecraft:geometry` layout leaves the size to the texture.
    let described = raw.get(id).is_some_and(|(_, body)| body.get("description").is_some());
    let texture_size = texture_size.unwrap_or(if described { unstated_size } else { LEGACY_TEXTURE });
    Some(Geometry { texture_size, bones: bones.iter().filter_map(|b| bone(b, texture_size)).collect() })
}

/// A bone's `poly_mesh`: index triples into its position, normal and uv lists. UVs count from the
/// bottom-left, in texels unless `normalized_uvs`.
fn polys(mesh: &Value, texture: [f32; 2]) -> Option<Vec<Vec<Corner>>> {
    let list = |key: &str| mesh.get(key).and_then(Value::as_array);
    let (positions, normals, uvs) = (list("positions")?, list("normals")?, list("uvs")?);
    let normalized = mesh.get("normalized_uvs").and_then(Value::as_bool).unwrap_or(false);
    let corner = |c: &Value| {
        let [p, n, t]: [usize; 3] = vec::<3>(Some(c))?.map(|i| i as usize);
        let [u, v]: [f32; 2] = vec(uvs.get(t))?;
        let uv = if normalized { [u, 1.0 - v] } else { [u / texture[0], 1.0 - v / texture[1]] };
        Some((vec(positions.get(p))?, vec(normals.get(n))?, uv))
    };
    Some(list("polys")?.iter().filter_map(|poly| poly.as_array()?.iter().map(corner).collect()).collect())
}

type RawBone = serde_json::Map<String, Value>;

/// Texture size and bones as JSON, the parent's first. A child's bone of the same name adds its
/// cubes and overrides the other keys it sets, or replaces the bone outright with `"reset": true`.
fn inherit(id: &str, raw: &HashMap<String, (Option<String>, Value)>, depth: usize) -> Option<(Option<[f32; 2]>, Vec<RawBone>)> {
    let (parent, body) = raw.get(id)?;
    let inherited = parent.as_deref().filter(|_| depth < 8).and_then(|p| inherit(p, raw, depth + 1));
    let (parent_size, mut bones) = inherited.unwrap_or_default();
    let size = |new: &str, old: &str| body.pointer(&format!("/description/{new}")).or_else(|| body.get(old)).and_then(Value::as_f64);
    let own = match (size("texture_width", "texturewidth"), size("texture_height", "textureheight")) {
        (Some(w), Some(h)) => Some([w as f32, h as f32]),
        _ => None,
    };
    for bone in body.get("bones").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_object) {
        let reset = bone.get("reset").and_then(Value::as_bool) == Some(true);
        match bones.iter_mut().find(|b| b.get("name") == bone.get("name")).filter(|_| !reset) {
            Some(slot) => {
                for (key, value) in bone {
                    match (key.as_str(), slot.get_mut(key), value) {
                        ("cubes", Some(Value::Array(old)), Value::Array(new)) => old.extend(new.iter().cloned()),
                        _ => drop(slot.insert(key.clone(), value.clone())),
                    }
                }
            }
            None => {
                bones.retain(|b| b.get("name") != bone.get("name"));
                bones.push(bone.clone());
            }
        }
    }
    Some((own.or(parent_size), bones))
}

fn vec<const N: usize>(v: Option<&Value>) -> Option<[f32; N]> {
    let a = v?.as_array()?;
    let mut out = [0.0; N];
    for (o, v) in out.iter_mut().zip(a) {
        *o = v.as_f64()? as f32;
    }
    (a.len() >= N).then_some(out)
}

fn bone(v: &RawBone, texture: [f32; 2]) -> Option<Bone> {
    let flag = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
    let num = |v: &RawBone, k: &str| v.get(k).and_then(Value::as_f64).map(|f| f as f32);
    let cubes = v.get("cubes").and_then(Value::as_array).filter(|_| !flag("neverRender"));
    let cubes = cubes.into_iter().flatten().filter_map(Value::as_object).filter_map(|c| {
        let uv = match c.get("uv")? {
            Value::Object(faces) => Uv::Faces(FACES.map(|f| {
                let face = faces.get(f)?;
                Some((vec(face.get("uv"))?, vec(face.get("uv_size"))?))
            })),
            other => Uv::Box(vec(Some(other))?),
        };
        let (origin, size): ([f32; 3], [f32; 3]) = (vec(c.get("origin"))?, vec(c.get("size"))?);
        // Without a pivot a cube turns around its centre.
        let centre = [0, 1, 2].map(|i| origin[i] + size[i] / 2.0);
        Some(Cube {
            origin,
            size,
            inflate: num(c, "inflate").or(num(v, "inflate")).unwrap_or(0.0),
            mirror: c.get("mirror").and_then(Value::as_bool).unwrap_or(flag("mirror")),
            rotation: vec(c.get("rotation")).map(|r| (r, vec(c.get("pivot")).unwrap_or(centre))),
            uv,
        })
    });
    Some(Bone {
        name: v.get("name")?.as_str()?.to_owned(),
        parent: v.get("parent").and_then(Value::as_str).map(str::to_owned),
        pivot: vec(v.get("pivot")).unwrap_or_default(),
        rotation: vec(v.get("rotation")).unwrap_or_default(),
        bind_pose_rotation: vec(v.get("bind_pose_rotation")).unwrap_or_default(),
        cubes: cubes.collect(),
        polys: v.get("poly_mesh").filter(|_| !flag("neverRender")).and_then(|m| polys(m, texture)).unwrap_or_default(),
    })
}
