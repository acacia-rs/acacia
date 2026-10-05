// Sun, moon and stars, added onto the cleared sky before the terrain. Follows globals.wgsl.

struct Celestial {
    // The sky's turn around the east-west axis.
    turn: mat4x4<f32>,
    // x: star brightness, y: moon phase (0..8)
    params: vec4<f32>,
};

@group(0) @binding(1) var<uniform> sky: Celestial;
@group(0) @binding(2) var sun: texture_2d<f32>;
@group(0) @binding(3) var moon: texture_2d<f32>;
@group(0) @binding(4) var samp: sampler;

const SUN: u32 = 0u;
const MOON: u32 = 1u;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) kind: u32,
};

@vertex
fn vs_main(@location(0) position: vec3<f32>, @location(1) kind: u32, @location(2) uv: vec2<f32>) -> VsOut {
    var out: VsOut;
    out.clip = g.view_proj * vec4<f32>((sky.turn * vec4<f32>(position, 0.0)).xyz, 1.0);
    out.uv = uv;
    out.kind = kind;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let phase = u32(sky.params.y);
    let cell = vec2<f32>(f32(phase % 4u), f32(phase / 4u));
    let sun_color = textureSample(sun, samp, in.uv);
    let moon_color = textureSample(moon, samp, (in.uv + cell) * vec2<f32>(0.25, 0.5));
    if in.kind == SUN {
        return sun_color;
    }
    if in.kind == MOON {
        return moon_color;
    }
    return vec4<f32>(vec3<f32>(sky.params.x), 1.0);
}
