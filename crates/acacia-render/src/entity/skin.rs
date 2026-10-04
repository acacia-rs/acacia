//! Player skins. A classic skin is a texture for one of the humanoid models; a persona skin
//! brings its own geometry, and keeps its face in a second texture with its own geometry.

use super::bake::{self, Mesh};
use super::geometry;

/// RGBA8 rows top to bottom. Instances sharing the `Arc` share the GPU texture.
pub struct Skin {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// The skin's own model; drawn instead of the instance's model.
    pub mesh: Option<Mesh>,
}

/// A skin as the server sends it.
pub struct SkinSource<'a> {
    pub size: (u32, u32),
    pub rgba: &'a [u8],
    /// Size and pixels of the face texture.
    pub face: Option<((u32, u32), &'a [u8])>,
    /// Names the geometries: `{"geometry": {"default": .., "animated_face": ..}}`.
    pub resource_patch: &'a str,
    pub geometry_data: &'a str,
}

impl Skin {
    pub fn new(source: SkinSource) -> Skin {
        let (width, height) = source.size;
        let own = |key: &str| {
            let geometry = geometry::from_skin(source.resource_patch, source.geometry_data, key)?;
            Some(bake::bake(&geometry)).filter(|m| !m.vertices.is_empty())
        };
        let plain = |mesh| Skin { width, height, rgba: source.rgba.to_vec(), mesh };
        let Some(mut body) = own("default") else { return plain(None) };
        let (Some(mut face), Some(((face_width, face_height), face_rgba))) = (own("animated_face"), source.face) else {
            return plain(Some(body));
        };
        // One texture for both: the face goes below the skin, and each mesh's UVs shrink to its part.
        let (atlas_width, atlas_height) = (width.max(face_width), height + face_height);
        let mut rgba = vec![0; (atlas_width * atlas_height * 4) as usize];
        let mut blit = |pixels: &[u8], (w, h): (u32, u32), top: u32| {
            for (row, line) in pixels.chunks_exact((w * 4) as usize).take(h as usize).enumerate() {
                let at = ((top + row as u32) * atlas_width * 4) as usize;
                rgba[at..at + line.len()].copy_from_slice(line);
            }
        };
        blit(source.rgba, (width, height), 0);
        blit(face_rgba, (face_width, face_height), height);
        let fit = |mesh: &mut Mesh, (w, h): (u32, u32), top: u32| {
            for v in &mut mesh.vertices {
                v.uv = [v.uv[0] * w as f32 / atlas_width as f32, (top as f32 + v.uv[1] * h as f32) / atlas_height as f32];
            }
        };
        fit(&mut body, (width, height), 0);
        fit(&mut face, (face_width, face_height), height);
        // The face's skeleton goes after the body's; its bones pose by the same names.
        let shift = body.joints.len();
        for v in &mut face.vertices {
            v.bone = (v.bone as usize + shift).min(bake::MAX_BONES - 1) as u32;
        }
        face.joints.iter_mut().for_each(|j| j.parent = j.parent.map(|p| p + shift));
        body.vertices.append(&mut face.vertices);
        body.bones.append(&mut face.bones);
        body.joints.append(&mut face.joints);
        Skin { width: atlas_width, height: atlas_height, rgba, mesh: Some(body) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATCH: &str = r#"{"geometry": {"default": "geometry.body", "animated_face": "geometry.face"}}"#;
    const GEOMETRY: &str = r#"{"format_version": "1.14.0", "minecraft:geometry": [
        {"description": {"identifier": "geometry.body", "texture_width": 4, "texture_height": 4},
         "bones": [{"name": "body", "pivot": [0, 24, 0], "poly_mesh": {"normalized_uvs": true,
            "positions": [[-4, 12, 0], [4, 12, 0], [4, 24, 0], [-4, 24, 0]], "normals": [[0, 0, -1]],
            "uvs": [[0, 0], [1, 0], [1, 1], [0, 1]], "polys": [[[0, 0, 0], [1, 0, 1], [2, 0, 2], [3, 0, 3]]]}}]},
        {"description": {"identifier": "geometry.face", "texture_width": 32, "texture_height": 64},
         "bones": [{"name": "head", "pivot": [0, 24, 0], "cubes": [{"origin": [-4, 24, -4], "size": [8, 8, 8], "uv": [0, 0]}]}]}]}"#;

    #[test]
    fn persona_skins_bake_their_polygons_and_stack_the_face_texture() {
        let (skin, face) = ([1u8; 4 * 4 * 4], [2u8; 2 * 4 * 4]);
        let source = SkinSource { size: (4, 4), rgba: &skin, face: Some(((2, 4), &face)), resource_patch: PATCH, geometry_data: GEOMETRY };
        let out = Skin::new(source);
        assert_eq!((out.width, out.height), (4, 8));
        assert_eq!((out.rgba[0], out.rgba[4 * 4 * 4], out.rgba[4 * 4 * 4 + 2 * 4]), (1, 2, 0), "skin, face, padding beside the face");
        let mesh = out.mesh.unwrap();
        assert_eq!(mesh.vertices.len(), 6 + 36);
        // The quad's first corner: bottom-left of the skin, which is the middle row of the atlas.
        assert_eq!((mesh.vertices[0].position, mesh.vertices[0].uv), ([-0.25, 0.75, 0.0], [0.0, 0.5]));
        assert!(mesh.vertices[6..].iter().all(|v| v.uv[1] >= 0.5 && v.uv[0] <= 0.5 && v.bone == 1));
        assert_eq!(mesh.bones, ["body", "head"]);
    }

    #[test]
    fn classic_skins_keep_their_texture_and_no_mesh() {
        let source = SkinSource { size: (1, 1), rgba: &[9; 4], face: None, resource_patch: r#"{"geometry":{"default":"geometry.humanoid.custom"}}"#, geometry_data: "" };
        let out = Skin::new(source);
        assert!(out.mesh.is_none() && out.rgba == [9; 4]);
    }
}
