//! A section mesh read back as the faces it draws, for tools that compare it with another
//! renderer (lookbake's `java-world-light`): world corners and each corner's [`VertexLight`] word.

use super::model::{CONTINUED, CORNER2, MODEL};
use super::quad::{AXES, LIQUID, Quad};
use super::SectionMesh;

/// One drawn face: corners in 1/256 block, world space, with their light words (all zero for a
/// face lit from the light volume). A merged run of whole faces comes back as one face per block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawnFace {
    pub corners: [[i32; 3]; 4],
    pub light: [u32; 4],
    /// The quad's face kind ([`super::quad`]), [`LIQUID`] for a liquid's model face, `None` for
    /// any other model face.
    pub kind: Option<u32>,
}

/// Every face of `mesh`, a section whose block origin is `origin`; of liquid faces, only those
/// drawn as model faces under Java lighting, with kind [`LIQUID`].
pub fn drawn_faces(mesh: &SectionMesh, origin: [i32; 3]) -> Vec<DrawnFace> {
    let mut out = Vec::new();
    for quads in [&mesh.solid, &mesh.translucent] {
        let mut i = 0;
        while i < quads.len() {
            let q = &quads[i];
            if q.0[0] & MODEL != 0 {
                let records = quads[i..].iter().take_while(|r| r.0[0] & MODEL != 0).enumerate().take_while(|(k, r)| *k == 0 || r.0[0] & CONTINUED != 0).count();
                out.push(model_face(&quads[i..i + records], origin));
                i += records;
                continue;
            }
            plain_faces(q, origin, &mut out);
            i += 1;
        }
    }
    out
}

/// Section-local 1/16 block to world 1/256 block.
fn world(origin: [i32; 3], local: [f32; 3]) -> [i32; 3] {
    [0, 1, 2].map(|a| origin[a] * 256 + (local[a] * 16.0).round() as i32)
}

fn model_face(records: &[Quad], origin: [i32; 3]) -> DrawnFace {
    let raw = |v: u32| (v & 0xffff) as f32 / 64.0 - 16.0;
    let (r0, r1, r2) = (&records[0].0, &records[1].0, &records[2].0);
    let c0 = [raw(r0[1]), raw(r0[1] >> 16), raw(r0[2])];
    let c1 = [raw(r0[2] >> 16), raw(r1[1]), raw(r1[1] >> 16)];
    let c3 = [raw(r1[2]), raw(r1[2] >> 16), raw(r2[1])];
    let c2 = if r0[0] & CORNER2 != 0 {
        let r3 = &records[3].0;
        [raw(r3[1]), raw(r3[1] >> 16), raw(r3[2])]
    } else {
        [0, 1, 2].map(|a| c1[a] + c3[a] - c0[a])
    };
    DrawnFace { corners: [c0, c1, c2, c3].map(|c| world(origin, c)), light: records[0].1, kind: (r0[0] & CORNER2 != 0).then_some(u32::from(LIQUID)) }
}

fn plain_faces(q: &Quad, origin: [i32; 3], out: &mut Vec<DrawnFace>) {
    let w0 = q.0[0];
    let kind = (w0 >> 27) & 15;
    if kind >= u32::from(LIQUID) {
        return;
    }
    let local = [w0 & 511, (w0 >> 9) & 511, (w0 >> 18) & 511].map(|v| v as f32);
    let size = [(q.0[1] & 255) + 1, ((q.0[1] >> 8) & 255) + 1].map(|v| v as f32);
    const CORNERS: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    if kind >= 6 {
        // Cross planes (gpu/terrain.wgsl): one diagonal of the block, front or back.
        let corners = CORNERS.map(|[u, v]| {
            let t = u * size[0];
            world(origin, [local[0] + if kind >= 8 { size[0] - t } else { t }, local[1] + v * size[1], local[2] + t])
        });
        out.push(DrawnFace { corners, light: q.1, kind: Some(kind) });
        return;
    }
    let (_, ua, va) = AXES[kind as usize];
    // A merged run spans whole blocks; its light is the same at every corner of every block.
    let (nu, nv) = if size[0] > 16.0 || size[1] > 16.0 { ((size[0] / 16.0) as u32, (size[1] / 16.0) as u32) } else { (1, 1) };
    let cell = if nu * nv > 1 { [16.0, 16.0] } else { size };
    for i in 0..nu {
        for j in 0..nv {
            let corners = CORNERS.map(|[u, v]| {
                let mut p = local;
                p[ua] += (i as f32 + u) * cell[0];
                p[va] += (j as f32 + v) * cell[1];
                world(origin, p)
            });
            out.push(DrawnFace { corners, light: q.1, kind: Some(kind) });
        }
    }
}
