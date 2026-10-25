//! [`VertexLight`] of the faces the mesher emits, under [`crate::look::Lighting::Java`].

use super::Ctx;
use super::quad::{AXES, LIT, SPLIT_13, VertexLight, vertex_light};
use super::smooth::{Dir, Lighter, OF_FACE, QuadIn, QuadOut, SectionLight, UP};
use crate::blocks::ModelFace;

pub(super) type SectionLighter<'a> = Lighter<SectionLight<'a>>;

/// Vanilla `FaceInfo`: a full face's vertices in Java's order, per Java direction.
const FACE_INFO: [[[f32; 3]; 4]; 6] = [
    [[0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 1.0]],
    [[0.0, 1.0, 0.0], [0.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, 0.0]],
    [[1.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
    [[0.0, 1.0, 1.0], [0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 1.0]],
    [[0.0, 1.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 1.0]],
    [[1.0, 1.0, 1.0], [1.0, 0.0, 1.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
];

impl Ctx<'_> {
    /// Java's light for vertices, when the table's look lights that way and the light is known.
    pub(super) fn lighter(&self) -> Option<SectionLighter<'_>> {
        let cells = self.cells.filter(|_| self.table.lighting == crate::look::Lighting::Java)?;
        Some(Lighter { world: SectionLight { ctx: self, cells }, nether: self.v.nether })
    }
}

fn words(out: &QuadOut) -> [u32; 4] {
    [0, 1, 2, 3].map(|k| vertex_light(out.coords[k] & 0xFF, out.coords[k] >> 16 & 0xFF, out.grey[k]))
}

/// Face `face` (the renderer's axis face) of the cube at `p`, in the plain quad's corner order.
pub(super) fn cube_face(lighter: &SectionLighter, p: [i32; 3], face: u8) -> VertexLight {
    let d = OF_FACE[usize::from(face)];
    let positions = FACE_INFO[usize::from(d)];
    let ctx = lighter.world.ctx;
    let ambient_occlusion = ctx.drawn(p, ctx.block(p)).light.ambient_occlusion;
    let out = words(&lighter.light(&QuadIn { pos: p, positions, shade: d, cull: Some(d), ambient_occlusion }));
    let (_, ua, va) = AXES[usize::from(face)];
    let mut light = [0; 4];
    let mut first = 0;
    for (k, v) in positions.iter().enumerate() {
        let corner = match (v[ua] > 0.5, v[va] > 0.5) {
            (false, false) => 0,
            (true, false) => 1,
            (true, true) => 2,
            (false, true) => 3,
        };
        light[corner] = out[k];
        if k == 0 {
            first = corner;
        }
    }
    if first % 2 == 1 {
        light[0] |= SPLIT_13;
    }
    light
}

/// A model face of the block at `p`: corner k is Java's vertex k.
pub(super) fn model_face(lighter: &SectionLighter, p: [i32; 3], f: &ModelFace) -> VertexLight {
    let [c0, c1, c3] = f.corners.map(|c| c.map(|v| v / 16.0));
    let c2 = [0, 1, 2].map(|i| c1[i] + c3[i] - c0[i]);
    // An unshaded face is one shaded as up (lookbake's bake), which the nether still dims.
    let shade: Dir = f.shade.map_or(UP, |s| OF_FACE[usize::from(s)]);
    let q = QuadIn { pos: p, positions: [c0, c1, c2, c3], shade, cull: f.cull.map(|s| OF_FACE[usize::from(s)]), ambient_occlusion: f.ambient_occlusion };
    words(&lighter.light(&q))
}

/// Whether a merged run of faces can share this face's light: the same at every corner.
pub(super) fn uniform(light: &VertexLight) -> bool {
    let strip = |w: u32| w & !SPLIT_13;
    light.iter().all(|&w| strip(w) == strip(light[0])) && (light[0] & LIT != 0 || light[0] == 0)
}
