// Shared head of the terrain and entity shaders: the per-frame uniform and the light curve.

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

// Blend between the darker curve (0) and a brighter one (1), like the brightness setting.
const GAMMA: f32 = 0.5;

// Brightness of (block, sky) light levels.
fn curve(l: vec2<f32>) -> f32 {
    let f = max(l.x, l.y) / 15.0;
    let dark = f / (4.0 - 3.0 * f);
    let b = mix(dark, 1.0 - pow(1.0 - dark, 4.0), GAMMA);
    return b + g.light.x * (1.0 - b);
}

fn fogged(rgb: vec3<f32>, dist: f32) -> vec3<f32> {
    return mix(rgb, g.fog.rgb, smoothstep(g.fog.w * 0.7, g.fog.w, dist));
}
