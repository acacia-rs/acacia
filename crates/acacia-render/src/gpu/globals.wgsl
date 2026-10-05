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
    // x: ambient brightness, y: sky light levels lost to the time of day
    light: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;

// Blend between the darker curve (0) and a brighter one (1), like the brightness setting.
const GAMMA: f32 = 0.5;

// Brightness of (block, sky) light levels.
fn curve(l: vec2<f32>) -> f32 {
    let f = max(l.x, l.y - g.light.y) / 15.0;
    let dark = f / (4.0 - 3.0 * f);
    let b = mix(dark, 1.0 - pow(1.0 - dark, 4.0), GAMMA);
    return b + g.light.x * (1.0 - b);
}

// Distances the fog measures to a point `rel` from the camera. x: for the fog, y: for the haze.
fn fog_dist(rel: vec3<f32>) -> vec2<f32> {
    let sphere = length(rel);
    return vec2(select(sphere, max(length(rel.xz), abs(rel.y)), g.fog_shape.y != 0.0), sphere);
}

fn fogged(rgb: vec3<f32>, dist: vec2<f32>) -> vec3<f32> {
    let ramp = saturate((dist.x - g.fog_shape.x) / (g.fog.w - g.fog_shape.x));
    let fog = select(smoothstep(0.0, 1.0, ramp), ramp, g.fog_shape.z != 0.0);
    let haze = select(0.0, saturate((dist.y - g.haze.x) / (g.haze.y - g.haze.x)), g.haze.y > 0.0);
    return mix(rgb, g.fog.rgb, max(fog, haze));
}
