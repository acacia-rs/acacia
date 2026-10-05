
// Smooth lighting from a section's bordered light volume (layout and OPAQUE_CELL: src/light/data.rs).
// Appended to terrain.wgsl, whose U_AXIS/V_AXIS it uses; `curve` is in globals.wgsl.

const OPAQUE: u32 = 255u;
const LIGHT_SIDE: i32 = 18;
const LIGHT_WORDS: u32 = 1458u;

var<private> NORMAL: array<vec3<f32>, 6> = array(
    vec3(1.0, 0.0, 0.0), vec3(-1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0),
    vec3(0.0, -1.0, 0.0), vec3(0.0, 0.0, 1.0), vec3(0.0, 0.0, -1.0),
);

fn cell_light(slot: u32, c: vec3<i32>) -> u32 {
    let q = clamp(c, vec3(-1), vec3(16)) + vec3(1);
    let i = u32((q.x * LIGHT_SIDE + q.z) * LIGHT_SIDE + q.y);
    return (light_cells[slot * LIGHT_WORDS + i / 4u] >> ((i % 4u) * 8u)) & 255u;
}

// (block, sky) levels of a light byte.
fn levels(v: u32) -> vec2<f32> {
    return vec2(f32(v >> 4u), f32(v & 15u));
}

// A face corner: the mean of the cells around it in the plane in front of the face, without
// opaque cells, and without the diagonal one when both edges are opaque.
fn corner(center: u32, e1: u32, e2: u32, diagonal: u32) -> vec2<f32> {
    var sum = levels(center);
    var n = 1.0;
    if e1 != OPAQUE { sum += levels(e1); n += 1.0; }
    if e2 != OPAQUE { sum += levels(e2); n += 1.0; }
    if diagonal != OPAQUE && (e1 != OPAQUE || e2 != OPAQUE) { sum += levels(diagonal); n += 1.0; }
    return sum / n;
}

fn flat_light(slot: u32, c: vec3<i32>) -> f32 {
    let v = cell_light(slot, c);
    return curve(levels(select(v, 0u, v == OPAQUE)));
}

fn brightness(slot: u32, face: u32, local: vec3<f32>) -> f32 {
    if face >= 6u {
        return flat_light(slot, vec3<i32>(floor(local)));
    }
    let n = NORMAL[face];
    // Just in front of the face: the next cell for a face on its block's side, the block's own
    // cell for a model face inside it.
    let s = local + n * 0.01;
    let c = vec3<i32>(floor(s));
    let center = cell_light(slot, c);
    if center == OPAQUE {
        // Faces inside their block (torches, plants against a wall) use the block's own cell.
        return flat_light(slot, vec3<i32>(floor(local - n * 0.01)));
    }
    let u = U_AXIS[face];
    let v = V_AXIS[face];
    let iu = vec3<i32>(u);
    let iv = vec3<i32>(v);
    var cells: array<u32, 9>;
    for (var j = 0; j < 3; j++) {
        for (var i = 0; i < 3; i++) {
            cells[j * 3 + i] = cell_light(slot, c + (i - 1) * iu + (j - 1) * iv);
        }
    }
    let c00 = corner(center, cells[3], cells[1], cells[0]);
    let c10 = corner(center, cells[5], cells[1], cells[2]);
    let c01 = corner(center, cells[3], cells[7], cells[6]);
    let c11 = corner(center, cells[5], cells[7], cells[8]);
    let fa = fract(dot(s, u));
    let fb = fract(dot(s, v));
    return curve(mix(mix(c00, c10, fa), mix(c01, c11, fa), fb));
}
