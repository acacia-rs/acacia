// Terrain quads by vertex pulling: 6 vertices per quad, no vertex or index buffers.
// Quad layout: see src/mesh/quad.rs. The instance index is the section slot.

struct Globals {
    view_proj: mat4x4<f32>,
    cam_block: vec4<i32>,
    cam_frac: vec4<f32>,
    // x: water surface opacity (biomes_client.json water_surface_transparency)
    water: vec4<f32>,
    // rgb: fog/sky colour, w: distance where fog is opaque
    fog: vec4<f32>,
    // x: ambient brightness
    light: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var<storage, read> quads: array<u32>;
@group(0) @binding(2) var<storage, read> origins: array<vec4<i32>>;
@group(0) @binding(3) var atlas: texture_2d_array<f32>;
@group(0) @binding(4) var atlas_sampler: sampler;
// Per slot, 18^3 light bytes; see light.wgsl.
@group(0) @binding(5) var<storage, read> light_cells: array<u32>;

var<private> U_AXIS: array<vec3<f32>, 6> = array(
    vec3(0.0, 0.0, 1.0), vec3(0.0, 0.0, 1.0), vec3(1.0, 0.0, 0.0),
    vec3(1.0, 0.0, 0.0), vec3(1.0, 0.0, 0.0), vec3(1.0, 0.0, 0.0),
);
var<private> V_AXIS: array<vec3<f32>, 6> = array(
    vec3(0.0, 1.0, 0.0), vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 1.0),
    vec3(0.0, 0.0, 1.0), vec3(0.0, 1.0, 0.0), vec3(0.0, 1.0, 0.0),
);
// Vanilla-style directional light: east/west, up, down, south/north, cross planes.
var<private> SHADE: array<f32, 10> = array(0.6, 0.6, 1.0, 0.5, 0.8, 0.8, 0.9, 0.9, 0.9, 0.9);
// Faces whose (u, v, u x v) basis is left-handed relative to the outward normal.
var<private> REVERSED: array<u32, 10> = array(1u, 0u, 1u, 0u, 0u, 1u, 0u, 1u, 0u, 1u);
var<private> TRIANGLES: array<array<u32, 6>, 4> = array(
    array(0u, 1u, 2u, 0u, 2u, 3u),
    array(0u, 2u, 1u, 0u, 3u, 2u),
    array(1u, 2u, 3u, 1u, 3u, 0u),
    array(1u, 3u, 2u, 1u, 0u, 3u),
);
var<private> CORNERS: array<vec2<f32>, 4> = array(vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 1.0));
var<private> AO: array<f32, 4> = array(0.45, 0.62, 0.8, 1.0);

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) shade: f32,
    @location(2) @interpolate(flat) layer: u32,
    @location(3) @interpolate(flat) tint_material: u32,
    @location(4) dist: f32,
    @location(5) @interpolate(flat) tint: vec3<f32>,
    // Section-local position in blocks.
    @location(6) local: vec3<f32>,
    // x: face, y: slot
    @location(7) @interpolate(flat) face_slot: vec2<u32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) slot: u32) -> VsOut {
    let q = vi / 6u;
    let w0 = quads[q * 3u];
    let w1 = quads[q * 3u + 1u];
    let w2 = quads[q * 3u + 2u];
    let local = vec3<f32>(f32(w0 & 511u), f32((w0 >> 9u) & 511u), f32((w0 >> 18u) & 511u));
    let face = (w0 >> 27u) & 15u;
    let size = vec2<f32>(f32(w1 & 511u), f32((w1 >> 9u) & 511u));
    let flip = (w2 >> 8u) & 1u;
    let corner = TRIANGLES[flip * 2u + REVERSED[face]][vi % 6u];
    let c = CORNERS[corner];

    var pos: vec3<f32>;
    var uv: vec2<f32>;
    if face < 6u {
        let u = U_AXIS[face];
        let v = V_AXIS[face];
        pos = local + u * (c.x * size.x) + v * (c.y * size.y);
        let pv = dot(pos, v);
        // Side textures run top-down; the repeat sampler tiles them per block.
        uv = vec2(dot(pos, u), select(-pv, pv, face == 2u || face == 3u)) / 16.0;
    } else {
        let t = c.x * size.x;
        pos = local + vec3(select(t, size.x - t, face >= 8u), c.y * size.y, t);
        uv = vec2(t, -pos.y) / 16.0;
    }

    let rel = vec3<f32>(origins[slot].xyz - g.cam_block.xyz) + pos / 16.0 - g.cam_frac.xyz;
    var out: VsOut;
    out.clip = g.view_proj * vec4(rel, 1.0);
    out.uv = uv;
    out.shade = SHADE[face] * AO[(w2 >> (corner * 2u)) & 3u];
    out.layer = (w1 >> 18u) & 4095u;
    out.tint_material = ((w1 >> 30u) & 3u) | (((w2 >> 9u) & 3u) << 2u);
    out.dist = length(rel);
    out.local = pos / 16.0;
    out.face_slot = vec2(face, slot);
    // 7-bit sRGB per channel; texels are linear after sampling, so the tint is too.
    let srgb = vec3<f32>(f32((w2 >> 11u) & 127u), f32((w2 >> 18u) & 127u), f32((w2 >> 25u) & 127u)) / 127.0;
    out.tint = pow(srgb, vec3(2.2));
    return out;
}

fn shade_texel(in: VsOut) -> vec4<f32> {
    let texel = textureSample(atlas, atlas_sampler, in.uv, in.layer);
    let tint = in.tint_material & 3u;
    let material = in.tint_material >> 2u;
    var rgb = texel.rgb;
    if tint != 0u {
        // Overlay: alpha marks the tinted part (the grass strip on grass block sides).
        rgb = select(rgb * in.tint, mix(rgb, rgb * in.tint, texel.a), material == 3u);
    }
    rgb = rgb * in.shade * brightness(in.face_slot.y, in.face_slot.x, in.local);
    let fog = smoothstep(g.fog.w * 0.7, g.fog.w, in.dist);
    // Water opacity is the biome's water_surface_transparency, not the texture's alpha.
    let alpha = select(texel.a, g.water.x, tint == 3u);
    return vec4(mix(rgb, g.fog.rgb, fog), alpha);
}

@fragment
fn fs_solid(in: VsOut) -> @location(0) vec4<f32> {
    let c = shade_texel(in);
    if (in.tint_material >> 2u) == 1u && c.a < 0.5 {
        discard;
    }
    return vec4(c.rgb, 1.0);
}

@fragment
fn fs_translucent(in: VsOut) -> @location(0) vec4<f32> {
    return shade_texel(in);
}
