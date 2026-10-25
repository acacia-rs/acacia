// Shared head of the terrain and entity shaders: the per-frame uniform and the light curve.

struct Globals {
    view_proj: mat4x4<f32>,
    cam_block: vec4<i32>,
    cam_frac: vec4<f32>,
    // x: water surface opacity, negative for the texture's alpha
    water: vec4<f32>,
    // rgb: fog/sky colour, w: distance where fog is opaque
    fog: vec4<f32>,
    // x: distance where fog begins, y: 1 for a cylinder around the camera, z: 1 for a linear ramp
    fog_shape: vec4<f32>,
    // A second, spherical linear fog. x: where it begins, y: where it is opaque, 0 for none
    haze: vec4<f32>,
    // Brightness per ambient occlusion level, 3 unoccluded
    ao: vec4<f32>,
    // x: 1 when colours multiply as Java's do, in sRGB (Look::lighting)
    light: vec4<f32>,
    // The frame's lightmap (src/light/lightmap.rs): 256 RGBA8 texels, sky * 16 + block, four a word.
    lightmap: array<vec4<u32>, 64>,
};

@group(0) @binding(0) var<uniform> g: Globals;

fn lightmap_texel(block: i32, sky: i32) -> vec3<f32> {
    let i = u32(sky * 16 + block);
    return unpack4x8unorm(g.lightmap[i / 4u][i % 4u]).rgb;
}

// Colour of (block, sky) light levels, blending neighbouring levels like Java's linear sampler.
fn lightmap(l: vec2<f32>) -> vec3<f32> {
    let p = clamp(l, vec2(0.0), vec2(15.0));
    let lo = vec2<i32>(floor(p));
    let hi = min(lo + 1, vec2(15));
    let f = p - floor(p);
    let low = mix(lightmap_texel(lo.x, lo.y), lightmap_texel(hi.x, lo.y), f.x);
    let high = mix(lightmap_texel(lo.x, hi.y), lightmap_texel(hi.x, hi.y), f.x);
    return mix(low, high, f.y);
}

// One brightness for shaders that take no colour; the lightmap's green.
fn curve(l: vec2<f32>) -> f32 {
    return lightmap(l).g;
}

fn srgb_encode(c: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3(0.0031308));
}

fn srgb_decode(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, c <= vec3(0.04045));
}

// A linear colour lit by `factor` (lightmap times vertex colour): Java multiplies the stored
// sRGB values, the renderer's own lighting multiplies linear ones.
fn lit(rgb: vec3<f32>, factor: vec3<f32>) -> vec3<f32> {
    if g.light.x == 0.0 {
        return rgb * factor;
    }
    return srgb_decode(saturate(srgb_encode(rgb) * factor));
}

// Distances the fog measures to a point `rel` from the camera. x: for the fog, y: for the haze.
fn fog_dist(rel: vec3<f32>) -> vec2<f32> {
    let sphere = length(rel);
    return vec2(select(sphere, max(length(rel.xz), abs(rel.y)), g.fog_shape.y != 0.0), sphere);
}

// How much of the fog colour covers a point at `dist` ([`fog_dist`]), 0 to 1.
fn fog_value(dist: vec2<f32>) -> f32 {
    let ramp = saturate((dist.x - g.fog_shape.x) / (g.fog.w - g.fog_shape.x));
    let fog = select(smoothstep(0.0, 1.0, ramp), ramp, g.fog_shape.z != 0.0);
    let haze = select(0.0, saturate((dist.y - g.haze.x) / (g.haze.y - g.haze.x)), g.haze.y > 0.0);
    return max(fog, haze);
}

// A colour Java writes as is, for the sRGB target.
fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, c <= vec3(0.04045));
}

fn fogged(rgb: vec3<f32>, dist: vec2<f32>) -> vec3<f32> {
    return mix(rgb, g.fog.rgb, fog_value(dist));
}
