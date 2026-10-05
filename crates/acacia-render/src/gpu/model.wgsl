
// Model faces: three records per face (layout: src/mesh/model.rs). Appended to terrain.wgsl.

const MODEL: u32 = 0x80000000u;
const CONTINUED: u32 = 0x40000000u;

// Section-local 1/16 block from the low 16 bits.
fn model_coord(v: u32) -> f32 {
    return f32(v & 0xffffu) / 64.0 - 16.0;
}

fn model_texel(v: u32) -> f32 {
    return f32(v & 1023u) / 32.0;
}

fn model_vertex(q: u32, vi: u32, slot: u32) -> VsOut {
    var out: VsOut;
    let look = quads[q * 3u];
    if (look & CONTINUED) != 0u {
        // Outside the clip volume: the record's triangles vanish.
        out.clip = vec4(2.0, 2.0, 2.0, 1.0);
        return out;
    }
    let a1 = quads[q * 3u + 1u];
    let a2 = quads[q * 3u + 2u];
    let color = quads[q * 3u + 3u];
    let b1 = quads[q * 3u + 4u];
    let b2 = quads[q * 3u + 5u];
    let c0w = quads[q * 3u + 6u];
    let c1w = quads[q * 3u + 7u];
    let c2w = quads[q * 3u + 8u];
    let p0 = vec3(model_coord(a1), model_coord(a1 >> 16u), model_coord(a2));
    let p1 = vec3(model_coord(a2 >> 16u), model_coord(b1), model_coord(b1 >> 16u));
    let p3 = vec3(model_coord(b2), model_coord(b2 >> 16u), model_coord(c1w));
    let t0 = vec2(model_texel(c0w), model_texel(c0w >> 10u));
    let t1 = vec2(model_texel(c0w >> 20u), model_texel(c1w >> 16u));
    let t3 = vec2(model_texel(c2w), model_texel(c2w >> 10u));

    let corner = TRIANGLES[((look >> 28u) & 1u) * 2u][vi % 6u];
    let c = CORNERS[corner];
    let pos = p0 + (p1 - p0) * c.x + (p3 - p0) * c.y;
    let kind = look & 15u;
    let rel = vec3<f32>(origins[slot].xyz - g.cam_block.xyz) + pos / 16.0 - g.cam_frac.xyz;
    out.clip = g.view_proj * vec4(rel, 1.0);
    out.uv = (t0 + (t1 - t0) * c.x + (t3 - t0) * c.y) / 16.0;
    out.shade = select(1.0, SHADE[min(kind, 5u)], kind < 6u) * g.ao[(look >> (4u + corner * 2u)) & 3u];
    out.layer = (look >> 16u) & 4095u;
    out.tint_material = ((look >> 14u) & 3u) | (((look >> 12u) & 3u) << 2u);
    out.dist = fog_dist(rel);
    out.local = pos / 16.0;
    out.face_slot = vec2(kind, slot);
    let srgb = vec3<f32>(f32(color & 127u), f32((color >> 7u) & 127u), f32((color >> 14u) & 127u)) / 127.0;
    out.tint = pow(srgb, vec3(2.2));
    return out;
}
